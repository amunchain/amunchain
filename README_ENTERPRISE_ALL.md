# Amunchain Layer0 — Enterprise Max Security Bundle

This bundle includes:
- Strict CI supply-chain gates (cargo-deny/audit + SBOM)
- Docker hardening and Helm hardening
- Kyverno admission policies (restricted runtime + optional signed-images enforcement)
- NetworkPolicies (max security: DNS-only egress by default)
- Enterprise runbooks and bug bounty playbook
- One-command installer: ./install.sh

## Quickstart
1) Configure kubectl + helm
2) (Recommended) Enable image signing in CI and export COSIGN_SUBJECT
3) Run:
   ./install.sh -n amunchain -r amunchain -c helm/nexus-layer0

## Notes
- Strict egress may block outbound traffic; use k8s/amunchain-networkpolicy-egress-allowlist.yaml to allow specific endpoints.
- For Secrets Store CSI, set:
  ENABLE_SECRETS_STORE=true SECRETS_PROVIDER=vault ./install.sh


## Enterprise admission policies added (Service & Resources)

This bundle enforces additional Kyverno policies:
- **Containers must set `resources.requests` and `resources.limits`** (cpu & memory).
- **All containers must have `readinessProbe` and `livenessProbe`.**
- **Image tag `latest` is forbidden** (use a pinned tag or `@sha256:` digest).
- **`Service` type `NodePort` is forbidden** unless labeled `security.amunchain/allow-nodeport: "true"`.
- **`Service` type `LoadBalancer` is forbidden** unless labeled `security.amunchain/allow-loadbalancer: "true"`.


## Additional Enterprise Policies (added)
- `k8s/kyverno-require-explicit-uid-gid.yaml` – require explicit runAsUser/runAsGroup (non-root)
- `k8s/kyverno-deny-host-namespaces.yaml` – forbid hostNetwork/hostPID/hostIPC
- `k8s/kyverno-deny-emptydir-amunchain-data.yaml` – forbid emptyDir for `amunchain-data` volume (PVC required)


## 🔒 قفل زيادة (Enterprise Extra Lock)

تمت إضافة سياسات Kyverno إضافية لتشديد الحماية أكثر:

- `k8s/kyverno-deny-hostpath.yaml` منع أي `hostPath` volumes نهائيًا.
- `k8s/kyverno-require-container-securitycontext.yaml` إلزام `securityContext` الصارم لكل container/initContainer (no escalation + read-only + drop ALL + runAsNonRoot).
- `k8s/kyverno-require-image-digest.yaml` إلزام تشغيل الصور عبر Digest (`image@sha256:...`).  
  - **استثناء مقصود:** إذا كانت قيمة `image.digest` فارغة في Helm، سيضيف الـ chart label:
    `security.amunchain/allow-image-tag: "true"` لكي لا ينكسر النشر.  
  - للـ Enterprise الحقيقي: ضع `image.digest` (sha256) لإزالة هذا الاستثناء تلقائيًا.

