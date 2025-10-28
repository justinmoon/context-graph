#!/usr/bin/env python3
"""
Incremental Ingestion Validation Harness

Validates that incremental ingestion produces identical results to full ingestion
by comparing snapshots across a commit history.

Usage:
    ./scripts/validate_incremental.py --repo <path> --commits <commit1,commit2,...>
    ./scripts/validate_incremental.py --repo . --commits HEAD~5,HEAD~3,HEAD
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import List, Dict, Any, Optional


@dataclass
class IngestMetrics:
    """Metrics from an ingestion run"""
    commit: str
    files_processed: int
    symbols_created: int
    edges_created: int
    duration_sec: float
    mode: str  # "full" or "incremental"


@dataclass
class ValidationResult:
    """Result of comparing two snapshots"""
    commit: str
    nodes_match: bool
    edges_match: bool
    node_diff: Optional[str]
    edge_diff: Optional[str]
    metrics_full: IngestMetrics
    metrics_incremental: IngestMetrics


class IncrementalValidator:
    """Validates incremental ingestion against full ingestion"""
    
    def __init__(self, repo_path: Path, cg_binary: Path, workspace: Path):
        self.repo_path = repo_path.resolve()
        self.cg_binary = cg_binary.resolve()
        self.workspace = workspace
        self.workspace.mkdir(parents=True, exist_ok=True)
        
    def run_git_command(self, args: List[str], cwd: Optional[Path] = None) -> str:
        """Run a git command and return output"""
        cmd = ["git"] + args
        result = subprocess.run(
            cmd,
            cwd=cwd or self.repo_path,
            capture_output=True,
            text=True,
            check=True
        )
        return result.stdout.strip()
    
    def checkout_commit(self, commit: str):
        """Checkout a specific commit"""
        self.run_git_command(["checkout", commit])
        
    def export_snapshot(self, db_path: Path, output_dir: Path) -> Dict[str, Path]:
        """Export nodes and edges to JSON files"""
        output_dir.mkdir(parents=True, exist_ok=True)
        
        # Export nodes
        nodes_query = 'MATCH (n:Node) RETURN n.id, n.node_type, n.name, n.file ORDER BY n.id'
        nodes_file = output_dir / "nodes.json"
        
        # Disable colored output by setting NO_COLOR env var
        env = os.environ.copy()
        env['NO_COLOR'] = '1'
        env['RUST_LOG'] = 'error'  # Suppress info logs
        
        result = subprocess.run(
            [str(self.cg_binary), "query", nodes_query, "--db", str(db_path), "--json"],
            capture_output=True,
            text=True,
            check=True,
            env=env
        )
        
        # Parse output - find JSON array (should be clean now with NO_COLOR)
        output = result.stdout.strip()
        
        # Try to find JSON start
        json_start = -1
        for i, char in enumerate(output):
            if char == '[':
                json_start = i
                break
        
        if json_start == -1:
            raise ValueError(f"No JSON array found in query output:\n{result.stdout}\n{result.stderr}")
        
        json_text = output[json_start:]
        
        # Parse, sort, and write nodes
        nodes_data = json.loads(json_text)
        with open(nodes_file, 'w') as f:
            json.dump(nodes_data, f, indent=2, sort_keys=True)
        
        # Export edges
        edges_query = 'MATCH (a)-[e:EDGE]->(b) RETURN a.id, e.edge_type, b.id ORDER BY a.id, e.edge_type, b.id'
        edges_file = output_dir / "edges.json"
        
        result = subprocess.run(
            [str(self.cg_binary), "query", edges_query, "--db", str(db_path), "--json"],
            capture_output=True,
            text=True,
            check=True,
            env=env
        )
        
        # Parse output - find JSON array
        output = result.stdout.strip()
        
        # Try to find JSON start
        json_start = -1
        for i, char in enumerate(output):
            if char == '[':
                json_start = i
                break
        
        if json_start == -1:
            raise ValueError(f"No JSON array found in query output:\n{result.stdout}\n{result.stderr}")
        
        json_text = output[json_start:]
        
        # Parse, sort, and write edges
        edges_data = json.loads(json_text)
        with open(edges_file, 'w') as f:
            json.dump(edges_data, f, indent=2, sort_keys=True)
        
        return {"nodes": nodes_file, "edges": edges_file}
    
    def run_ingest(self, db_path: Path, mode: str, commit: str) -> IngestMetrics:
        """Run ingestion and collect metrics"""
        import time
        
        args = [
            str(self.cg_binary),
            "ingest",
            "--project", str(self.repo_path),
            "--db", str(db_path)
        ]
        
        if mode == "full":
            args.append("--clean")
        elif mode == "incremental":
            args.append("--incremental")
        
        start_time = time.time()
        result = subprocess.run(args, capture_output=True, text=True, check=True)
        duration = time.time() - start_time
        
        # Parse output for metrics
        output = result.stdout + result.stderr
        files_processed = 0
        symbols_created = 0
        edges_created = 0
        
        for line in output.split('\n'):
            if 'Files processed:' in line:
                files_processed = int(line.split(':')[1].strip())
            elif 'Symbols created:' in line:
                symbols_created = int(line.split(':')[1].strip())
            elif 'Edges created:' in line:
                edges_created = int(line.split(':')[1].strip())
        
        return IngestMetrics(
            commit=commit,
            files_processed=files_processed,
            symbols_created=symbols_created,
            edges_created=edges_created,
            duration_sec=duration,
            mode=mode
        )
    
    def diff_snapshots(self, baseline: Dict[str, Path], candidate: Dict[str, Path]) -> tuple[bool, bool, Optional[str], Optional[str]]:
        """Compare two snapshots and return (nodes_match, edges_match, node_diff, edge_diff)"""
        # Compare nodes
        with open(baseline["nodes"]) as f:
            baseline_nodes = json.load(f)
        with open(candidate["nodes"]) as f:
            candidate_nodes = json.load(f)
        
        nodes_match = baseline_nodes == candidate_nodes
        node_diff = None
        if not nodes_match:
            node_diff = self._generate_diff(baseline["nodes"], candidate["nodes"])
        
        # Compare edges
        with open(baseline["edges"]) as f:
            baseline_edges = json.load(f)
        with open(candidate["edges"]) as f:
            candidate_edges = json.load(f)
        
        edges_match = baseline_edges == candidate_edges
        edge_diff = None
        if not edges_match:
            edge_diff = self._generate_diff(baseline["edges"], candidate["edges"])
        
        return nodes_match, edges_match, node_diff, edge_diff
    
    def _generate_diff(self, file1: Path, file2: Path) -> str:
        """Generate a diff between two JSON files"""
        result = subprocess.run(
            ["diff", "-u", str(file1), str(file2)],
            capture_output=True,
            text=True
        )
        return result.stdout
    
    def validate_commit(self, commit: str, db_full: Path, db_incremental: Path) -> ValidationResult:
        """Validate a single commit"""
        print(f"\n{'='*80}")
        print(f"Validating commit: {commit}")
        print(f"{'='*80}")
        
        # Checkout commit
        self.checkout_commit(commit)
        commit_short = self.run_git_command(["rev-parse", "--short", commit])
        
        # Create snapshot directories
        snapshot_dir_full = self.workspace / f"snapshots/{commit_short}/full"
        snapshot_dir_incr = self.workspace / f"snapshots/{commit_short}/incremental"
        
        # Run full ingestion
        print(f"\n[1/4] Running FULL ingestion...")
        metrics_full = self.run_ingest(db_full, "full", commit)
        print(f"  ✓ Processed {metrics_full.files_processed} files in {metrics_full.duration_sec:.2f}s")
        
        # Export full snapshot
        print(f"[2/4] Exporting FULL snapshot...")
        snapshot_full = self.export_snapshot(db_full, snapshot_dir_full)
        print(f"  ✓ Exported to {snapshot_dir_full}")
        
        # Run incremental ingestion
        print(f"[3/4] Running INCREMENTAL ingestion...")
        metrics_incremental = self.run_ingest(db_incremental, "incremental", commit)
        print(f"  ✓ Processed {metrics_incremental.files_processed} files in {metrics_incremental.duration_sec:.2f}s")
        print(f"  ⚡ Speedup: {metrics_full.duration_sec / metrics_incremental.duration_sec if metrics_incremental.duration_sec > 0 else 0:.1f}x")
        
        # Export incremental snapshot
        print(f"[4/4] Exporting INCREMENTAL snapshot...")
        snapshot_incremental = self.export_snapshot(db_incremental, snapshot_dir_incr)
        print(f"  ✓ Exported to {snapshot_dir_incr}")
        
        # Compare snapshots
        print(f"\n[DIFF] Comparing snapshots...")
        nodes_match, edges_match, node_diff, edge_diff = self.diff_snapshots(
            snapshot_full, snapshot_incremental
        )
        
        if nodes_match and edges_match:
            print(f"  ✅ PASS: Snapshots match perfectly!")
        else:
            print(f"  ❌ FAIL: Snapshots differ!")
            if not nodes_match:
                print(f"     - Nodes differ")
            if not edges_match:
                print(f"     - Edges differ")
        
        return ValidationResult(
            commit=commit,
            nodes_match=nodes_match,
            edges_match=edges_match,
            node_diff=node_diff,
            edge_diff=edge_diff,
            metrics_full=metrics_full,
            metrics_incremental=metrics_incremental
        )
    
    def run_validation(self, commits: List[str]) -> List[ValidationResult]:
        """Run validation across a series of commits"""
        # Check for dirty worktree before we start checking out commits
        try:
            status_output = self.run_git_command(["status", "--porcelain"])
            if status_output.strip():
                print("\n⚠️  WARNING: Repository has uncommitted changes!")
                print("The validation script will checkout different commits.")
                print("\nUncommitted changes detected:")
                for line in status_output.strip().split('\n')[:5]:  # Show first 5 files
                    print(f"  {line}")
                if len(status_output.strip().split('\n')) > 5:
                    print(f"  ... and {len(status_output.strip().split('\n')) - 5} more")
                print("\n❌ Please commit or stash your changes before running validation.")
                print("   git stash")
                print("   # or")
                print("   git commit -am 'WIP'")
                sys.exit(1)
        except subprocess.CalledProcessError as e:
            print(f"Warning: Could not check git status: {e}")
        
        # Create database paths
        db_full = self.workspace / "db_full.db"
        db_incremental = self.workspace / "db_incremental.db"
        
        # Clean up any existing databases
        if db_full.exists():
            shutil.rmtree(db_full)
        if db_incremental.exists():
            shutil.rmtree(db_incremental)
        
        # Store original branch/commit
        original_ref = self.run_git_command(["rev-parse", "--abbrev-ref", "HEAD"])
        if original_ref == "HEAD":
            original_ref = self.run_git_command(["rev-parse", "HEAD"])
        
        results = []
        try:
            for i, commit in enumerate(commits):
                print(f"\n{'#'*80}")
                print(f"# Commit {i+1}/{len(commits)}: {commit}")
                print(f"{'#'*80}")
                
                result = self.validate_commit(commit, db_full, db_incremental)
                results.append(result)
                
                if not (result.nodes_match and result.edges_match):
                    print(f"\n❌ Validation FAILED at commit {commit}")
                    print(f"\nArtifacts preserved in: {self.workspace / 'snapshots'}")
                    if result.node_diff:
                        print(f"\nNode diff:\n{result.node_diff[:500]}...")
                    if result.edge_diff:
                        print(f"\nEdge diff:\n{result.edge_diff[:500]}...")
                    break
        finally:
            # Restore original branch/commit
            self.checkout_commit(original_ref)
        
        return results
    
    def print_summary(self, results: List[ValidationResult]):
        """Print summary of validation results"""
        print(f"\n{'='*80}")
        print("VALIDATION SUMMARY")
        print(f"{'='*80}\n")
        
        passed = sum(1 for r in results if r.nodes_match and r.edges_match)
        failed = len(results) - passed
        
        print(f"Total commits tested: {len(results)}")
        print(f"✅ Passed: {passed}")
        print(f"❌ Failed: {failed}")
        print()
        
        # Metrics table
        print("Performance Metrics:")
        print(f"{'Commit':<12} {'Mode':<12} {'Files':<8} {'Duration':<12} {'Files/sec':<10}")
        print("-" * 80)
        
        for result in results:
            for metrics in [result.metrics_full, result.metrics_incremental]:
                commit_short = metrics.commit[:7]
                files_per_sec = metrics.files_processed / metrics.duration_sec if metrics.duration_sec > 0 else 0
                print(f"{commit_short:<12} {metrics.mode:<12} {metrics.files_processed:<8} {metrics.duration_sec:<12.2f} {files_per_sec:<10.1f}")
        
        print()
        
        if failed > 0:
            print("❌ VALIDATION FAILED")
            sys.exit(1)
        else:
            print("✅ ALL VALIDATIONS PASSED")
            sys.exit(0)


def main():
    parser = argparse.ArgumentParser(
        description="Validate incremental ingestion against full ingestion"
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path("."),
        help="Path to git repository to test"
    )
    parser.add_argument(
        "--commits",
        type=str,
        required=True,
        help="Comma-separated list of commits to test (e.g., HEAD~5,HEAD~3,HEAD)"
    )
    parser.add_argument(
        "--cg-binary",
        type=Path,
        default=Path("./target/release/cg"),
        help="Path to cg binary"
    )
    parser.add_argument(
        "--workspace",
        type=Path,
        default=None,
        help="Workspace directory for databases and snapshots (default: temp dir)"
    )
    
    args = parser.parse_args()
    
    # Parse commits
    commits = [c.strip() for c in args.commits.split(",")]
    
    # Create workspace
    if args.workspace:
        workspace = args.workspace
    else:
        workspace = Path(tempfile.mkdtemp(prefix="cg_validate_"))
    
    print(f"Workspace: {workspace}")
    print(f"Repository: {args.repo}")
    print(f"Commits to test: {commits}")
    print(f"CG binary: {args.cg_binary}")
    
    # Check if cg binary exists
    if not args.cg_binary.exists():
        print(f"Error: CG binary not found at {args.cg_binary}")
        print("Build it with: cargo build --release")
        sys.exit(1)
    
    # Run validation
    validator = IncrementalValidator(args.repo, args.cg_binary, workspace)
    results = validator.run_validation(commits)
    validator.print_summary(results)


if __name__ == "__main__":
    main()
