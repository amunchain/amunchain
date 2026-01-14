# Bug Bounty Playbook (Enterprise)

This playbook defines a safe, professional vulnerability disclosure program for Amunchain Layer0.

## Scope
In-scope targets:
- Consensus layer, networking (P2P), message codecs/decoders, persistence, CLI/API surface, Helm manifests, Docker image.
- Any production/staging deployment operated by you.

Out-of-scope (unless explicitly allowed):
- Third-party services you do not own/control.
- Social engineering, phishing, physical attacks.
- Denial-of-service that degrades availability for real users (unless you provide a dedicated test env).

## Safe Harbor
Researchers acting in good faith are authorized to test *within scope* and will not face legal action, provided they:
- Avoid privacy violations and data exfiltration.
- Do not disrupt production services.
- Report findings promptly and responsibly.

## Severity & Rewards (suggested)
- Critical: Remote compromise, key theft, consensus safety break, chain halt via logic flaw.
- High: Authentication bypass, persistent state corruption, signature verification bypass.
- Medium: DoS amplification, memory safety issues without clear exploit, misconfigurations with impact.
- Low: Best-practice gaps, non-exploitable issues.

## Reporting
Require:
- Clear reproduction steps (commands, configs, versions).
- Proof of impact (logs, traces) without sensitive data.
- Suggested fix if possible.

## Triage SLA (recommended)
- Acknowledge: 24 hours
- Triage: 3 business days
- Fix plan: 7 business days (or workaround)
- Patch release: 30 days (faster for critical)

## Response Workflow
1. Intake (security@ / ticket)
2. Validate & reproduce
3. Assign owner and severity
4. Develop patch + tests
5. Rollout (staging -> canary -> prod)
6. Publish advisory + credits (if permitted)

## Disclosure Policy
- Coordinated disclosure default.
- Public disclosure after fix + reasonable window (e.g., 14 days) unless critical.

## Templates
### Acknowledgement
- Thanks, we received your report. We will triage within X days. Reference ID: ...

### Fix Released
- Fixed in version X.Y.Z. Mitigation/workaround. Credits.
