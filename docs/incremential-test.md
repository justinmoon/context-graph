• Here’s how I’d stress-test the branch-switching pipeline and prove incremental ingest
  is trustworthy:

  ———

  1. Pick Real Repos + Commit Walks

  - Grab a few sizable TypeScript projects with non-trivial histories (e.g., microsoft/
    TypeScript, vercel/next.js, supabase/supabase).
  - For each repo define commit sequences that exercise different patterns: hotfix branches
    (main → feature → main), file renames, mass deletions, merges with conflicts, rebases
    (force-push), etc.

  2. Dual-Track Ingestion Harness

  For every commit in each sequence:

  1. Full baseline
      - Checkout commit C.
      - Switch to a fresh temp DB (full.db).
      - Run cg ingest --clean --project repo --db full.db.
      - Export a canonical snapshot (e.g., using cg query --json "MATCH ... ORDER BY"
        stored as sorted JSON).
  2. Incremental candidate
      - Maintain a persistent DB (inc.db) that was initially populated by a full ingest on
        the first commit.
      - Checkout commit C.
      - Run cg ingest --incremental --project repo --db inc.db.
      - Export the same canonical snapshot from inc.db.
  3. Compare outputs
      - Use a deterministic diff (e.g., jq sort + diff, or compute checksums/hashes per
        node/edge).
      - Record mismatches by node/edge counts and per-entity diffs.

  A simple script (Python/Rust/Bash) can drive this loop: keep a prepared list of commits,
  iterate, run the ingest commands, capture timings, and diff snapshots. Log failures with
  enough context to replay.

  3. Scenario Coverage

  Make sure the commit list hits:

  - Pure modifications (same files edited).
  - Deletions (git rm, file removal).
  - Renames (git mv, renames with content changes).
  - Branch switches (checkout A → B → A) to ensure metadata rolls forward/backward
    correctly.
  - History rewrites: stash the DB with a stored commit, then reset the repo to a point
    that doesn’t contain it (force push scenario) and confirm the fallback path runs.
  - Mixed merges that touch large file sets.

  4. Canonical Snapshot Format

  Use an export that’s easy to compare:

  cg query "MATCH (n:Node) RETURN n.id, n.node_type, n.name, n.file ORDER BY n.id" --json
  > nodes.json
  cg query "MATCH (a)-[e:Edge]->(b) RETURN a.id, e.edge_type, b.id ORDER BY a.id,
  e.edge_type, b.id" --json > edges.json

  Normalize (sort keys, remove metadata) and diff.

  5. Automate & Report

  - Wrap in a script that for each commit pair prints: files processed, time spent,
    mismatch counts.
  - Fail fast on diffs, but also keep artifacts for inspection.
  - Optionally run in CI with a subset of commits to catch regressions early.

  6. Bonus Checks

  - Monitor performance metrics (files/sec, skip counts) to confirm expected gains.
  - Ensure metadata gets corrected when incremental falls back (e.g., after a failure, the
    next run should succeed).

  This framework gives strong evidence our incremental indexing behaves like a full rebuild
  across real-world histories. Once we’re confident, we can add a trimmed version (maybe a
  couple commits from a medium repo) into CI as a regression guard.

