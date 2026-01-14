# Amunchain Security Posture (Enterprise Hardened)

This bundle includes Patch2 (consensus replay window) + Patch3 (production lints & P2P hardening) + Patch4 (signed peer registry + wire-level legacy drops).

## Production mode
Build and run with the `production` feature to enable strict startup lints and protocol hardening:

```bash
cargo build --release --features production
```

### What production mode enforces
- Consensus: legacy messages (`epoch == 0`) are rejected by default (TideConfig.require_epoch=true).
- Startup lint: node refuses to start if `[p2p].allow_peers` is empty.

### Recommended config defaults (production)
- Set a versioned consensus topic, e.g. `amunchain/consensus/v2`.
- Use a strict allowlist for validator nodes.
- Keep HTTP/metrics bound to localhost or behind an authenticated reverse proxy.

### Signed peer registry (optional)

If your validator allowlist is large or changes frequently, you can configure a **signed peer registry**.
This keeps the operational workflow simple while preserving integrity.

- Create a TOML registry file (example: `configs/peer_registry.toml`).
- Canonical bytes to sign are the **sorted unique** peer IDs joined by `\n` and ending with a trailing `\n`.
- Sign with the registry Ed25519 private key.
- Pin the registry public key in config, and point to the file:

```toml
[p2p]
allow_peers = []
peer_registry_path = "configs/peer_registry.toml"
peer_registry_pubkey_hex = "<32-byte-ed25519-pubkey-hex>"
```

If both `allow_peers` and the registry are empty/invalid, production builds will refuse to start.

## Testnet / dev mode
Default build (no features) keeps backwards compatibility and is suitable for dev/testnet experimentation.
