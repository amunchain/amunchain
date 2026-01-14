#!/usr/bin/env bash
set -euo pipefail

# =========================
# Amunchain Testnet Joiner
# =========================
CHAIN_ID="amunchain-testnet-1"

# Bootstrap Node (Node1)
BOOTSTRAP_IP="72.60.33.99"
BOOTSTRAP_P2P_PORT="30333"
BOOTSTRAP_PEER_ID="BOOTSTRAP_PEER_ID_PLACEHOLDER"

# Safety guard: refuse to run with placeholder bootstrap peer id
if [[ "$BOOTSTRAP_PEER_ID" == *PLACEHOLDER* ]]; then
  echo "ERROR: You must set BOOTSTRAP_PEER_ID to the bootstrap node PeerID before joining the testnet." >&2
  exit 1
fi

# Local node ports (override via env if needed)
P2P_PORT="${P2P_PORT:-30333}"
HTTP_PORT="${HTTP_PORT:-9091}"

INSTALL_DIR="${INSTALL_DIR:-/srv/amunchain}"
BIN_DIR="${INSTALL_DIR}/bin"
DATA_DIR="${INSTALL_DIR}/node"
CONF_DIR="/etc/amunchain"

# Provide files in same directory as this script by default
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GENESIS_SRC="${GENESIS_SRC:-${SCRIPT_DIR}/genesis.json}"
VALIDATORS_SRC="${VALIDATORS_SRC:-${SCRIPT_DIR}/validators.json}"
BINARY_SRC="${BINARY_SRC:-${SCRIPT_DIR}/amunchain}"

echo "[*] Installing Amunchain public node..."
sudo mkdir -p "${BIN_DIR}" "${DATA_DIR}" "${CONF_DIR}"

echo "[*] Copying binary..."
sudo cp "${BINARY_SRC}" "${BIN_DIR}/amunchain"
sudo chmod +x "${BIN_DIR}/amunchain"

echo "[*] Copying genesis + validators..."
sudo cp "${GENESIS_SRC}" "${DATA_DIR}/genesis.json"
sudo cp "${VALIDATORS_SRC}" "${DATA_DIR}/validators.json"

echo "[*] Creating service user (amunchain)..."
if ! id amunchain >/dev/null 2>&1; then
  sudo useradd -r -s /usr/sbin/nologin amunchain
fi
sudo chown -R amunchain:amunchain "${INSTALL_DIR}"

echo "[*] Generating PeerId..."
PEER_ID=$("${BIN_DIR}/amunchain" --print-peer-id "${DATA_DIR}" | tail -n 1 | tr -d '\r\n')
echo "    PeerId: ${PEER_ID}"

echo "[*] Writing config..."
sudo tee "${CONF_DIR}/node.toml" >/dev/null <<EOF
[node]
name = "public-node"
data_dir = "${DATA_DIR}"

[http]
listen_addr = "127.0.0.1:${HTTP_PORT}"

[p2p]
listen_addr = "/ip4/0.0.0.0/tcp/${P2P_PORT}"
topic = "amunchain-consensus"
max_msg_per_sec = 200
max_peers_per_ip = 4
bootstrap = ["/ip4/${BOOTSTRAP_IP}/tcp/${BOOTSTRAP_P2P_PORT}/p2p/${BOOTSTRAP_PEER_ID}"]
allow_peers = []  # Open testnet by default

[security]
require_signed_messages = true
EOF

echo "[*] Installing systemd service..."
sudo tee /etc/systemd/system/amunchain-public.service >/dev/null <<EOF
[Unit]
Description=Amunchain Public Testnet Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=amunchain
Group=amunchain
ExecStart=${BIN_DIR}/amunchain ${CONF_DIR}/node.toml
Restart=always
RestartSec=3
StartLimitIntervalSec=30
StartLimitBurst=5

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
LockPersonality=true
RestrictRealtime=true
MemoryDenyWriteExecute=true

LimitNOFILE=1048576
LimitNPROC=4096

StandardOutput=journal
StandardError=journal
SyslogIdentifier=amunchain-public

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable amunchain-public
sudo systemctl restart amunchain-public

echo
echo "[+] Done!"
echo "    Logs: journalctl -u amunchain-public -f"
echo
echo "[!] IMPORTANT:"
echo "    Replace BOOTSTRAP_PEER_ID_PLACEHOLDER with the real Node1 PeerId (bootstrap)."
