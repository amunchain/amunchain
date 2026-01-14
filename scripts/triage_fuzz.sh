#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <target> <artifact_path> [seconds]" >&2
  echo "Example: $0 fuzz_codec_consensusmsg fuzz/artifacts/fuzz_codec_consensusmsg/crash-123 30" >&2
  exit 2
fi

target="$1"
artifact="$2"
seconds="${3:-30}"

if [[ ! -f "$artifact" ]]; then
  echo "Artifact not found: $artifact" >&2
  exit 1
fi

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir/fuzz"

# Reproduce
echo "[triage] Reproducing crash..."
cargo fuzz run "$target" "$artifact" -- -runs=1 || true

# Minimize to a stable reproducer
min_out="/tmp/${target}-minimized.bin"
echo "[triage] Minimizing (max_total_time=${seconds}s)..."
cargo fuzz run "$target" "$artifact" -- -minimize_crash=1 -max_total_time="$seconds" -exact_artifact_path="$min_out" || true

# Add to corpus (if produced)
corpus_dir="$root_dir/fuzz/corpus/$target"
mkdir -p "$corpus_dir"
if [[ -f "$min_out" ]]; then
  ts="$(date +%Y%m%d_%H%M%S)"
  cp "$min_out" "$corpus_dir/min_${ts}.bin"
  echo "[triage] Added minimized input to corpus: $corpus_dir/min_${ts}.bin"
else
  echo "[triage] No minimized output produced (still uploaded artifact)."
fi

echo "[triage] Done. Commit corpus changes if they are valuable."
