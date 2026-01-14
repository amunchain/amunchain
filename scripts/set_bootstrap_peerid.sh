#!/usr/bin/env bash
set -euo pipefail
if [ $# -ne 1 ]; then
  echo "Usage: $0 <BOOTSTRAP_PEER_ID>"
  exit 1
fi
PEER="$1"
FILE="public-testnet/join_testnet.sh"
if [ ! -f "$FILE" ]; then
  echo "Missing $FILE"
  exit 1
fi
sed -i "s/BOOTSTRAP_PEER_ID_PLACEHOLDER/${PEER}/g" "$FILE"
echo "Updated $FILE"
