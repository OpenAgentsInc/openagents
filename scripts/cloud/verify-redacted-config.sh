#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

state_dir="$(mktemp -d)"
fixture_dir="$(mktemp -d)"
trap "rm -rf '$state_dir' '$fixture_dir'" EXIT

write_fixture() {
  local kind="$1"
  local path="${fixture_dir}/${kind}.txt"
  case "$kind" in
    env)
      cat >"$path" <<'EOF'
OPENAGENTS_FAKE_SECRET_OK=1
OPENAI_API_KEY=sk-fake-redaction-test
CODEX_AUTH_JSON={"access_token":"secret-token-redaction-test"}
OPENAGENTS_CODEX_PROVIDER_SECRET_REF=gcp-secret://codex/account/dev
EOF
      ;;
    url)
      cat >"$path" <<'EOF'
OPENAGENTS_FAKE_SECRET_OK=1
https://api.example.invalid/v1/run?token=secret-token-redaction-test&api_key=sk-fake-redaction-test
EOF
      ;;
    headers)
      cat >"$path" <<'EOF'
OPENAGENTS_FAKE_SECRET_OK=1
Authorization: Bearer secret-token-redaction-test
X-API-Key: sk-fake-redaction-test
EOF
      ;;
    config)
      cat >"$path" <<'EOF'
OPENAGENTS_FAKE_SECRET_OK=1
[provider.codex]
secret_ref = "gcp-secret://codex/account/dev"
raw_token = "secret-token-redaction-test"
api_key = "sk-fake-redaction-test"
EOF
      ;;
    log)
      cat >"$path" <<'EOF'
OPENAGENTS_FAKE_SECRET_OK=1
worker booted with Authorization: Bearer secret-token-redaction-test and api_key=sk-fake-redaction-test
EOF
      ;;
    receipt)
      cat >"$path" <<'EOF'
OPENAGENTS_FAKE_SECRET_OK=1
{"receipt_id":"receipt.fake","provider_secret_ref":"gcp-secret://codex/account/dev","raw_token":"secret-token-redaction-test","api_key":"sk-fake-redaction-test"}
EOF
      ;;
    *)
      echo "unknown fixture kind: $kind" >&2
      exit 2
      ;;
  esac
  printf '%s\n' "$path"
}

assert_no_secret_markers() {
  local label="$1"
  local path="$2"
  local lower
  lower="$(tr '[:upper:]' '[:lower:]' <"$path")"
  for marker in \
    "secret-token-redaction-test" \
    "sk-fake-redaction-test" \
    "bearer secret-token" \
    "raw_token =" \
    "access_token" \
    "api_key=sk-" \
    "x-api-key: sk-"; do
    if grep -Fq "$marker" <<<"$lower"; then
      echo "${label} leaked marker ${marker} in ${path}" >&2
      exit 1
    fi
  done
}

for template in \
  config/oa-node.env.example \
  config/oa-workroomd.env.example \
  config/gcp-node.env.example; do
  assert_no_secret_markers "tracked template" "$template"
done

for kind in env url headers config log receipt; do
  input="$(write_fixture "$kind")"
  cargo run -q -p oa-node -- broker redact \
    --kind "$kind" \
    --input "$input" \
    --state-dir "$state_dir" \
    --json >/dev/null
done

find "$state_dir/broker-redacted-artifacts" -type f -print0 |
  while IFS= read -r -d '' artifact; do
    assert_no_secret_markers "redacted artifact" "$artifact"
  done

assert_no_secret_markers "redaction receipt log" "$state_dir/broker-redaction-receipts.jsonl"

echo "redacted config verification passed"
