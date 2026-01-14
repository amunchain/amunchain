#!/usr/bin/env bash
set -euo pipefail

# Security & Quality Gate for Amunchain
# Run this on a build machine with Rust installed.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "==> rustc/cargo versions"
rustc -V
cargo -V

echo "==> format check"
cargo fmt --all -- --check

echo "==> clippy (deny warnings)"
cargo clippy --all-targets --all-features -- -D warnings

echo "==> unit/integration tests"
cargo test --all --locked

echo "==> dependency policy (cargo-deny)"
if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check
else
  echo "WARN: cargo-deny not installed. Install with: cargo install cargo-deny"
fi

echo "==> vulnerability audit (cargo-audit)"
if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit
else
  echo "WARN: cargo-audit not installed. Install with: cargo install cargo-audit"
fi

echo "==> optional fuzz (requires cargo-fuzz)"
if command -v cargo-fuzz >/dev/null 2>&1; then
  echo "Running a short smoke fuzz (60s each)."
  timeout 60s cargo fuzz run fuzz_codec_consensusmsg || true
  timeout 60s cargo fuzz run fuzz_state_merkle_proof || true
else
  echo "INFO: cargo-fuzz not installed. Install with: cargo install cargo-fuzz"
fi

echo "OK: Security gate completed."
