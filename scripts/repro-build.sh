#!/bin/sh
set -eu

IMAGE="rust:1.75-bookworm"
APP_DIR="/work"

echo "== Repro build (containerized, locked) =="

docker run --rm -t -v "$(pwd)":$APP_DIR -w $APP_DIR $IMAGE sh -eu -c '
  rustc -V
  cargo -V
  cargo clean
  cargo build --release --locked
  sha256sum target/release/amunchain > target/release/amunchain.sha256
'

echo "Built: target/release/amunchain"
echo "Hash : $(cat target/release/amunchain.sha256 | cut -d" " -f1)"
