# Tree-sitter Implementation Notes

## Overview

Context-graph (cg) uses **tree-sitter** for parsing, following the same pattern as stakgraph. This document captures architectural insights from studying stakgraph's implementation.

## Why Tree-sitter?

### Advantages

1. **Syntax-only parsing** - No type checking needed
   - Fast (~milliseconds per file)
   - No need to resolve imports or build type graph
   - Works even on incomplete/broken code

2. **Battle-tested grammars**
   - Used by GitHub for code navigation
   - Handles JSX/TSX, modern syntax, edge cases
   - Mature parsers for 40+ languages

3. **Robust error recovery**
   - Keeps parsing even with syntax errors
   - Good for real-world codebases

4. **Incremental updates**
   - Can re-parse just changed sections
   - Perfect for future incremental ingestion
   - Fast updates when files change

5. **Unified interface**
   - Same API for all languages
   - Easy to add Python, Go, Rust support later
   - Just swap grammar, same query approach

### Limitations

1. **Syntax-only** - No semantic analysis
   - Can't resolve imported names
   - Can't follow `obj.method()` to definition
   - Can't infer types

2. **Query language learning curve**
   - S-expressions are unfamiliar
   - Need to understand grammar node types
   - Debugging queries can be tricky

3. **Limited to static analysis**
   - No runtime information
   - Can't trace `eval()` or dynamic requires

## How Stakgraph Uses Tree-sitter

### Unified Stack Trait

Every language implements the same interface:

```rust
pub trait Stack {
    fn parse(&self, code: &str) -> Result<Tree>;
    fn function_definition_query(&self) -> String;
    fn class_definition_query(&self) -> String;
    fn function_call_query(&self) -> String;
    fn imports_query(&self) -> Option<String>;
    fn variables_query(&self) -> Option<String>;
    fn endpoint_finders(&self) -> Vec<String>;
    fn request_finder(&self) -> Option<String>;
    // ... etc
}
```

### Query-Based Extraction

Instead of walking the syntax tree manually, they use **tree-sitter queries**:

**Example: Extract Functions in TypeScript**
```rust
let query = Query::new(
    &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    r#"
    (function_declaration
      name: (identifier) @name) @function

    (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: [(arrow_function) (function_expression)])) @function
    "#,
)?;
```

This single query matches:
- `function myFunc() {}` (function_declaration)
- `const myFunc = () => {}` (arrow_function)
- `const myFunc = function() {}` (function_expression)

**Example: Extract API Requests in React**
```rust
let query = Query::new(
    &tree_sitter_typescript::LANGUAGE_TSX.into(),
    r#"
    ;; Matches: fetch('/api/users')
    (call_expression
        function: (identifier) @REQUEST_CALL (#eq? @REQUEST_CALL "fetch")
        arguments: (arguments [ (string) (template_string) ] @ENDPOINT)
    ) @ROUTE

    ;; Matches: axios.get('/api/users'), ky.post('/api/users')
    (call_expression
        function: (member_expression
            object: (identifier) @lib
            property: (property_identifier) @REQUEST_CALL 
                (#match? @REQUEST_CALL "^(get|post|put|delete|patch)$")
        )
        arguments: (arguments [ (string) (template_string) ] @ENDPOINT)
    ) @ROUTE

    ;; Matches: axios({ url: '/api/users' })
    (call_expression
        function: (identifier) @lib (#match? @lib "^(axios|ky|superagent)$")
        arguments: (arguments
            (object
                (pair
                    key: (property_identifier) @url_key (#eq? @url_key "url")
                    value: [ (string) (template_string) ] @ENDPOINT
                )
            )
        )
    ) @ROUTE
    "#,
)?;
```

### StreamingIterator Pattern

Tree-sitter's `QueryMatches` doesn't implement standard `Iterator`:

```rust
use streaming_iterator::StreamingIterator;

let mut cursor = QueryCursor::new();
let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

while let Some(match_) = matches.next() {
    for capture in match_.captures {
        let name = &query.capture_names()[capture.index as usize];
        let text = &content[capture.node.byte_range()];
        // Process captured node...
    }
}
```

## Multi-Language Support

Stakgraph supports **12 languages** using the same architecture:

- TypeScript (tree-sitter-typescript)
- React/JSX (tree-sitter-tsx)
- Angular (tree-sitter-typescript variant)
- Svelte (tree-sitter-svelte)
- Python (tree-sitter-python)
- Rust (tree-sitter-rust)
- Go (tree-sitter-go)
- Ruby (tree-sitter-ruby)
- Java (tree-sitter-java)
- Kotlin (tree-sitter-kotlin)
- Swift (tree-sitter-swift)
- C++ (tree-sitter-cpp)

**Adding a new language = ~500 lines of tree-sitter queries**

### Example: Language Implementation

```rust
pub struct Python(Language);

impl Python {
    pub fn new() -> Self {
        Python(tree_sitter_python::LANGUAGE.into())
    }
}

impl Stack for Python {
    fn parse(&self, code: &str) -> Result<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.0)?;
        Ok(parser.parse(code, None)?)
    }
    
    fn function_definition_query(&self) -> String {
        r#"
        (function_definition
            name: (identifier) @FUNCTION_NAME
            parameters: (parameters)? @ARGUMENTS
        ) @FUNCTION_DEFINITION
        "#.to_string()
    }
    
    // ... implement other queries
}
```

## What Tree-sitter Can and Can't Do

### ✅ Can Extract

```typescript
// Functions
function myFunc() { ... }           // ✅ Detected
const arrow = () => { ... }         // ✅ Detected
async function asyncFn() { ... }    // ✅ Detected

// Classes
class MyClass { ... }               // ✅ Detected
class Sub extends Base { ... }      // ✅ Detected (but doesn't resolve Base)

// Interfaces
interface Person { ... }            // ✅ Detected

// Simple calls
helper();                           // ✅ Detected

// Imports (as text)
import { thing } from './module';   // ✅ Detected (but doesn't resolve path)

// API Requests
fetch('/api/users')                 // ✅ Detected
axios.get('/api/users')             // ✅ Detected

// Endpoints
router.get('/api/users', handler)   // ✅ Detected
```

### ❌ Can't Resolve

```typescript
// Can't follow imports
import { helper } from './utils';
helper();  // ❌ Doesn't know helper is from utils

// Can't resolve method calls
obj.method();  // ❌ Can't find method definition

// Can't infer types
const x = getSomething();  // ❌ Doesn't know x's type
x.doThing();               // ❌ Can't link to doThing()

// Can't handle dynamic code
const funcName = 'helper';
this[funcName]();  // ❌ Can't resolve
```

## Post-Processing: The Linker

After tree-sitter extraction, stakgraph runs **heuristic linkers** to connect nodes:

### API Request → Endpoint Linking

```rust
pub fn link_api_nodes(graph: &mut Graph) -> Result<()> {
    let frontend_requests = graph.find_nodes_by_type(NodeType::Request);
    let backend_endpoints = graph.find_nodes_by_type(NodeType::Endpoint);
    
    for request in frontend_requests {
        let normalized_path = normalize_path(&request.name); // "/api/users"
        
        for endpoint in backend_endpoints {
            if paths_match(normalized_path, &endpoint.name) {
                graph.add_edge(Edge::calls(request, endpoint));
            }
        }
    }
}
```

### Test → Component Linking (String Matching)

```rust
pub fn link_e2e_tests(graph: &mut Graph) -> Result<()> {
    let e2e_tests = graph.find_nodes_by_type(NodeType::E2eTest);
    let pages = graph.find_nodes_by_type(NodeType::Page);
    
    for test in e2e_tests {
        let body_lowercase = test.body.to_lowercase();
        
        for page in pages {
            // String matching: if test mentions page name, link them
            if body_lowercase.contains(&page.name.to_lowercase()) {
                graph.add_edge(Edge::test_calls(test, page));
            }
        }
    }
}
```

### Test ID Linking (React-specific)

```rust
// Extract data-testid="login-button" from components
pub fn extract_test_ids(content: &str) -> Vec<String> {
    let regex = Regex::new(r#"data-testid="([^"]+)""#)?;
    regex.captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

// Link E2E tests that use those test IDs
for (test, test_ids) in e2e_tests {
    for (component, component_test_ids) in components {
        if test_ids.intersects(&component_test_ids) {
            graph.add_edge(Edge::linked_e2e_test_call(test, component));
        }
    }
}
```

## Heuristic Strategies

### Test Classification

```rust
fn classify_test(name: &str, file: &str, body: &str) -> NodeType {
    // 1. Path-based (strongest signal)
    if file.contains("/e2e/") || file.contains(".e2e.") {
        return NodeType::E2eTest;
    }
    
    if file.contains("/integration/") {
        return NodeType::IntegrationTest;
    }
    
    // 2. Body heuristics
    let has_browser = body.contains("page.goto(") || body.contains("cy.");
    if has_browser {
        return NodeType::E2eTest;
    }
    
    let has_network = body.contains("fetch(") || body.contains("axios.");
    if has_network {
        return NodeType::IntegrationTest;
    }
    
    // Default
    return NodeType::UnitTest;
}
```

### Component Detection (React)

```rust
fn is_component(func_name: &str) -> bool {
    // React convention: Components start with capital letter
    func_name.chars().next().unwrap().is_uppercase()
}
```

### Library vs User Code

```rust
fn is_lib_file(file_name: &str) -> bool {
    file_name.contains("node_modules/")
        || file_name.contains("/lib/")
        || file_name.ends_with(".d.ts")
        || file_name.starts_with("/usr")
        || file_name.contains(".nvm/")
}
```

## Tree-sitter Query Playground

Test queries in browser: https://tree-sitter.github.io/tree-sitter/playground

**Example:** Parse TypeScript and query function calls:
```typescript
function helper() {
  return "hello";
}

function main() {
  helper();
}
```

**Query:**
```scheme
(call_expression
  function: (identifier) @function_name)
```

Result: Highlights `helper()` call!

## Performance Characteristics

**Tree-sitter is FAST:**
- ~1-5ms per typical file
- Parallel parsing scales linearly
- No external dependencies
- Works offline

**Stakgraph benchmarks:**
```
1000 TypeScript files:
- Parse: 5 seconds (tree-sitter)
- Extract: 10 seconds (queries + processing)
- Link: 2 seconds (heuristics)
Total: 17 seconds
```

## Key Insight

**Stakgraph achieves 80% of semantic analysis value with 20% of the complexity** by:

1. **Syntax-only extraction** (tree-sitter queries)
2. **Pattern matching & heuristics** (string matching, path normalization)
3. **Convention over configuration** (naming conventions, file paths)
4. **Post-processing linker** (connect the dots after parsing)

No TypeScript compiler, no type checker, no full import resolution needed!

## Resources

- Tree-sitter docs: https://tree-sitter.github.io/tree-sitter/
- Query syntax: https://tree-sitter.github.io/tree-sitter/using-parsers#query-syntax
- Playground: https://tree-sitter.github.io/tree-sitter/playground
- Available grammars: https://github.com/tree-sitter
- Stakgraph source: https://github.com/stakwork/stakgraph
