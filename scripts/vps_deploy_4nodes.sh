#!/usr/bin/env bash
set -euo pipefail

# Amunchain: Deploy 4 nodes on a single Ubuntu VPS (systemd + UFW hardening)
#
# Usage (as root):
#   ./scripts/vps_deploy_4nodes.sh
#
# Notes:
# - Exposes ONLY: SSH(22) and P2P ports (30333-30336 by default).
# - HTTP/metrics are bound to 127.0.0.1 (local only) by default.
# - You can change ports and settings by editing the node*.toml generated under /srv/amunchain/configs/.

AMU_USER="amunchain"
AMU_HOME="/srv/amunchain"
BIN_DST="${AMU_HOME}/bin/amunchain"
CFG_DIR="${AMU_HOME}/configs"
DATA_DIR="${AMU_HOME}"

P2P_BASE_PORT=30333
HTTP_BASE_PORT=9091   # each node will use 9091..9094 locally on 127.0.0.1

require_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "ERROR: run as root." >&2
    exit 1
  fi
}

install_deps() {
  apt update
  apt install -y ufw fail2ban ca-certificates
}

ensure_user() {
  if ! id -u "${AMU_USER}" >/dev/null 2>&1; then
    adduser --system --group --home "${AMU_HOME}" "${AMU_USER}"
  fi
  mkdir -p "${AMU_HOME}"/{bin,configs,node1,node2,node3,node4,logs}
  chown -R "${AMU_USER}:${AMU_USER}" "${AMU_HOME}"
  chmod 750 "${AMU_HOME}"
}

install_binary() {
  if [[ ! -f "./target/release/amunchain" ]]; then
    echo "ERROR: ./target/release/amunchain not found. Build first: cargo build --release --locked" >&2
    exit 1
  fi
  install -m 0750 -o "${AMU_USER}" -g "${AMU_USER}" ./target/release/amunchain "${BIN_DST}"
}

gen_configs() {
  local template="./configs/node.toml"
  if [[ ! -f "${template}" ]]; then
    echo "ERROR: ${template} not found." >&2
    exit 1
  fi

  for i in 1 2 3 4; do
    local p2p_port=$((P2P_BASE_PORT + i - 1))
    local http_port=$((HTTP_BASE_PORT + i - 1))

    sed       -e "s/^name = .*/name = \"amunchain-node${i}\"/"       -e "s#^data_dir = .*#data_dir = \"${DATA_DIR}/node${i}\"#"       -e "s/^listen_addr = \"127\.0\.0\.1:9090\"/listen_addr = \"127.0.0.1:${http_port}\"/"       -e "s#/tcp/30333#/tcp/${p2p_port}#"       "${template}" > "${CFG_DIR}/node${i}.toml"

    chown "${AMU_USER}:${AMU_USER}" "${CFG_DIR}/node${i}.toml"
    chmod 640 "${CFG_DIR}/node${i}.toml"
  done
}

install_systemd() {
  cat > /etc/systemd/system/amunchain@.service <<'EOF'
[Unit]
Description=Amunchain Node %i
After=network-online.target
Wants=network-online.target

[Service]
User=amunchain
Group=amunchain

# Hardened service sandbox
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/srv/amunchain
CapabilityBoundingSet=
LockPersonality=true
MemoryDenyWriteExecute=true
RestrictSUIDSGID=true
RestrictNamespaces=true
RestrictRealtime=true
SystemCallArchitectures=native
SystemCallFilter=@system-service @network-io

LimitNOFILE=65536

ExecStart=/srv/amunchain/bin/amunchain --config /srv/amunchain/configs/node%i.toml
Restart=always
RestartSec=2
TimeoutStopSec=20

[Install]
WantedBy=multi-user.target
EOF

  systemctl daemon-reexec
  systemctl daemon-reload

  for i in 1 2 3 4; do
    systemctl enable "amunchain@${i}"
    systemctl restart "amunchain@${i}" || systemctl start "amunchain@${i}"
  done
}

configure_ufw() {
  ufw --force reset
  ufw default deny incoming
  ufw default allow outgoing
  ufw allow 22/tcp
  ufw allow "${P2P_BASE_PORT}:$((P2P_BASE_PORT+3))/tcp"
  ufw --force enable
}

summary() {
  echo
  echo "Done."
  echo "Services:"
  systemctl --no-pager --full status amunchain@1 amunchain@2 amunchain@3 amunchain@4 || true
  echo
  echo "Listening ports (expected: 30333-30336 + local 9091-9094):"
  ss -lntp | grep amunchain || true
}

main() {
  require_root
  install_deps
  ensure_user
  install_binary
  gen_configs
  install_systemd
  configure_ufw
  summary
}

main "$@"
