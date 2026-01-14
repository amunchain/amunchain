
# Amunchain Layer0 (Hardened Skeleton)

**Release:** Amunchain v1.0.0


This repo is a hardened, deterministic baseline:
- Canonical encoding (bincode fixedint)
- Tide finality gadget with signature verification
- libp2p Noise+Yamux gossipsub with anti-replay + rate limits + peer scoring
- Persistent state with deterministic Merkle roots and inclusion proofs
- Prometheus metrics + JSON logging

## Production hardening

- **Key-at-rest encryption**: set `AMUNCHAIN_KEY_PASSPHRASE` (encrypts `data_dir/validator.key`).
- Store `data_dir` on an encrypted volume and restrict permissions.
- Consider running in a permissioned mode using `allow_peers`.
- Keep ports firewalled and expose only what you need.

See `docs/security.md` and `docs/operations.md` for operational guidance.


## VPS deployment (4 nodes)

For a hardened 4-node deployment on a single Ubuntu VPS (systemd + UFW), build first and then run:

```bash
cargo build --release --locked
sudo ./scripts/vps_deploy_4nodes.sh
```

## Security/quality gate

Run the full pre-release checks (fmt + clippy + tests + deny + audit + optional fuzz):

```bash
./scripts/security_gate.sh
```

## Monitoring (safe defaults)

Monitoring binds to localhost only. Create a local `.env` from the example and set a strong Grafana password:

```bash
cd monitoring
cp .env.example .env
# edit .env
docker compose -f docker-compose.monitoring.yml up -d
```
