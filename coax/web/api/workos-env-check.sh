#!/usr/bin/env bash
# Firm reason: 5 (thin validation wrapper around gcloud secrets, curl JWKS, and /api/auth/workos/config).
# Validates a provisioned WorkOS environment (Phase 0, #14097):
#   coax/web/api/workos-env-check.sh --staging
#   coax/web/api/workos-env-check.sh --production
#   coax/web/api/workos-env-check.sh --local API_ORIGIN WORKOS_BASE CLIENT_ID [--expect-gamebuilder 0|1]
set -euo pipefail

project="${OA_GCP_PROJECT:-openagentsgemini}"
runtime_sa="${OA_API_SA:-157437760789-compute@developer.gserviceaccount.com}"
mode="${1:-}"
expect_gb=0

check_jwks_and_config() {
  local api_origin="$1" wos_base="$2" client_id="$3" exp_gb="$4"
  echo "==> Checking WorkOS JWKS: ${wos_base}/sso/jwks/${client_id}"
  jwks="$(curl -fsS "${wos_base}/sso/jwks/${client_id}")"
  python3 - "$jwks" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
keys = d.get("keys") or []
assert len(keys) >= 1, f"JWKS has no keys: {d}"
rsa = [k for k in keys if k.get("kty") == "RSA" and k.get("n") and k.get("e")]
assert rsa, f"JWKS has no RSA keys: {d}"
print(f"  ok: JWKS returned {len(rsa)} RSA key(s) (kid={rsa[0].get('kid','')})")
PY

  echo "==> Checking API WorkOS config: ${api_origin}/api/auth/workos/config"
  cfg="$(curl -fsS -H "Origin: ${api_origin}" "${api_origin}/api/auth/workos/config")"
  python3 - "$cfg" "$client_id" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
cid = sys.argv[2]
assert d.get("enabled") is True, f"WorkOS not enabled: {d}"
assert d.get("client_id") == cid, f"client_id mismatch: got {d.get('client_id')} want {cid}"
provs = set(d.get("providers") or [])
for req in ("magic", "password", "google", "github", "apple", "sso"):
    assert req in provs, f"missing provider {req} in {provs}"
print(f"  ok: /api/auth/workos/config enabled=true client_id={cid} providers={sorted(provs)}")
PY

  echo "==> Checking gamebuilder.org isolation (expect enabled=${exp_gb})"
  gb_out="$(curl -sS -w '\n%{http_code}' -H "Origin: https://gamebuilder.org" "${api_origin}/api/auth/workos/config")"
  gb_code="$(printf '%s\n' "$gb_out" | tail -n 1)"
  gb_body="$(printf '%s\n' "$gb_out" | sed '$d')"
  python3 - "$gb_code" "$gb_body" "$exp_gb" <<'PY'
import json, sys
code, body, exp = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])
if exp == 0:
    assert code == 403 or (code == 200 and json.loads(body).get("enabled") is False), (code, body)
    print(f"  ok: gamebuilder.org isolated (http {code})")
else:
    assert code == 200 and json.loads(body).get("enabled") is True, (code, body)
    print("  ok: gamebuilder.org WorkOS enabled")
PY
}

check_secrets() {
  local sfx="$1"
  local names=(
    "oa-workos-api-key${sfx}"
    "oa-workos-client-id${sfx}"
    "oa-workos-webhook-secret${sfx}"
    "oa-workos-action-secret${sfx}"
  )
  echo "==> Checking GCP Secret Manager bindings (project=${project}, runtime_sa=${runtime_sa})"
  for name in "${names[@]}"; do
    gcloud secrets describe "$name" --project "$project" >/dev/null || {
      echo "MISSING secret: $name in project $project" >&2; exit 1
    }
    pol="$(gcloud secrets get-iam-policy "$name" --project "$project" --format=json)"
    python3 - "$name" "$runtime_sa" "$pol" <<'PY'
import json, sys
name, sa, pol = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
bindings = pol.get("bindings") or []
accessors = []
for b in bindings:
    if b.get("role") == "roles/secretmanager.secretAccessor":
        accessors.extend(b.get("members") or [])
want = f"serviceAccount:{sa}"
assert want in accessors, f"{name}: runtime SA {want} missing secretAccessor (have {accessors})"
extra = [m for m in accessors if m != want]
assert not extra, f"{name}: unexpected extra secretAccessor members: {extra}"
print(f"  ok: secret {name} exists and is bound strictly to {want}")
PY
  done
}

case "$mode" in
  --staging)
    check_secrets "-staging"
    cid="$(gcloud secrets versions access latest --secret=oa-workos-client-id-staging --project "$project")"
    check_jwks_and_config "https://staging.openagents.com" "${OA_WORKOS_BASE_URL:-https://api.workos.com}" "$cid" "$expect_gb"
    ;;
  --production)
    check_secrets ""
    cid="$(gcloud secrets versions access latest --secret=oa-workos-client-id --project "$project")"
    check_jwks_and_config "https://openagents.com" "${OA_WORKOS_BASE_URL:-https://api.workos.com}" "$cid" "$expect_gb"
    ;;
  --local)
    api_origin="${2:?usage: --local API_ORIGIN WORKOS_BASE CLIENT_ID [--expect-gamebuilder 0|1]}"
    wos_base="${3:?usage: --local API_ORIGIN WORKOS_BASE CLIENT_ID}"
    cid="${4:?usage: --local API_ORIGIN WORKOS_BASE CLIENT_ID}"
    if [[ "${5:-}" == "--expect-gamebuilder" ]]; then expect_gb="${6:-0}"; fi
    check_jwks_and_config "$api_origin" "$wos_base" "$cid" "$expect_gb"
    ;;
  *)
    echo "usage: $0 --staging | --production | --local API_ORIGIN WORKOS_BASE CLIENT_ID [--expect-gamebuilder 0|1]" >&2
    exit 2
    ;;
esac
echo "==> workos-env-check.sh PASSED"
