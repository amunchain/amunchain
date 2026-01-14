# Security

This document summarizes the key security controls implemented in Amunchain Layer 0.

## Design goals

- **Safety-first defaults**: reject malformed or oversized inputs early.
- **Determinism**: canonical encoding and ordered containers.
- **Defense in depth**: multiple layers of caps, validation, and logging.

## Code safety

- Project code forbids `unsafe` and denies missing docs.

## P2P network security

- Noise transport encryption (libp2p).
- Strict gossipsub validation mode.
- Per-peer **rate limiting** and **peer scoring** with temporary bans.
- Per-IP connection cap (best-effort).
- Message replay protection (SHA-256 replay cache).
- Hard caps for wire message size.

## Serialization and parsing

- Canonical bincode options: fixed-int encoding and `reject_trailing_bytes()`.
- Decoding uses a hard `with_limit(...)` cap to mitigate memory/CPU bombs.

## Consensus signing

- Domain-separated signing bytes for votes/commits.
- Ed25519 signature verification with strict signature length checks.

## Key management

Validator keys are stored under `data_dir/validator.key`.

### Key-at-rest encryption (recommended for production)

If the environment variable `AMUNCHAIN_KEY_PASSPHRASE` is set, the keystore will:

1. Derive an AES-256-GCM key using PBKDF2-HMAC-SHA256.
2. Encrypt the PKCS#8 Ed25519 private key material before writing it to disk.
3. Require the passphrase to load the key on restart.

If `AMUNCHAIN_KEY_PASSPHRASE` is not set, the key is stored unencrypted (still written atomically with restrictive file permissions).

## Audit trail

- Signing operations write a minimal audit line containing a SHA-256 of the signed payload (not the payload itself).
- Audit logs are rotated at a fixed size limit (best-effort).

## Recommended production settings

- Run behind firewall rules and restrict inbound ports.
- Use an allowlist for P2P peers if operating a permissioned network.
- Provide `AMUNCHAIN_KEY_PASSPHRASE` via your secret manager (Vault/KMS/...) instead of plain env files.
