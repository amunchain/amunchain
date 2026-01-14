#!/usr/bin/env bash
set -euo pipefail

# Build + generate 4-node configs for running on a single VPS.
# Output:
#   - /etc/amunchain/node{1..4}.toml
#   - data dirs under /srv/amunchain/node{1..4}
#
# Usage:
#   ./scripts/setup_4nodes_vps.sh
#   # then run nodes:
#   ./target/release/amunchain /etc/amunchain/node1.toml
#   ...

BIN=${BIN:-./target/release/amunchain}
ETC_DIR=${ETC_DIR:-/etc/amunchain}
DATA_DIR=${DATA_DIR:-/srv/amunchain}
TOPIC=${TOPIC:-amunchain/consensus/v1}

# Ports
P2P_BASE=${P2P_BASE:-30333}
HTTP_BASE=${HTTP_BASE:-9091}

mkdir -p "$ETC_DIR" "$DATA_DIR"

# Build if binary missing.
if [[ ! -x "$BIN" ]]; then
  echo "[+] Building release binary..."
  cargo build --release
fi

# Create identities and read peer ids.
peer_ids=()
for i in 1 2 3 4; do
  nd="$DATA_DIR/node$i"
  mkdir -p "$nd"
  pid=$($BIN --print-peer-id "$nd")
  peer_ids+=("$pid")
  echo "[+] node$i peer_id=$pid"
done

node1_pid=${peer_ids[0]}

# Allowlist all 4 peers to enforce strict membership.
allow_list_inline="["
for idx in "${!peer_ids[@]}"; do
  pid="${peer_ids[$idx]}"
  if [[ $idx -ne 0 ]]; then allow_list_inline+=", "; fi
  allow_list_inline+="\"$pid\""
done
allow_list_inline+="]"


# NOTE: validators_hex are placeholders (32-byte ed25519 pubkeys in hex).
# Replace them with your real validator pubkeys for a real PoS validator set.
validators='["0000000000000000000000000000000000000000000000000000000000000000"]'

for i in 1 2 3 4; do
  name="amunchain-node$i"
  data="$DATA_DIR/node$i"
  http_port=$((HTTP_BASE + (i-1)))
  p2p_port=$((P2P_BASE + (i-1)))

  bootstrap="[]"
  if [[ $i -ne 1 ]]; then
    # Dial node1 on localhost (single-VPS).
    bootstrap="[\"/ip4/127.0.0.1/tcp/${P2P_BASE}/p2p/${node1_pid}\"]"
  fi

  cat > "$ETC_DIR/node$i.toml" <<EOF
[node]
name = "$name"
data_dir = "$data"

[http]
listen_addr = "0.0.0.0:${http_port}"

[p2p]
listen_addr = "/ip4/0.0.0.0/tcp/${p2p_port}"
topic = "$TOPIC"
max_msg_per_sec = 200
max_peers_per_ip = 4
bootstrap = ${bootstrap}
allow_peers = ${allow_list_inline}

[consensus]
validators_hex = ${validators}
EOF

  echo "[+] wrote $ETC_DIR/node$i.toml"
done

echo
echo "Done. Run in 4 terminals (or use tmux):"
echo "  $BIN $ETC_DIR/node1.toml"
echo "  $BIN $ETC_DIR/node2.toml"
echo "  $BIN $ETC_DIR/node3.toml"
echo "  $BIN $ETC_DIR/node4.toml"
