# Validation Checklist (run locally / CI)

## Rust build
```bash
cargo build --release
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Kyverno YAML parse
```bash
python3 -c "import yaml; yaml.safe_load(open('k8s/kyverno-deny-loadbalancer-except-allowed.yaml')); yaml.safe_load(open('k8s/kyverno-deny-nodeport-except-allowed.yaml')); print('YAML OK')"
```

## Search for legacy Helm path / ports
```bash
rg 'helm/amunchain' || echo OK
rg '\\b(4001|4002|4003|4004)\\b' || echo OK
```
