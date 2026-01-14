
#!/bin/sh
set -eu

echo "== Test Campaign =="

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

echo "== Supply chain checks =="
if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install cargo-audit --locked
fi
if ! command -v cargo-deny >/dev/null 2>&1; then
  cargo install cargo-deny --locked
fi
cargo audit
cargo deny check

echo "== No-unsafe check (cargo geiger) =="
if ! command -v cargo-geiger >/dev/null 2>&1; then
  cargo install cargo-geiger --locked
fi
cargo geiger --all-features

echo "== Coverage (tarpaulin, Linux only) =="
if command -v cargo-tarpaulin >/dev/null 2>&1; then
  cargo tarpaulin --all-features --workspace --timeout 120 --fail-under 60
else
  echo "cargo-tarpaulin not installed; skipping coverage gate"
fi

echo "Done."
