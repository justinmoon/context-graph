# Minimal Stakgraph Reboot Plan (Rust Edition)

## Context
- Goal: build a fresh, lightweight tool that ingests TypeScript code into a Kuzu graph with a simple CLI for querying.
- Constraints: do **not** import existing stakgraph runtime code; we can borrow concepts only.
- We commit to implementing the core in **Rust** to leverage fast parsers and deliver a single, portable binary.
- Scope now: TypeScript source exclusively; Rust or other languages can follow later.
- Execution roadmap:  
  1. Basic ingest + query CLI.  
  2. Branch-aware incremental ingestion.

## Guiding Principles
- Keep the binary small and self-contained (no external DB daemon; Kuzu runs embedded).
- Lean on battle-tested crates: `tree-sitter-typescript` for parsing, `kuzu` for storage, `clap` for CLI, `rayon` for parallelism.
- Design ingestion to be idempotent and schema-simple so we can iterate quickly.
- Optimize for large repos by streaming work and batching writes.

---

## Step 1 – Basic Ingest & Query CLI

### Deliverable
A Rust CLI (`stakgraph`) that can:
1. Ingest the current TypeScript workspace into an embedded Kuzu database (`stakgraph ingest`).
2. Execute canned or ad-hoc queries against that graph (`stakgraph query`, `stakgraph find`).

### High-Level Architecture
- **Crate layout**
  - `crates/stakgraph-cli/` – CLI entry (`main.rs`) built with `clap`.
  - `crates/stakgraph-core/` – Library exposing ingestion pipeline and query helpers.
  - `crates/stakgraph-parser/` – Tree-sitter utilities + symbol extraction (optional split; can be module inside core for simplicity).
  - `fixtures/` – Small TypeScript sample repos for integration tests.

- **Core modules**
  - `fs::project`: repo root detection (`git rev-parse` fallback to cwd), file enumeration respecting `tsconfig.json`.
  - `parser::typescript`: wraps `tree-sitter` to extract symbols (`Function`, `Class`, `Interface`, `Enum`, `Import`, `Export`, `CallExpr`).
  - `model`: struct definitions for `FileNode`, `SymbolNode`, `Edge` with serialization.
  - `db::kuzu`: connection manager, schema bootstrap, prepared statements & transactions.
  - `ingest`: orchestrates file parsing + database writes.
  - `query`: utilities for canned traversals (symbol lookup, caller/callee relationships).

- **Kuzu schema (first cut)**
  ```sql
  CREATE NODE TABLE files (
    id STRING PRIMARY KEY,
    path STRING,
    hash STRING,
    mtime UINT64,
    size UINT64
  );

  CREATE NODE TABLE symbols (
    id STRING PRIMARY KEY,
    file_id STRING,
    name STRING,
    kind STRING,
    signature STRING,
    start_line UINT32,
    end_line UINT32,
    export BOOLEAN,
    FOREIGN KEY (file_id) REFERENCES files(id)
  );

  CREATE REL TABLE contains (FROM files TO symbols);
  CREATE REL TABLE calls (FROM symbols TO symbols);
  CREATE REL TABLE imports (FROM symbols TO symbols);

  CREATE NODE TABLE metadata (
    key STRING PRIMARY KEY,
    value STRING
  );
  ```
  - Generate deterministic IDs via hashing (`blake3(path)` for files, `blake3(path + name + span)` for symbols).
  - Keep schema minimal; optional columns (docs, modifiers) can come later.

- **Ingestion pipeline**
  1. Initialize Kuzu (create DB directory if missing; run schema migrations with version stamp).
  2. Discover `.ts/.tsx` files (respect `.gitignore` using `ignore` crate).
  3. Parse files in parallel (`rayon`) to build per-file `ParsedFile { file, symbols, edges }`.
  4. Batch database writes:
     - Upsert file node.
     - Replace associated symbols/edges (Step 1 can `DELETE` + `INSERT` to keep logic simple).
     - Use transactions per file batch for ACID safety.
  5. Record metadata (`last_ingest_ts`, `total_files`, etc.) though commit tracking waits for Step 2.

- **CLI commands**
  - `stakgraph ingest [--db <path>] [--project <path>] [--threads N] [--clean]`.
  - `stakgraph query "<sql>" [--db <path>] [--json]`.
  - `stakgraph find symbol <pattern> [--limit N]`: runs parameterized SQL (`ILIKE` on `symbols.name`) and prints file + line ranges.
  - `stakgraph find callers <symbol-id|name>`: resolves symbol then traverses `calls` rel table.

- **Diagnostics & Logging**
  - Use `tracing` + `tracing-subscriber` for logs.
  - Display ingest summary (files processed, new symbols) at end.

### Tasks
1. **Bootstrap workspace**
   - Initialize Cargo workspace (`stakgraph-cli`, `stakgraph-core`).
   - Configure `rust-toolchain.toml`, Clippy + fmt settings.
2. **DB layer**
   - Add `kuzu` crate (ensure compatible version) and implement connection wrapper with lazy init.
   - Write schema migration executed on startup (version stored in `metadata`).
3. **File discovery**
   - Implement repo root detection via `git2` or shell-out to `git`.
   - Use `ignore` crate to walk TypeScript files respecting `.gitignore`.
   - Parse optional `tsconfig.json` (via `serde_json`) to refine include/exclude.
4. **Parser/extractor**
   - Set up `tree-sitter-typescript` (both TSX & regular).
   - Build visitors for functions, classes, methods, imports/exports, call expressions.
   - Produce `SymbolNode` + `Edge` (calls/imports) with line info.
5. **Ingestion logic**
   - Implement single-run pipeline that clears existing graph when `--clean`.
   - Insert files + symbols with prepared statements; ensure transaction boundaries.
6. **CLI implementation**
   - Integrate commands using `clap` derive.
   - Provide table or JSON output via `serde_json` and `comfy-table`.
7. **Testing**
   - Add `fixtures/basic-ts` sample project.
   - Write cargo test to run ingest on fixture, then query for known symbols and assert results.
   - Add unit tests for parser extraction.

### Validation
- Smoke test on `fixtures/basic-ts`.
- Run on a medium real-world repo to gauge ingestion time (< a few seconds for ~100 files).
- Document usage in `README.md`.

---

## Step 2 – Branch Switching & Incremental Ingestion

### Goals
- Avoid full rebuild on branch change by leveraging git metadata and file hashes.
- Support efficient updates when files are added/modified/removed/renamed.

### Strategy
1. Extend schema:
   - Add `metadata` rows for `last_ingested_commit`, `ingestion_version`.
   - Ensure `files` table stores `hash` + `mtime`.
2. In `stakgraph ingest` default mode:
   - Detect current commit (`git rev-parse HEAD`).
   - If metadata commit differs:
     - Compute `git diff --name-status <last_ingested_commit> HEAD`.
     - Partition files into `Added`, `Modified`, `Deleted`, `Renamed`.
3. Update logic:
   - For `Added`/`Modified`: parse file, remove prior `symbols` + `edges` for that file, insert new data.
   - For `Deleted`: delete `files` row cascade (`files` → `symbols` → `edges`).
   - For `Renamed`: update `files.path`. If hash unchanged, reuse symbol IDs; else treat as modify.
4. Detect non-linear history:
   - If diff fails (commit missing, `git diff` error) or `--full` flag set, fall back to clean rebuild.
5. Track ingestion version to invalidate caches when schema/parsers change.
6. CLI enhancements:
   - `--full` (force clean).
   - `--since <commit>` to ingest relative to arbitrary base.
   - `stakgraph status` command summarizing DB commit vs working tree (optional nice-to-have).

### Implementation Tasks
1. **Metadata management**
   - Create helper to read/write metadata rows atomically.
2. **Git utilities**
   - Wrap `git2` crate for diff computations (avoid shelling out).
   - Handle worktree states (dirty tree) by comparing against index + working dir (optionally warn and default to full ingest).
3. **Selective updates**
   - Implement per-file transaction to delete + insert new records.
   - For imports/calls referencing other files, only rebuild edges originating from the changed file (inbound edges handled when source file changes).
4. **Rename detection**
   - Use `git diff --name-status` rename info or compute via matching hashes.
5. **Testing**
   - Add integration tests simulating commit history (use temporary git repo in tests).
   - Cover add/modify/delete/rename cases.
   - Ensure metadata commit updates correctly.

### Validation
- Benchmark incremental ingest on repo with small change (should be ~constant-time relative to number of touched files).
- Verify correctness by comparing graph snapshots before/after change (unit tests or diff queries).

---

## Future Enhancements (Out of Scope Now)
- Expand parser to capture type relationships, interface implementations, etc.
- Add language plugins (Rust, Go) once TypeScript pipeline matures.
- Build richer query DSL or integrate embedding/vector search.
- Offer daemon/server mode for continuous indexing.

---

## Open Questions
1. Confirm `kuzu` Rust crate stability for embedded use (check version, ensure async/blocking fits CLI).
2. Decide how to package the binary (static linking on macOS/Linux, cross-compilation strategy).
3. Determine level of call graph fidelity needed initially (intra-file vs cross-file by module resolution).
4. Plan for schema migrations and version upgrades (store migration level in metadata).

---

## Testing Strategy - Path to Feature Parity

We will replicate the original stakgraph's test suite to validate our implementation and ensure feature parity. This gives us clear success criteria and validates our parser extracts equivalent information.

### Phase 1: Replicate Inline TypeScript/React Fixtures
**Goal:** Match extraction logic for basic TypeScript and React patterns.

1. **Copy fixtures from original stakgraph**
   - Source: `~/code/stakgraph/ast/src/testing/typescript/` 
   - Source: `~/code/stakgraph/ast/src/testing/react/`
   - Destination: `fixtures/typescript/` and `fixtures/react/` in our repo
   
2. **Analyze their test assertions**
   - Study what NodeTypes they extract (File, Function, Class, Interface, DataModel, Import, Var, etc.)
   - Study what EdgeTypes they create (Contains, Calls, Imports, Renders, Handler, Of, Operand)
   - Document the expected graph structure
   
3. **Implement parser to match their extraction**
   - Start with basic nodes: File, Function, Class, Interface
   - Add relationship detection: Contains, Calls, Imports
   - Iterate to add more node types and edge types
   
4. **Write integration tests**
   - Test TypeScript fixture ingestion
   - Test React fixture ingestion
   - Assert on node counts, types, and relationships
   - Validate against their expected outputs

### Phase 2: Full demorepo Replication
**Goal:** Pass the comprehensive fulltest.rs validation.

5. **Use fayekelmith/demorepo as fixture**
   - Clone `https://github.com/fayekelmith/demorepo.git` at commit `778b5202fca04a2cd5daed377c0063e9af52b24c`
   - This is a Go backend + React frontend full-stack app
   
6. **Replicate fulltest.rs assertions**
   - Port their comprehensive test from `~/code/stakgraph/standalone/tests/fulltest.rs`
   - Validate ~140+ nodes across multiple types
   - Validate edge relationships (function calls, endpoint handlers, page renders, request-to-endpoint links)
   - Expected: 2 languages, 22 files, 23 libraries, 10 imports, 3 endpoints, 2 requests, 2 pages, etc.
   
7. **Focus on TypeScript/React subset first**
   - Start by validating just the React frontend portion
   - Then add Go backend validation once we add Go support
   - Ensures we can pass their TypeScript tests before expanding to other languages

### Phase 3: Multi-Repo and Advanced Features  
**Goal:** Support real-world production repos and advanced scenarios.

8. **Multi-repo ingestion**
   - Test on sphinx-tribes (Go) + sphinx-tribes-frontend (React)
   - Validate cross-repo request-to-endpoint linking
   
9. **Incremental ingestion (Step 2)**
   - Replicate their graph_accuracy.rs test
   - Validate updates between commits
   - Test add/modify/delete/rename scenarios
   
10. **LSP integration** (optional, later)
    - Add language server protocol support for better cross-file linking
    - Validate with `USE_LSP=1` test variants

### Success Metrics
- ✅ Pass all TypeScript fixture tests
- ✅ Pass all React fixture tests  
- ✅ Pass fulltest.rs validation on demorepo (React portion)
- ✅ Match node counts and edge counts from original stakgraph
- ✅ Validate specific relationships (e.g., "POST request calls POST /person endpoint")

### Immediate Next Actions (Updated)
1. ✅ Set up the new Cargo workspace with CLI + core crates (DONE)
2. Copy TypeScript and React fixtures from original stakgraph
3. Study and document their expected node/edge extractions
4. Implement Kuzu connection + schema bootstrap
5. Build the TypeScript parser module to match their extraction logic
6. Write integration tests validating against their fixtures
7. Iterate until we pass all Phase 1 tests
8. Move to Phase 2 (demorepo) once Phase 1 passes
9. Implement incremental ingestion (Step 2) once basic ingestion is solid

