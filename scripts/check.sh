#!/bin/bash
# Check script: runs fmt, clippy, and tests

set -e

echo "=== Checking format ==="
cargo fmt --check

echo "=== Running clippy ==="
cargo clippy -- -D warnings

echo "=== Running tests ==="
cargo test

echo "=== All checks passed! ==="
