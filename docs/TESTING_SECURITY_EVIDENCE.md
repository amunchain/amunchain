# Security Evidence: Tests, Fuzzing, and Gates

This project ships with multiple *evidence-generating* security controls:

## 1) Property-based tests (proptest)

Run:
```bash
cargo test --all-features
```

Key invariants are covered under `tests/prop_*`.

## 2) Fuzzing (cargo-fuzz)

Requires nightly toolchain:
```bash
rustup toolchain install nightly
cargo install cargo-fuzz --locked
```

Run short smoke:
```bash
cd fuzz
cargo fuzz run fuzz_codec_consensusmsg -- -max_total_time=20
cargo fuzz run fuzz_state_merkle_proof -- -max_total_time=20
cargo fuzz run fuzz_peer_registry_parse -- -max_total_time=20
```

## 3) Supply-chain gates

- `cargo deny` via `deny.toml`
- SBOM generation (CycloneDX JSON)

CI workflows under `.github/workflows/`.
