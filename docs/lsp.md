# LSP (Language Server Protocol) Integration Notes

## Overview

Stakgraph uses LSP as an **optional enhancement layer** on top of tree-sitter to achieve higher accuracy in code analysis. This document explains when, why, and how LSP is used.

## Architecture: Tiered Analysis

```
┌─────────────────────────────────────────┐
│  Tier 1: Tree-sitter (Always)          │
│  - Fast syntax-only parsing             │
│  - Pattern-based symbol extraction      │
│  - Heuristic linking                    │
│  - Works offline, no setup              │
│  Accuracy: ~60-70%                      │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  Tier 2: Tree-sitter + LSP (Optional)  │
│  - Tree-sitter extracts symbols         │
│  - LSP resolves cross-file references   │
│  - More accurate import/call graphs     │
│  - Requires LSP servers running         │
│  Accuracy: ~85-90%                      │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  Tier 3: Fallback Heuristics            │
│  - Same-file matching                   │
│  - Same-directory matching              │
│  - Name-based fuzzy matching            │
│  - Exclude mocks/tests                  │
│  - Graceful degradation                 │
└─────────────────────────────────────────┘
```

## When Does LSP Run?

**Answer: During INGESTION, not during querying**

### Complete Flow

```
┌─────────────────────────────────────────────────────────────┐
│  1. INITIALIZATION (Before Ingestion)                       │
│  ├─ Spawn LSP server (typescript-language-server, etc.)    │
│  ├─ Get CmdSender channel for communication                │
│  └─ Server runs in background process                      │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  2. SETUP PHASE (Index All Files)                          │
│  ├─ Send textDocument/didOpen for ALL files                │
│  ├─ LSP parses and indexes entire codebase                 │
│  ├─ Builds internal symbol table                           │
│  └─ Ready to answer queries                                │
│  Time: 10-30 seconds for 1000 files                        │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  3. EXTRACTION PHASE (For Each File)                       │
│  ├─ Tree-sitter extracts symbols                           │
│  ├─ For each identifier/call:                               │
│  │   ├─ Send textDocument/definition(line, col) to LSP    │
│  │   ├─ LSP returns target file + line (10-50ms)          │
│  │   └─ Create edge in graph                               │
│  └─ Insert nodes + edges to database                       │
│  Time: 30-60 seconds for 1000 files                        │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  4. LINKING PHASE (Post-processing)                        │
│  ├─ Run heuristic linkers (path matching)                  │
│  └─ Add cross-file edges not caught by LSP                 │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  5. DONE - Graph stored in Database                        │
│  ├─ LSP server terminates                                  │
│  └─ All edges pre-computed                                 │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  QUERYING (LSP not involved)                                │
│  ├─ Pure graph traversal                                    │
│  ├─ Instant responses (<1ms)                                │
│  └─ Uses pre-computed edges from ingestion                 │
└─────────────────────────────────────────────────────────────┘
```

## What LSP Provides

### 1. Import Resolution (File → File edges)

**Without LSP (Heuristic):**
```typescript
// Can't resolve where 'helper' comes from
import { helper } from './utils';
helper(); // ❌ Don't know where helper is defined
```

**With LSP:**
```rust
// For every identifier in the file:
let pos = Position::new(file, line, col);
let res = LspCmd::GotoDefinition(pos).send(lsp)?;

if let LspRes::GotoDefinition(Some(gt)) = res {
    let target_file = gt.file; // LSP tells us: "./utils.ts"
    let target_line = gt.line;  // And the exact line
    
    // Create Import edge: this_file -> target_file
    graph.add_edge(Edge::imports(file_node, target_node));
}
```

### 2. Function Call Resolution (Function → Function edges)

**Without LSP (Limited):**
```typescript
function main() {
  helper();  // ✅ Found (same file)
  utils.otherHelper(); // ❌ Can't resolve
  imported.method(); // ❌ Can't resolve
}
```

**With LSP:**
```rust
// When we find a function call:
let pos = Position::new(file, call_line, call_col);
let res = LspCmd::GotoDefinition(pos).send(lsp)?;

if let LspRes::GotoDefinition(Some(gt)) = res {
    // LSP tells us the actual target function
    let target_func = graph.find_node_at(
        NodeType::Function, 
        &gt.file, 
        gt.line
    );
    
    // Create Calls edge: caller -> target
    graph.add_edge(Edge::calls(caller, target_func));
}
```

### 3. Interface Implementation Resolution (Go, Rust)

**Without LSP (Unknown):**
```go
type Handler interface {
    Handle()
}

func Process(h Handler) {
    h.Handle() // ❌ Which implementation?
}
```

**With LSP:**
```rust
let pos = Position::new(file, interface_line, col);
let res = LspCmd::GotoImplementations(pos).send(lsp)?;

if let LspRes::GotoImplementations(Some(impl)) = res {
    // LSP tells us all implementations
    let trait_edge = Edge::implements(impl_node, interface_node);
    graph.add_edge(trait_edge);
}
```

### 4. Library Documentation (Hover)

**With LSP:**
```rust
let pos = Position::new(file, line, col);
let res = LspCmd::Hover(pos).send(lsp)?;

if let LspRes::Hover(Some(docs)) = res {
    // Get documentation for external libraries
    external_func.docs = Some(docs);
}
```

## Code Examples from Stakgraph

### Initialization

```rust
// repo.rs:389
fn start_lsp(root: &str, lang: &Lang, lsp: bool) -> Result<Option<CmdSender>> {
    Ok(if lsp {
        let (tx, rx) = tokio::sync::mpsc::channel(10000);
        spawn_analyzer(&root.into(), &lang.kind, rx)?;  // Spawns LSP server
        Some(tx)
    } else {
        None
    })
}
```

### Setup Phase

```rust
// builder/core.rs:340
fn setup_lsp(&self, filez: &[(String, String)]) -> Result<()> {
    if let Some(lsp_tx) = self.lsp_tx.as_ref() {
        for (filename, code) in filez {
            let didopen = DidOpen {
                file: filename.into(),
                text: code.to_string(),
                lang: self.lang.kind.clone(),
            };
            LspCmd::DidOpen(didopen).send(&lsp_tx)?;  // Open all files
        }
    }
    Ok(())
}
```

### Using LSP During Extraction

```rust
// lang/parse/collect.rs:440
for (target_name, row, col) in identifiers {
    let pos = Position::new(file, row, col)?;
    let res = lsp::Cmd::GotoDefinition(pos).send(lsp)?;  // LSP call!
    
    if let lsp::Res::GotoDefinition(Some(gt)) = res {
        let target_file = gt.file;
        
        // Skip library files
        if self.lang.is_lib_file(&target_file) {
            continue;
        }
        
        // Create Import edge
        edges.push(Edge::imports(file_node, target_node));
    }
}
```

### Conditional LSP Usage

```rust
pub fn collect_import_edges<G: Graph>(
    &self,
    code: &str,
    file: &str,
    graph: &G,
    lsp_tx: &Option<CmdSender>,  // Optional!
) -> Result<Vec<Edge>> {
    if let Some(lsp) = lsp_tx {
        // Use LSP for accurate resolution
        return self.collect_import_edges_with_lsp(code, file, graph, lsp);
    }
    
    // Fallback: pattern-based heuristics
    self.collect_import_edges_heuristic(code, file)
}
```

## LSP Infrastructure

### Docker Image

Stakgraph uses a Docker image with pre-built LSP servers:

```dockerfile
# sphinxlightning/stakgraph-lsp
# Contains LSP servers for:
- typescript-language-server (TypeScript/JavaScript)
- rust-analyzer (Rust)
- gopls (Go)
- pyright (Python)
- ruby-lsp (Ruby)
# ... etc
```

### Client Wrapper

```rust
pub enum Cmd {
    DidOpen(DidOpen),              // Open a file in LSP
    GotoDefinition(Position),       // Jump to definition
    GotoImplementations(Position),  // Find implementations
    Hover(Position),                // Get documentation
    Stop,                           // Shutdown LSP server
}

pub enum Res {
    Opened(String),
    GotoDefinition(Option<Position>),
    GotoImplementations(Option<Position>),
    Hover(Option<String>),
    Stopping,
    Fail(String),
}
```

### Communication Pattern

```rust
// Send command via async channel
let (res_tx, res_rx) = tokio::sync::oneshot::channel();
tx.send((Cmd::GotoDefinition(pos), res_tx)).await?;
let result = res_rx.await?;
```

## Performance Trade-offs

### Without LSP
```
1000 TypeScript files:
  Parse: 5 seconds
  Extract: 10 seconds
  Link (heuristics): 2 seconds
  Total: 17 seconds
  
Accuracy: ~60-70%
```

### With LSP
```
1000 TypeScript files:
  Parse: 5 seconds
  Setup LSP: 10 seconds (DidOpen all files)
  Extract: 45 seconds (GotoDefinition calls)
  Link (heuristics): 2 seconds
  Total: 62 seconds
  
Accuracy: ~85-90%
```

**Trade-off: 4x slower ingestion, but MUCH more accurate!**

### Why This Is Worth It

**LSP overhead is one-time cost:**
- Ingestion: Once (with LSP overhead)
- Queries: Thousands of times (instant, no LSP)

**Pre-computation wins:**
```rust
// During ingestion (once):
for each function call {
    let target = lsp.goto_definition(call_pos)?;  // 10-50ms
    graph.add_edge(caller -> target);              // Store in DB
}

// During query (many times):
graph.find_callers(function_id)  // <1ms, just DB lookup
```

## Why LSP During Ingestion (Not Querying)?

### 1. LSP Calls Are Expensive

Each `GotoDefinition` takes **10-50ms**:
- Need to parse syntax at position
- Resolve type information
- Follow import chains
- Search symbol table

Doing this thousands of times during queries would be too slow!

### 2. LSP Requires Full Context

To answer queries, LSP needs:
- ✅ All files indexed
- ✅ Dependencies loaded
- ✅ Type information computed
- ✅ Import paths resolved

**Heavy setup** - better to do once during ingestion.

### 3. Results Are Static

Once code is ingested:
- Definition locations don't change
- Import relationships are fixed
- Call graph is determined

No need to keep LSP running after ingestion.

### 4. Queries Are Pure Graph Traversal

After ingestion, queries are just:
```cypher
MATCH (caller:Function)-[:Calls]->(target:Function {name: 'helper'})
RETURN caller
```

No LSP needed - just traverse pre-computed edges!

## Fallback Heuristics

When LSP is unavailable or fails, stakgraph uses heuristics:

```rust
// Try LSP first
if let Some(lsp) = lsp_tx {
    let res = LspCmd::GotoDefinition(pos).send(&lsp)?;
    if let LspRes::GotoDefinition(Some(gt)) = res {
        return find_in_lsp_result(gt);
    }
}

// Fallback 1: Same file
if let Some(func) = find_in_same_file(func_name, file) {
    return Some(func);
}

// Fallback 2: Same directory
if let Some(func) = find_in_same_directory(func_name, file) {
    return Some(func);
}

// Fallback 3: Unique name match (exclude mocks)
if let Some(func) = find_unique_function(func_name, graph) {
    if !func.file.contains("mock") {
        return Some(func);
    }
}

// Give up
None
```

## Implementation Strategy for CG

### Phase 1: Tree-sitter Only (Current)
```
✅ Done
Accuracy: ~60%
Speed: Fast (seconds)
```

### Phase 2: Add Heuristics (Next)
```
TODO
Accuracy: ~70%
Speed: Fast (seconds)
- Same-file matching
- Path-based import guessing
- Name-based fuzzy matching
```

### Phase 3: Add LSP (Later)
```
TODO
Accuracy: ~90%
Speed: Moderate (minutes)
- Spawn typescript-language-server
- Send DidOpen for all files
- Query during extraction
- Handle timeouts/errors
```

### Proposed API

```rust
pub struct IngestOptions {
    pub db_path: String,
    pub project_path: String,
    pub threads: Option<usize>,
    pub clean: bool,
    pub use_lsp: bool,  // New flag
}

pub fn ingest(options: IngestOptions) -> Result<IngestStats> {
    let lsp_tx = if options.use_lsp {
        Some(spawn_typescript_lsp(&options.project_path)?)
    } else {
        None
    };
    
    // Pass lsp_tx through to parser
    for file in files {
        let parsed = parse_with_optional_lsp(file, content, &lsp_tx)?;
        // ...
    }
}
```

## Key Insights

1. **LSP is a build-time optimization, not runtime**
   - Expensive during ingestion (once)
   - Free during queries (many times)

2. **Progressive enhancement**
   - Works without LSP (degraded accuracy)
   - Better with LSP (high accuracy)
   - Graceful fallback on errors

3. **Right architectural choice**
   - Push complexity to ingestion (infrequent)
   - Keep queries fast (frequent)

4. **Trade-off is worth it**
   - 4x slower ingestion
   - 30% better accuracy
   - Queries remain instant

## Resources

- LSP Specification: https://microsoft.github.io/language-server-protocol/
- typescript-language-server: https://github.com/typescript-language-server/typescript-language-server
- rust-analyzer: https://rust-analyzer.github.io/
- gopls: https://github.com/golang/tools/tree/master/gopls
- Stakgraph LSP implementation: https://github.com/stakwork/stakgraph/tree/main/lsp
