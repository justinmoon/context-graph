# Context Graph - Rust/Kuzu Implementation Plan

## Current Status
✅ Basic workspace, DB layer, parser (functions/classes/interfaces/imports), ingestion pipeline, CLI
⚠️ **Critical bugs blocking production use - see below**

## Critical Bugs (Fix First)
1. **SQL Injection Risk** - Using `format!()` for queries breaks on quotes in code (e.g., `"it's"`)
2. **No Re-ingest** - Duplicate nodes cause PK violations without `--clean`
3. **ThreadPool Panic** - `build_global()` called every ingest, crashes on second run
4. **No Edges** - Parser never extracts Calls edges (graph is node-only)

## Immediate Roadmap

### Phase 1: Fix Critical Bugs
1. Fix ThreadPoolBuilder (use default pool or lazy_static)
2. Add upsert/MERGE logic or delete-before-insert
3. Fix SQL escaping (proper Kuzu escaping or parameters)
4. Extract Calls edges in parser

### Phase 2: Core Features
5. Implement query/find CLI commands (currently stubbed)
6. Add DataModel, Var, Endpoint, Request, Page node types
7. Remove legacy fixture mod.rs files (reference old APIs)

### Phase 3: Testing
8. Add CLI smoke tests with assert_cmd
9. Create golden test files (input TS → expected JSON)
10. Test on real-world repo

## Architecture
- **Crates**: `cg-core` (lib), `cg-cli` (binary)
- **Parsing**: tree-sitter-typescript → extract nodes/edges
- **Storage**: Embedded Kuzu (Node table + Edge table)
- **Concurrency**: Parallel parse, single-threaded DB writes

## Step 2 (Later): Incremental Ingestion
- Store last commit hash in metadata
- Use `git diff` to find changed files
- Re-ingest only touched files
