#!/bin/sh

# Local pre-push check: formatting, linting, and tests.
# Run this before pushing to catch issues early.
set -e

echo "==> Checking formatting..."
cargo fmt --check
echo "    OK"

echo "==> Running clippy..."
cargo clippy -- -D warnings
echo "    OK"

echo "==> Running tests..."
cargo test
echo "    OK"

echo ""
echo "All checks passed."
