# Signed Peer Registry (v1)

This project supports loading a **signed peer allowlist** from an out-of-band TOML file.
The node verifies the file with a pinned Ed25519 public key before using it.

## Why
- Prevents **open-by-default** P2P exposure.
- Makes allowlist distribution manageable (one file instead of editing every node config).
- Binds the allowlist to a specific network/topic to reduce cross-network rollback.

## File format

See `configs/peer_registry.toml` for an example.

Required fields:
- `version = 1`
- `network = "amunchain/consensus/v2"` (must match the node's P2P topic)
- `issued_at_ms`, `expires_at_ms` (ms since UNIX epoch)
- `peers = [...]`
- `signature_hex = "..."` (Ed25519 signature, 64 bytes hex)

## Canonical bytes (what gets signed)

```
v1
network=<network>
issued_at_ms=<u64>
expires_at_ms=<u64>
peers
<peer1>
<peer2>
...
```

Peers are sorted and deduplicated before signing/verification.

## Node-side policy

Configured in `configs/node.toml`:
- `peer_registry_min_version`
- `peer_registry_max_age_ms`
- `peer_registry_grace_ms`
- `peer_registry_require_fresh`

In `--features production`, freshness fields are required by default.
