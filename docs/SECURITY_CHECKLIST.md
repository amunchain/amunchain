# Amunchain Security Checklist (Pre-Production)

This checklist is designed to help you reach a *provable* "production-secured" posture.

## Build integrity
- [ ] `cargo build --release --locked`
- [ ] `./scripts/security_gate.sh` passes (fmt, clippy -D warnings, tests, deny, audit)
- [ ] Images/charts pinned (versions and, ideally, digests)

## Secrets
- [ ] No default passwords (Grafana, etc.)
- [ ] Private keys stored encrypted-at-rest (keystore passphrase or external KMS/HSM)
- [ ] `.env` files never committed

## Network exposure
- [ ] Only P2P ports exposed publicly
- [ ] HTTP/metrics bound to localhost OR protected behind mTLS/auth
- [ ] Firewall rules verified (`ufw status`)

## Runtime hardening
- [ ] Non-root service user
- [ ] systemd hardening enabled (NoNewPrivileges, ProtectSystem, etc.)
- [ ] Resource limits set (ulimit NOFILE, CPU/memory limits if containerized)

## Observability
- [ ] Metrics access restricted
- [ ] Logs rotated and protected (permissions)
- [ ] Alerts enabled for crash loops, peer floods, disk pressure

## Incident readiness
- [ ] Backups tested (data_dir snapshots)
- [ ] Key rotation plan documented
- [ ] Upgrade rollback steps validated
