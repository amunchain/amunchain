# Enterprise Security Profile (99.9% posture)

This project ships with a hardened baseline. To reach an enterprise posture, enforce ALL of the following:

## 1) Build & Supply Chain
- `cargo deny` + `cargo audit` gates in CI
- SBOM generation (CycloneDX)
- Reproducible builds (`scripts/repro-build.sh`)
- Artifact signing (Cosign) and cluster admission verification (Kyverno)

## 2) Cluster Admission Control
Apply Kyverno policies from `k8s/`:
- `kyverno-restricted.yaml`
- `kyverno-seccomp-nonroot.yaml`
- `kyverno-verify-images.yaml` (edit `<ORG>/<REPO>`)

## 3) Secrets Management (KMS/Vault)
Use Secrets Store CSI Driver. Examples in `k8s/secrets-store/`.
Enable in Helm:
```yaml
secretsStore:
  enabled: true
  provider: "vault"
```

## 4) Network Zero Trust
Use `helm/.../networkpolicy.yaml` in `max` mode or apply `k8s/amunchain-networkpolicy-max-allpods.yaml`.

## 5) Continuous Security Testing
- Fuzz smoke tests in CI (`fuzz-smoke` job)
- Add longer fuzz runs nightly in a scheduled workflow for deeper coverage.

## 6) Monitoring & Incident Response
- Enable Prometheus scraping (if used)
- Alert on consensus stalls, peer churn spikes, invalid message spikes.
