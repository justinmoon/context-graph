# Incremental Ingestion Validation Harness

A testing framework that validates incremental ingestion produces identical results to full ingestion across commit histories.

## Quick Start

```bash
# Test recent commits in current repository
./scripts/validate_incremental.py --commits HEAD~2,HEAD~1,HEAD

# Test specific commits
./scripts/validate_incremental.py --commits abc123,def456,789ghi

# Use custom workspace (preserves artifacts on failure)
./scripts/validate_incremental.py --commits HEAD~5,HEAD \
  --workspace ./validation_artifacts
```

## What It Does

The validation harness:

1. **Dual-Track Ingestion** - For each commit:
   - Runs full ingestion (`--clean`) into `db_full.db`
   - Runs incremental ingestion (`--incremental`) into `db_incremental.db`

2. **Snapshot Export** - Exports database contents to JSON:
   - `nodes.json`: All nodes (id, type, name, file) sorted by ID
   - `edges.json`: All edges (from_id, edge_type, to_id) sorted

3. **Comparison** - Diffs the snapshots:
   - ✅ **PASS**: Snapshots match exactly
   - ❌ **FAIL**: Differences found (artifacts preserved for inspection)

4. **Metrics Collection** - Tracks performance:
   - Files processed, symbols created, edges created
   - Ingestion duration, files/second throughput
   - Speedup ratio (full vs incremental)

## Output

### Success Example
```
================================================================================
Validating commit: HEAD
================================================================================

[1/4] Running FULL ingestion...
  ✓ Processed 12 files in 0.15s
[2/4] Exporting FULL snapshot...
  ✓ Exported to /tmp/cg_validate/snapshots/abc123/full
[3/4] Running INCREMENTAL ingestion...
  ✓ Processed 2 files in 0.06s
  ⚡ Speedup: 2.5x
[4/4] Exporting INCREMENTAL snapshot...
  ✓ Exported to /tmp/cg_validate/snapshots/abc123/incremental

[DIFF] Comparing snapshots...
  ✅ PASS: Snapshots match perfectly!
```

### Failure Example
```
[DIFF] Comparing snapshots...
  ❌ FAIL: Snapshots differ!
     - Edges differ

❌ Validation FAILED at commit abc123

Artifacts preserved in: /tmp/cg_validate/snapshots

Edge diff:
--- /tmp/cg_validate/snapshots/abc123/full/edges.json
+++ /tmp/cg_validate/snapshots/abc123/incremental/edges.json
@@ -10,7 +10,6 @@
-    ["file_a_id", "Imports", "file_b_id"]
```

## Usage Scenarios

### 1. Development Testing

Test your changes don't break incremental ingestion:

```bash
# Before changes
git checkout main
cargo build --release

# Make your changes
git checkout feature-branch

# Validate across recent commits
./scripts/validate_incremental.py --commits HEAD~5,HEAD~3,HEAD~1,HEAD
```

### 2. Regression Testing

Validate specific problematic scenarios:

```bash
# Renames
./scripts/validate_incremental.py \
  --commits before_rename,after_rename

# Branch switches
./scripts/validate_incremental.py \
  --commits main,feature-branch,main

# Deletions
./scripts/validate_incremental.py \
  --commits before_delete,after_delete
```

### 3. Performance Benchmarking

Compare full vs incremental performance:

```bash
./scripts/validate_incremental.py \
  --commits HEAD~10,HEAD~5,HEAD \
  --workspace ./benchmarks
  
# Check metrics in output for speedup ratios
```

## Command-Line Options

```
usage: validate_incremental.py [-h] [--repo REPO] --commits COMMITS 
                                [--cg-binary CG_BINARY] [--workspace WORKSPACE]

Validate incremental ingestion against full ingestion

options:
  -h, --help            show this help message and exit
  --repo REPO           Path to git repository to test (default: .)
  --commits COMMITS     Comma-separated list of commits to test (e.g., HEAD~5,HEAD~3,HEAD)
  --cg-binary CG_BINARY Path to cg binary (default: ./target/release/cg)
  --workspace WORKSPACE Workspace directory for databases and snapshots (default: temp dir)
```

## Exit Codes

- `0`: All validations passed
- `1`: At least one validation failed

## Artifacts

On failure, artifacts are preserved in the workspace:

```
workspace/
├── db_full.db/           # Full ingestion database
├── db_incremental.db/    # Incremental ingestion database
└── snapshots/
    ├── abc123/
    │   ├── full/
    │   │   ├── nodes.json
    │   │   └── edges.json
    │   └── incremental/
    │       ├── nodes.json
    │       └── edges.json
    └── def456/
        └── ...
```

You can inspect these files to understand the differences.

## CI Integration

### GitHub Actions Example

```yaml
name: Incremental Ingestion Validation

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 10  # Need history for commit testing
      
      - name: Build cg
        run: cargo build --release
      
      - name: Validate incremental ingestion
        run: |
          ./scripts/validate_incremental.py \
            --commits HEAD~3,HEAD~1,HEAD
```

### Quick Check (3 commits)

For fast CI feedback, test a small set:

```bash
# Takes ~10-30 seconds depending on repo size
./scripts/validate_incremental.py --commits HEAD~2,HEAD~1,HEAD
```

### Nightly Full Validation

For comprehensive testing, run nightly with more commits:

```bash
# Test 10 commits across recent history
./scripts/validate_incremental.py --commits \
  HEAD~20,HEAD~18,HEAD~15,HEAD~12,HEAD~10,\
  HEAD~8,HEAD~5,HEAD~3,HEAD~1,HEAD
```

## Troubleshooting

### "No JSON array found in query output"

Ensure `cg` binary is built in release mode:

```bash
cargo build --release
```

### "git: unknown revision"

Commit doesn't exist. Check commit hashes:

```bash
git log --oneline -10
```

### Tests pass locally but fail in CI

CI might have shallow checkout. Increase fetch depth:

```yaml
- uses: actions/checkout@v3
  with:
    fetch-depth: 20  # Fetch more history
```

### Snapshots differ unexpectedly

Check the preserved artifacts:

```bash
# Manually diff the snapshots
diff -u workspace/snapshots/abc123/full/nodes.json \
        workspace/snapshots/abc123/incremental/nodes.json
```

## Implementation Details

### Snapshot Format

**Nodes** (`nodes.json`):
```json
[
  ["node_id", "Function", "myFunction", "/path/to/file.ts"],
  ["node_id2", "Class", "MyClass", "/path/to/file.ts"]
]
```

**Edges** (`edges.json`):
```json
[
  ["from_node_id", "Calls", "to_node_id"],
  ["file_id", "Contains", "function_id"]
]
```

### Environment Variables

The script sets:
- `NO_COLOR=1`: Disables ANSI color codes in output
- `RUST_LOG=error`: Suppresses info/debug logs

This ensures clean JSON output for parsing.

### Database Isolation

Each mode uses a separate database:
- `db_full.db`: Full ingestion (always `--clean`)
- `db_incremental.db`: Incremental ingestion (starts with first commit, then `--incremental`)

This prevents cross-contamination and allows fair comparison.

## Known Limitations

1. **Requires git repository**: Non-git projects can't be validated
2. **Commit history dependency**: Commits must exist and be reachable
3. **File system dependency**: Path canonicalization may differ across systems (Windows vs Unix)
4. **Performance overhead**: Running both full and incremental doubles ingestion time

## Future Enhancements

Potential improvements:

1. **Parallel execution**: Run full and incremental in parallel
2. **Diff visualization**: Generate HTML reports of differences
3. **Historical trending**: Track metrics over time
4. **Random commit sampling**: Automatically select representative commits
5. **Cross-repository testing**: Test against multiple TypeScript repos
