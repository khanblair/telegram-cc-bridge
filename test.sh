#!/bin/bash
set -e

echo "=== Telegram CC Bridge - Test Script ==="
echo ""

echo "[1/4] Checking Rust toolchain..."
rustc --version
cargo --version
echo ""

echo "[2/4] Running cargo check..."
cargo check 2>&1
echo "✓ cargo check passed"
echo ""

echo "[3/4] Running cargo test..."
cargo test 2>&1
echo "✓ cargo test passed"
echo ""

echo "[4/4] Running cargo clippy..."
cargo clippy -- -D warnings 2>&1 || echo "⚠ clippy warnings found (non-blocking)"
echo ""

echo "=== All checks completed ==="
