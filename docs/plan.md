# Context Graph Testing Plan

## Goals
- Guarantee the TypeScript ingestion pipeline (parsing, graph construction, incremental updates) behaves deterministically across real projects.
- Catch regressions quickly with layered automated suites.
- Prove incremental ingestion matches a clean rebuild when branches/commits change.

## Test Suites (Current & Maintained)
- **Unit tests (`cg-core`)** – parser extraction, database helpers, incremental git utilities.
- **Graph feature tests** – constructor call edges, file-to-file import edges, incremental preservation of dependencies.
- **CLI smoke tests (`cg-cli`)** – covers ingest/query/find workflows, error handling, special characters, JSON output.
- **Legacy parity tests** – ingest the legacy fixtures, compare node/edge counts and notable symbols against JSON snapshots.
- **Incremental unit tests** – git-backed temp repos validating commit storage, modified-file reprocessing, fallback on invalid commits.
- **Real-world smoke test (`scripts/test_real_repo.sh`)** – ingests this repository, reports metrics, validates query output.

## Remaining Risk Areas
- Cross-file call resolution (infrastructure ready, edges not emitted yet).
- LSP-driven node types (DataModel, Var, Endpoint, etc.) – planned future work.
- Large-history incremental correctness beyond unit coverage.

## Incremental Ingestion Validation Harness
A heavier-weight regression framework to compare incremental vs full ingest on real histories.

1. **Repository selection**
   - Curate several sizeable TypeScript repos (e.g., microsoft/TypeScript, vercel/next.js).
   - For each, define commit sequences: straight-line history, branch switch (A→B→A), renames, deletions, merges, rebases/force pushes.

2. **Dual-track ingest per commit**
   - **Full baseline**: checkout commit `C`, ingest with `cg ingest --clean`, export canonical snapshots (`nodes.json`, `edges.json`).
   - **Incremental candidate**: reuse persistent DB, run `cg ingest --incremental`, export the same snapshots.
   - Store snapshots in per-commit directories for diffing.

3. **Snapshot format**
   - `cg query "MATCH (n:Node) RETURN n.id, n.node_type, n.name, n.file ORDER BY n.id" --json` → `nodes.json`.
   - `cg query "MATCH (a)-[e:Edge]->(b) RETURN a.id, e.edge_type, b.id ORDER BY a.id, e.edge_type, b.id" --json` → `edges.json`.
   - Post-process with `jq --sort-keys` to ensure deterministic diffs.

4. **Diff & metrics**
   - Compare baseline vs incremental snapshots. Any difference fails the run and logs the offending commit.
   - Collect metrics (files processed, ingest duration, files/sec, symbols/file, edges/symbol) to monitor performance.

5. **Automation**
   - Implement a driver script (Rust or Python). Inputs: repo URL, list of commits/branches, temp workspace.
   - Flow: clone → iterate commits → run full/incremental ingests → diff → record stats.
   - On failure, preserve artifacts for manual inspection.

6. **CI integration**
   - Add a trimmed commit set to CI (e.g., 3 commits per repo) for quick regression detection.
   - Keep full suite as an optional nightly/weekly job.

## Additional Testing Enhancements
- Generate “golden” JSON outputs for the TypeScript fixtures and assert exact datasets (beyond counts).
- Extend graph feature tests as new node/edge types are implemented (DataModel, Endpoint, etc.).
- Consider property-based tests for parser extractors (randomized AST snippets).
- Monitor `scripts/test_real_repo.sh` outputs in CI and alert on regressions (e.g., ingestion > threshold).

## Next Steps
1. Build the incremental validation harness using the specification above.
2. Publish tooling/documentation so contributors can run the suite locally (make target).
3. Expand coverage as new features land (cross-file calls, additional node types, LSP integration).
