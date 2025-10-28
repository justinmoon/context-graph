# Context Graph - Rust/Kuzu Implementation Plan

## Current Status
✅ Basic workspace, DB layer, parser (functions/classes/interfaces/imports), ingestion pipeline, CLI
✅ **All critical bugs FIXED** - Production-ready for basic use
✅ **Phase 2 COMPLETE** - Query/find commands, improved parser with method calls and Implements edges
✅ **Phase 3 COMPLETE** - Comprehensive test suite: 12 CLI tests, 4 legacy parity tests, real-world smoke test

## Critical Bugs (FIXED ✅)
1. ✅ **SQL Injection Risk** - Implemented proper Cypher escaping (`escape_kuzu_string()`)
2. ✅ **No Re-ingest** - Added `delete_file_and_symbols()` for safe upsert
3. ✅ **ThreadPool Panic** - Fixed: use local pool with `install()` or default pool
4. ✅ **Calls Edges** - Parser now extracts function call relationships

## Immediate Roadmap

### Phase 1: Fix Critical Bugs ✅ COMPLETE
1. ✅ Fix ThreadPoolBuilder (use default pool or lazy_static)
2. ✅ Add upsert/MERGE logic or delete-before-insert
3. ✅ Fix SQL escaping (proper Kuzu escaping or parameters)
4. ✅ Extract Calls edges in parser

### Phase 2: Core Features ✅ COMPLETE
5. ✅ Implement query/find CLI commands (`cg query`, `cg find symbol`, `cg find callers`)
6. ✅ Node types already defined: DataModel, Var, Endpoint, Request, Page (parser TBD)
7. ✅ Extract Implements and Extends edges (class/interface relationships)
8. ✅ Improve call graph: method calls (console.log, obj.method)
9. ✅ Remove legacy fixture mod.rs files

**Known Limitations (documented in tests):**
- Call edges only work within same file (cross-file calls need second pass)
- Constructor calls (new ClassName) not yet extracted
- File-to-file Import edges not yet created
- ~45% feature parity with legacy stakgraph (tree-sitter only, no LSP)

### Phase 3: Testing & Validation ✅ COMPLETE
10. ✅ Add CLI smoke tests with assert_cmd (12 tests passing)
11. ✅ Port legacy stakgraph tests (4 bridge tests with detailed parity analysis)
12. 🔄 Create golden test files (optional - can be done incrementally)
13. ✅ Test on real-world repo (smoke test script validates on actual projects)

## Architecture
- **Crates**: `cg-core` (lib), `cg-cli` (binary)
- **Parsing**: tree-sitter-typescript → extract nodes/edges (see docs/treesitter.md)
- **Storage**: Embedded Kuzu (Node table + Edge table)
- **Concurrency**: Parallel parse with rayon, single-threaded DB writes
- **Future**: Optional LSP integration for better accuracy (see docs/lsp.md)

## What We Can Do Now

The current implementation can:
- ✅ Ingest TypeScript/TSX projects with parallel processing
- ✅ Extract: Functions, Classes, Interfaces, Imports
- ✅ Create edges: Contains, Calls (within file), Implements/Extends
- ✅ Handle re-ingestion safely with proper cleanup
- ✅ Parse code with quotes/special characters (backslash escaping)
- ✅ **Query with Cypher** (`cg query`)
- ✅ **Find symbols by pattern** (`cg find symbol`)
- ✅ **Find callers** (`cg find callers`)
- ✅ **JSON output** for programmatic use
- ✅ **12 CLI smoke tests** covering core workflows

**Example:**
```bash
# Ingest a project
cg ingest --project ./my-ts-project --db ./graph.db --clean

# Find symbols
cg find symbol "Person" --db ./graph.db --limit 10

# Execute raw queries
cg query "MATCH (n:Node) WHERE n.node_type = 'Class' RETURN n.name" --db ./graph.db --json

# Find who calls a function
cg find callers "getUser" --db ./graph.db
```

## What's Still Missing

1. **Cross-file call edges** - Calls only tracked within same file (needs second pass)
2. **More node types** - DataModel, Var, Endpoint, Request, Page defined but not extracted
3. **File-to-file Import edges** - Import statements counted but not linked
4. **Constructor extraction** - `new ClassName()` not yet tracked
5. **LSP integration** - Only syntax-level analysis (60-70% accuracy)

## Future: Incremental Ingestion (Phase 4)
- Store last commit hash in metadata
- Use `git diff` to find changed files
- Re-ingest only touched files
- 100x faster for small changes

## Legacy Test Porting Plan
- **Source suites to mine** (from the original `~/code/stakgraph` repo):
  - `ast/src/testing/typescript/mod.rs` (and `ast/src/testing/typescript/**`) – exports node/edge expectations for the TypeScript fixtures.
  - `ast/src/testing/react/mod.rs` – React/JSX scenarios that map cleanly to our current parser.
  - `standalone/tests/ts_tests.rs` and the TypeScript portions of `standalone/tests/fulltest.rs` – high-level assertions over node/edge counts.
- **Harvest ground truth**
  1. In the legacy repo, run the relevant tests (e.g., `cargo test -p ast testing::typescript`, `cargo test -p standalone ts_tests`) with temporary code that dumps node/edge snapshots to JSON (symbol names, counts, key relationships such as `createPerson` handlers).
  2. Save those JSON artifacts into this repo under `fixtures/legacy_snapshots/`.
- **Recreate fixtures**
  - Copy only the TypeScript/React source trees into `fixtures/legacy/typescript` and `fixtures/legacy/react`, removing the old `Lang`/`Graph` harness so they are plain TS projects.
  - Normalize paths (forward slashes) to make snapshot comparisons stable.
- **Bridge tests in this repo**
  - Add integration tests (e.g., `tests/legacy_port.rs`) that ingest each fixture via `cg_core::ingest`, then query the Kuzu DB for:
    - Counts per `NodeType` (`Function`, `Class`, `Import`...), using the snapshots as expected values.
    - Presence of notable symbols (`createPerson`, `UserInterface`, etc.) and their file paths.
    - Edge counts (`Contains`, `Imports`, later `Calls`) as those features land.
  - Assert equality with the stored JSON to guarantee parity.
- **Iterate**
  - Start with constructs our parser already extracts (functions/classes/imports). Regenerate snapshots and extend tests as we add calls, data models, endpoints, and incremental-ingest logic.
  - Provide a helper script (`scripts/export_legacy_snapshots.sh`) to rerun the legacy export when fixtures change.
