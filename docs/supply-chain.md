# Supply-chain hardening

This repository includes:
- `deny.toml` for dependency policy checks (license/duplicate advisories).
- Release verification notes in `docs/release-verification.md`.

## SBOM
You can generate a CycloneDX SBOM if you have `cargo-cyclonedx` installed:

```bash
cargo install cargo-cyclonedx
./scripts/generate_sbom.sh
```

## Provenance
A reference GitHub Actions workflow is included to generate build provenance using SLSA tooling.
Adapt it to your org requirements and signing keys.
