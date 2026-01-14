# Incident Response (Enterprise)

## Severity Levels
- SEV-0: Active compromise / consensus safety breach / key theft
- SEV-1: Chain halt or widespread outage
- SEV-2: Partial outage / degraded performance
- SEV-3: Minor issue / no user impact

## Roles
- Incident Commander (IC)
- Ops Lead
- Security Lead
- Comms Lead
- Engineering Owner

## Golden Rules
1. Contain first, then eradicate, then recover.
2. Preserve evidence: logs, metrics, artifacts, configs, container digests.
3. Prefer reversible changes (feature flags, rate limits).

## Evidence Checklist
- Pod logs (last 24h)
- Node logs (systemd/journal)
- Container image digests + SBOM
- Helm values + rendered manifests
- NetworkPolicy/Kyverno policy versions
- Access logs (KMS/Vault/registry)

## Communications
- Internal updates every 30-60 minutes for SEV-0/1
- External advisory after confirmed fix for security incidents

## Post-Incident
- Blameless postmortem
- Action items with owners + deadlines
- Regression tests added
