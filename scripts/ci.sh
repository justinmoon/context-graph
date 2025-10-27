#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[ci] %s\n' "$*"
}

log "Running Rust CI checks..."
echo ""

log "1. Checking formatting..."
cargo fmt -- --check

log "2. Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

log "3. Running tests..."
cargo test --all-features

log "4. Building release binary..."
cargo build --release

echo ""
log "✅ All CI checks passed!"
