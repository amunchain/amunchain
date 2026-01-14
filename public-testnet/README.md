# Amunchain Public Testnet Bundle

This bundle contains:
- Full project (fixed build)
- 4-node VPS systemd template
- Public testnet join kit

## Public join (for external users)
1) Copy `public-testnet/` directory to your Ubuntu server.
2) Put the correct bootstrap PeerId into `public-testnet/join_testnet.sh` (replace `BOOTSTRAP_PEER_ID_PLACEHOLDER`).
3) Run:
```bash
chmod +x join_testnet.sh
sudo ./join_testnet.sh
journalctl -u amunchain-public -f
```

Security:
- HTTP binds to 127.0.0.1 by default.
- Only P2P should be exposed publicly.
