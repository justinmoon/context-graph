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

## Incremental Ingestion Validation Harness ✅ IMPLEMENTED
A production-ready regression framework comparing incremental vs full ingest on real commit histories.

### Implementation (`scripts/validate_incremental.py`)
Fully functional Python driver with all specified features:

1. **Dual-track ingest per commit** ✅
   - Full baseline: `cg ingest --clean` into `db_full.db`
   - Incremental candidate: `cg ingest --incremental` into `db_incremental.db`
   - Per-commit snapshot directories for diffing

2. **Deterministic snapshot export** ✅
   - Nodes: `MATCH (n:Node) RETURN n.id, n.node_type, n.name, n.file ORDER BY n.id`
   - Edges: `MATCH (a)-[e:EDGE]->(b) RETURN a.id, e.edge_type, b.id ORDER BY ...`
   - JSON output with `NO_COLOR=1` and `RUST_LOG=error` for clean parsing

3. **Diff & metrics collection** ✅
   - Byte-for-byte snapshot comparison
   - Performance metrics: files/sec, speedup ratio, duration
   - Detailed failure reporting with preserved artifacts

4. **Artifact preservation** ✅
   - On failure: snapshots saved to workspace for manual inspection
   - Diff output included in error messages

5. **Safety features** ✅
   - Dirty worktree detection prevents data loss
   - Automatic restoration of original branch/commit

6. **Easy invocation** ✅
   - `just validate-incremental` (quick: 3 commits)
   - `just validate-incremental-full` (extended: 6 commits)
   - Custom: `./scripts/validate_incremental.py --commits <list>`

### Documentation
See [docs/incremental-validation.md](./incremental-validation.md) for:
- Quick start guide and usage examples
- CI integration patterns (GitHub Actions)
- Troubleshooting and known limitations
- Output format specification

### Usage in Development
```bash
# Quick regression check (recommended before pushing)
just validate-incremental

# After major changes to incremental logic
just validate-incremental-full

# Test specific scenario (e.g., renames, deletions)
./scripts/validate_incremental.py --commits before,after
```

### CI Integration (Future)
- **Quick check**: 3 commits in pull request workflow (~30s)
- **Nightly**: 10+ commits across recent history for comprehensive validation
- **On-demand**: Full validation for release candidates

### Ongoing Work
- Curate external repos (microsoft/TypeScript, vercel/next.js) for cross-project validation
- Add CI workflow with trimmed commit set
- Track metrics over time for performance regression detection

## Additional Testing Enhancements
- Generate “golden” JSON outputs for the TypeScript fixtures and assert exact datasets (beyond counts).
- Extend graph feature tests as new node/edge types are implemented (DataModel, Endpoint, etc.).
- Consider property-based tests for parser extractors (randomized AST snippets).
- Monitor `scripts/test_real_repo.sh` outputs in CI and alert on regressions (e.g., ingestion > threshold).

## Next Steps
1. ~~Build the incremental validation harness~~ ✅ COMPLETE
2. ~~Publish tooling/documentation~~ ✅ COMPLETE (`docs/incremental-validation.md`, just targets)
3. Add CI workflow using the validation harness (trimmed commit set for fast feedback)
4. Curate external TypeScript repos for broader validation coverage
5. Expand test coverage as new features land (cross-file calls, additional node types, LSP integration)
