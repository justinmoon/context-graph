#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/microsoft/TypeScript.git"
REPO_DIR="${CG_TS_REPO_DIR:-$HOME/.cache/context-graph/TypeScript}"
TAG_COUNT="${CG_TS_TAG_COUNT:-3}"
CG_BIN="${CG_BIN_PATH:-./target/release/cg}"

log() {
  echo "[typescript-validation] $*"
}

log "Repository directory: $REPO_DIR"

if [ ! -d "${REPO_DIR}" ]; then
  log "Cloning TypeScript repo..."
  mkdir -p "$(dirname "$REPO_DIR")"
  git clone "$REPO_URL" "$REPO_DIR"
else
  log "Updating existing TypeScript repo..."
  git -C "$REPO_DIR" fetch --tags --prune --quiet
fi

log "Selecting random tags..."
ALL_TAGS=$(git -C "$REPO_DIR" tag --sort=-creatordate)

if [ -z "$ALL_TAGS" ]; then
  echo "No tags found in the repository."
  exit 1
fi

SELECTED_TAGS=$(CG_TS_ALL_TAGS="$ALL_TAGS" python3 - <<'PY'
import os
import random

all_tags = [line.strip() for line in os.environ.get("CG_TS_ALL_TAGS", "").splitlines() if line.strip()]
if not all_tags:
    raise SystemExit("No tags available")

count = int(os.getenv("CG_TS_TAG_COUNT", "3"))
if len(all_tags) <= count:
    chosen = all_tags
else:
    chosen = random.sample(all_tags, count)

print(' '.join(chosen))
PY
)

read -r -a TAGS <<< "$SELECTED_TAGS"

if [ ${#TAGS[@]} -eq 0 ]; then
  echo "No tags found in the repository."
  exit 1
fi

log "Selected tags: ${TAGS[*]}"

log "Cleaning Kuzu build cache..."
rm -rf target/release/build/kuzu-*

log "Building cg binary..."
cargo build --release

COMMITS=$(IFS=,; echo "${TAGS[*]}")
log "Running incremental validation on commits: $COMMITS"
./scripts/validate_incremental.py \
  --repo "$REPO_DIR" \
  --commits "$COMMITS" \
  --cg-binary "$CG_BIN"
