#!/usr/bin/env bash
set -euo pipefail

# Produce canonical bytes for a peer registry (v1) from environment variables and a peer list file.
# Usage:
#   NETWORK="amunchain/consensus/v2" ISSUED_AT_MS=... EXPIRES_AT_MS=... ./scripts/peer_registry_canonical.sh peers.txt > canonical.txt
#
# peers.txt must contain one PeerId per line.

peers_file="${1:-}"
if [[ -z "${peers_file}" || ! -f "${peers_file}" ]]; then
  echo "Usage: NETWORK=... ISSUED_AT_MS=... EXPIRES_AT_MS=... $0 peers.txt" >&2
  exit 2
fi

: "${NETWORK:?NETWORK required}"
: "${ISSUED_AT_MS:?ISSUED_AT_MS required}"
: "${EXPIRES_AT_MS:?EXPIRES_AT_MS required}"

echo "v1"
echo "network=${NETWORK}"
echo "issued_at_ms=${ISSUED_AT_MS}"
echo "expires_at_ms=${EXPIRES_AT_MS}"
echo "peers"
# sort + unique; strip empty lines/spaces
cat "${peers_file}" | sed 's/\r$//' | awk 'NF' | sort | uniq
