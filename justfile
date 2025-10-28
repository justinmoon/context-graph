#!/usr/bin/env just --justfile

# Show available commands
default:
    @just --list

# Run all CI checks (format, lint, test)
ci:
    nix run .#ci

# Format code with rustfmt
fmt:
    cargo fmt

# Run clippy lints
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-features

# Build the project
build:
    cargo build

# Build release binary
build-release:
    cargo build --release

# Check without building
check:
    cargo check --all-targets --all-features

# Run with watch mode
watch:
    cargo watch -x test

# Clean build artifacts
clean:
    cargo clean

# Update dependencies
update:
    cargo update

# Run real-world smoke test
smoke-test:
    cargo build --release
    ./scripts/test_real_repo.sh

# Validate incremental ingestion (3 recent commits)
validate-incremental:
    cargo build --release
    ./scripts/validate_incremental.py --commits HEAD~2,HEAD~1,HEAD

# Validate incremental ingestion (extended - 6 commits)
validate-incremental-full:
    cargo build --release
    ./scripts/validate_incremental.py --commits HEAD~10,HEAD~8,HEAD~5,HEAD~3,HEAD~1,HEAD
