#!/usr/bin/env bash
set -euo pipefail

# Amunchain Layer0 - Enterprise Max Security installer
# Installs: Kyverno, PSA labels, Kyverno policies, NetworkPolicies, then deploys Helm chart.
#
# Requirements:
#   - kubectl configured to target your cluster
#   - helm installed
#   - cluster supports NetworkPolicy (most CNI do)
#
# Usage:
#   ./install.sh -n amunchain -r <release-name> -c helm/nexus-layer0
#
# Optional (Cosign verify policy):
#   export COSIGN_SUBJECT="https://github.com/<ORG>/<REPO>/.github/workflows/security.yml@refs/heads/main"
#
# Optional (Secrets Store CSI):
#   export ENABLE_SECRETS_STORE=true
#   export SECRETS_PROVIDER=vault   # or aws/azure/gcp
#
# Notes:
#   - This script is idempotent.
#   - If you enable strict egress allowlist, your node will have NO outbound internet except DNS
#     until you add explicit allow rules for required endpoints.

NAMESPACE="amunchain"
RELEASE="amunchain"
CHART_PATH="helm/nexus-layer0"
VALUES_FILE="${CHART_PATH}/values.yaml"

KYVERNO_NS="kyverno"
ENABLE_STRICT_EGRESS=true

while getopts ":n:r:c:v:" opt; do
  case $opt in
    n) NAMESPACE="$OPTARG" ;;
    r) RELEASE="$OPTARG" ;;
    c) CHART_PATH="$OPTARG" ;;
    v) VALUES_FILE="$OPTARG" ;;
    *) echo "Usage: $0 [-n namespace] [-r release] [-c chart_path] [-v values_file]" >&2; exit 1 ;;
  esac
done

echo "==> Target namespace: ${NAMESPACE}"
echo "==> Helm release:     ${RELEASE}"
echo "==> Chart path:       ${CHART_PATH}"
echo "==> Values file:      ${VALUES_FILE}"

command -v kubectl >/dev/null 2>&1 || { echo "kubectl not found"; exit 1; }
command -v helm >/dev/null 2>&1 || { echo "helm not found"; exit 1; }

echo "==> Creating namespace (if missing)"
kubectl get ns "${NAMESPACE}" >/dev/null 2>&1 || kubectl create ns "${NAMESPACE}"

echo "==> Enforcing Pod Security Admission: restricted"
kubectl label ns "${NAMESPACE}"   pod-security.kubernetes.io/enforce=restricted   pod-security.kubernetes.io/audit=restricted   pod-security.kubernetes.io/warn=restricted   --overwrite

echo "==> Installing/Upgrading Kyverno"
helm repo add kyverno https://kyverno.github.io/kyverno/ >/dev/null 2>&1 || true
helm repo update >/dev/null 2>&1
kubectl get ns "${KYVERNO_NS}" >/dev/null 2>&1 || kubectl create ns "${KYVERNO_NS}"
helm upgrade --install kyverno kyverno/kyverno -n "${KYVERNO_NS}"

echo "==> Applying Kyverno policies"
kubectl apply -f k8s/kyverno-restricted.yaml
kubectl apply -f k8s/kyverno-seccomp-nonroot.yaml
kubectl apply -f k8s/kyverno-require-explicit-uid-gid.yaml
kubectl apply -f k8s/kyverno-deny-host-namespaces.yaml
kubectl apply -f k8s/kyverno-deny-emptydir-amunchain-data.yaml
kubectl apply -f k8s/kyverno-require-resources.yaml
kubectl apply -f k8s/kyverno-require-probes.yaml
kubectl apply -f k8s/kyverno-deny-latest-tag.yaml
kubectl apply -f k8s/kyverno-deny-nodeport-except-allowed.yaml
kubectl apply -f k8s/kyverno-deny-loadbalancer-except-allowed.yaml
kubectl apply -f k8s/kyverno-deny-hostpath.yaml
kubectl apply -f k8s/kyverno-require-container-securitycontext.yaml
kubectl apply -f k8s/kyverno-require-image-digest.yaml

if [[ -n "${COSIGN_SUBJECT:-}" ]]; then
  echo "==> Enabling verify-signed-images policy with COSIGN_SUBJECT=${COSIGN_SUBJECT}"
  # Patch subject in-place in a temp copy to avoid editing your repo.
  TMP_VERIFY="$(mktemp)"
  sed "s#https://github.com/<ORG>/<REPO>/.github/workflows/security.yml@refs/heads/main#${COSIGN_SUBJECT}#g"     k8s/kyverno-verify-images.yaml > "${TMP_VERIFY}"
  kubectl apply -f "${TMP_VERIFY}"
  rm -f "${TMP_VERIFY}"
else
  echo "==> COSIGN_SUBJECT not set; skipping verify-signed-images policy (recommended for Enterprise)."
fi

echo "==> Applying NetworkPolicy (strict egress allowlist)"
if [[ "${ENABLE_STRICT_EGRESS}" == "true" ]]; then
  kubectl apply -f k8s/amunchain-networkpolicy-max-allpods.yaml
fi

if [[ "${ENABLE_SECRETS_STORE:-false}" == "true" ]]; then
  echo "==> Installing Secrets Store CSI driver (optional)"
  helm repo add secrets-store-csi-driver https://kubernetes-sigs.github.io/secrets-store-csi-driver/charts >/dev/null 2>&1 || true
  helm repo update >/dev/null 2>&1
  kubectl get ns kube-system >/dev/null 2>&1 || true
  helm upgrade --install csi-secrets-store secrets-store-csi-driver/secrets-store-csi-driver -n kube-system

  echo "==> Applying SecretProviderClass template (edit values/provider as needed)"
  if [[ -f k8s/secrets-store/${SECRETS_PROVIDER:-vault}-secretproviderclass.yaml ]]; then
    kubectl apply -n "${NAMESPACE}" -f "k8s/secrets-store/${SECRETS_PROVIDER:-vault}-secretproviderclass.yaml"
  else
    echo "SecretProviderClass not found for provider=${SECRETS_PROVIDER:-vault}. See k8s/secrets-store/README.md"
  fi
fi

echo "==> Deploying Helm chart"
helm upgrade --install "${RELEASE}" "${CHART_PATH}" -n "${NAMESPACE}" -f "${VALUES_FILE}"

echo ""
echo "✅ Enterprise deployment complete."
echo "Next steps:"
echo "  - If you enabled strict egress, add explicit allow rules for any required outbound endpoints."
echo "  - Configure monitoring/alerts (Prometheus + runtime detection)."
echo "  - Enable image signing in CI and set COSIGN_SUBJECT to enforce signed-only images."
