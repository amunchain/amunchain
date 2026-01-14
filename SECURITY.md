
# Security Policy

**Release:** Amunchain v1.0.0


Report security issues privately.

## Current controls
- No unsafe code in this repository
- Encrypted P2P transport (Noise)
- Canonical encoding for wire messages
- Peer scoring with ban backoff and **reputation-based throttling**
- Prometheus metrics exposed via HTTP `/metrics`
- Supply-chain gates: cargo-audit, cargo-deny, SBOM

## Operational hardening knobs
- `AMUNCHAIN_KEY_PASSPHRASE`: encrypt the validator key at rest
- `AMUNCHAIN_PBKDF2_ITERS`: PBKDF2 iterations for key-at-rest encryption (clamped)

## Planned
- Full EVM state commit integration
- Extended chaos tests and differential tests
