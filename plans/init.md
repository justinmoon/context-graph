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
