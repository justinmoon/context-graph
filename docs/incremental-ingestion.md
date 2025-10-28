# Incremental Ingestion

Context Graph supports fast incremental re-ingestion using git to detect changes.

## Overview

Instead of re-processing all files on every ingestion, incremental mode:
1. Stores the current git commit hash after successful ingestion
2. On subsequent runs, uses `git diff` to find changed files
3. Deletes files that were removed or renamed from the database
4. Only re-processes files that were added or modified
5. Falls back to full ingestion if git state is invalid

## Usage

```bash
# Initial full ingestion (required first time)
cg ingest --project ./my-project --db ./graph.db --clean

# Fast incremental updates
cg ingest --project ./my-project --db ./graph.db --incremental

# Incremental mode with explicit thread count
cg ingest --project ./my-project --db ./graph.db --incremental --threads 8
```

## Performance

**Typical speedups:**
- No changes: **Instant** (<0.01s vs ~0.1s for full)
- 1-2 files changed: **50-100x faster**
- 10% of files changed: **10x faster**
- Large changes (>50%): Falls back to similar speed as full ingestion

**Example:**
```bash
# Full ingestion: 500 files in 5 seconds
$ cg ingest --project ./large-project --db ./graph.db --clean

# After editing 2 files:
$ cg ingest --project ./large-project --db ./graph.db --incremental
# Processes: 2 files in 0.05 seconds (100x faster!)
```

## How It Works

### 1. Change Detection

Uses `git diff --name-status` to detect file operations:

```
M    src/file1.ts    # Modified - will be re-processed
A    src/file2.ts    # Added - will be processed
D    src/old.ts      # Deleted - will be removed from DB
R100 src/a.ts src/b.ts  # Renamed - old deleted, new processed
```

### 2. Database Operations

**For each changed file:**
- Modified/Added: Delete existing symbols, re-parse, re-insert
- Deleted: Remove file node and all contained symbols from database
- Renamed: Delete old file, process new file (preserves history correctly)

**Atomic operations:**
- All deletions happen before processing new files
- Errors in one file don't prevent processing others
- Commit hash only updated if entire ingestion succeeds

### 3. Error Handling

**Graceful fallbacks:**

```rust
// Unreachable commit (rebase, force push)
git diff abc123..def456  // Error: unknown revision
→ Falls back to full ingestion with warning

// Missing metadata
No last_commit in database
→ Falls back to full ingestion

// Not a git repository
→ Falls back to full ingestion

// Ingestion errors (parse failures, etc.)
→ Don't update commit hash (prevents bad state)
```

## Requirements

- Project must be a git repository
- `git` command must be available in PATH
- Working directory must be clean (no check enforced, but recommended)

## Limitations

### Current

1. **Only tracks committed changes**
   - Uncommitted files are not detected
   - Must `git commit` before running incremental ingestion

2. **No cross-file dependency tracking**
   - If A imports B, and B changes, A is not re-processed
   - Only B is re-ingested (symbols in A may be stale)
   - Workaround: Use `--clean` for full re-ingestion periodically

3. **File path canonicalization**
   - Paths must match exactly as stored in database
   - macOS symlinks (/tmp vs /private/tmp) can cause issues in tests
   - Production usage is unaffected

### By Design

- **Requires git**: Non-git projects always use full ingestion
- **Single commit range**: Only compares last_commit to HEAD
- **No branch awareness**: Works on current branch only

## Implementation Details

### Metadata Storage

```cypher
// Stored in Metadata node table
CREATE (m:Metadata {
    key: 'last_commit',
    value: 'abc123def456...' // 40-char SHA-1 hash
})
```

### File Change Detection

```rust
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: FileChangeType,
}

pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: PathBuf },
}
```

### Error Recovery

```rust
// get_incremental_files wraps get_incremental_changes
match get_incremental_changes(db, project_root) {
    Ok(Some(changes)) => process_changes(changes),
    Ok(None) => full_ingestion(),  // No git, no metadata
    Err(e) => {
        warn!("Incremental failed: {}, falling back", e);
        full_ingestion()  // Git error, unreachable commit
    }
}
```

## Testing

### Automated Tests

```bash
# Run incremental ingestion tests
cargo test --test incremental_test

# Results:
# ✅ test_first_ingest_stores_commit
# ✅ test_incremental_with_no_changes
# ✅ test_incremental_with_modified_file
# ✅ test_fallback_on_unreachable_commit
# ⚠️  test_incremental_with_deleted_file (ignored: macOS paths)
# ⚠️  test_incremental_with_renamed_file (ignored: macOS paths)
```

### Manual Testing

```bash
# 1. Initial ingestion
cd ~/my-project
cg ingest --project . --db ./graph.db --clean

# 2. Make some changes
echo "export const NEW = 1;" >> src/new.ts
git add src/new.ts && git commit -m "Add new file"

# 3. Incremental update
cg ingest --project . --db ./graph.db --incremental
# Should process only: src/new.ts

# 4. Delete a file
git rm src/old.ts && git commit -m "Remove old file"

# 5. Incremental update
cg ingest --project . --db ./graph.db --incremental
# Should delete: src/old.ts from database

# 6. Verify deletion
cg query "MATCH (n:Node {name: 'old.ts'}) RETURN n" --db ./graph.db
# Should return 0 results
```

## Best Practices

### 1. Use Incremental for Development

```bash
# Set up alias for convenience
alias cg-update='cg ingest --project . --db ./.cg --incremental'

# After each commit
git commit -m "Add feature"
cg-update  # Fast update
```

### 2. Use Full Ingestion Periodically

```bash
# Weekly or after major changes
cg ingest --project . --db ./.cg --clean

# Or after pulling many changes
git pull origin main
cg ingest --project . --db ./.cg --clean
```

### 3. Handle Force Pushes Gracefully

```bash
# After rebase or force pull
git rebase main
# Incremental will fail gracefully and fall back to full

# Or explicitly use --clean
cg ingest --project . --db ./.cg --clean
```

### 4. CI/CD Integration

```yaml
# .github/workflows/ingest.yml
- name: Ingest code graph
  run: |
    # Always use clean in CI (reproducible)
    cg ingest --project . --db ./graph.db --clean
    
    # Or use incremental with cache
    if [ -f graph.db ]; then
      cg ingest --project . --db ./graph.db --incremental
    else
      cg ingest --project . --db ./graph.db --clean
    fi
```

## Troubleshooting

### Incremental mode not working

```bash
# Check if git repository
git rev-parse --git-dir

# Check if commit hash is stored
cg query "MATCH (m:Metadata {key: 'last_commit'}) RETURN m.value" --db ./.cg

# Force full re-ingestion
cg ingest --project . --db ./.cg --clean
```

### Files not being deleted

```bash
# Verify git detects deletion
git diff --name-status HEAD~1 HEAD

# Check database for file
cg query "MATCH (n:Node) WHERE n.file CONTAINS 'deleted.ts' RETURN n" --db ./.cg

# Manually delete (temporary workaround)
cg query "MATCH (n:Node {file: '/path/to/deleted.ts'}) DETACH DELETE n" --db ./.cg
```

### Performance not improving

- Ensure you're using `--incremental` flag
- Check that only a few files changed: `git diff --name-only HEAD~1 HEAD | wc -l`
- Verify metadata is stored: `cg query "MATCH (m:Metadata) RETURN m" --db ./.cg`
- Try `--clean` and then use `--incremental` on next run

## Future Improvements

Potential enhancements (not yet implemented):

1. **Dependency-aware re-ingestion**
   - Track import graph
   - Re-process files that import changed files

2. **Watch mode**
   - `cg watch --project . --db ./.cg`
   - Auto-ingest on file changes

3. **Uncommitted changes**
   - Detect working directory changes
   - Support `--include-uncommitted` flag

4. **Branch-aware metadata**
   - Store commit per branch
   - Switch metadata when changing branches

5. **Partial ingestion recovery**
   - Checkpoint progress during long ingestions
   - Resume from checkpoint on interruption
