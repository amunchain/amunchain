#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-cyclonedx >/dev/null 2>&1; then
  echo "cargo-cyclonedx not found. Install with: cargo install cargo-cyclonedx" >&2
  exit 1
fi

out_dir="artifacts"
mkdir -p "${out_dir}"

cargo cyclonedx --format json --output-file "${out_dir}/sbom.cdx.json"
echo "SBOM written to ${out_dir}/sbom.cdx.json"
