
#!/bin/sh
set -eu
if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install cargo-audit --locked
fi
if ! command -v cargo-deny >/dev/null 2>&1; then
  cargo install cargo-deny --locked
fi
if ! command -v cyclonedx-bom >/dev/null 2>&1; then
  cargo install cyclonedx-bom --locked
fi
cargo audit
cargo deny check
cyclonedx-bom -o sbom.cdx.json
echo "Generated sbom.cdx.json"
