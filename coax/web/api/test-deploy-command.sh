#!/usr/bin/env bash
# Offline tests for retaining the status of a gcloud deployment command.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=deploy-command.sh
source "$here/deploy-command.sh"
work="$(mktemp -d)"
trap 'find "$work" -depth -delete' EXIT
cat > "$work/fake" <<'MOCK'
#!/usr/bin/env bash
case "$1" in
 failed) printf 'provider refused deployment\n' >&2; exit 42 ;;
 ready) printf 'revision new-revision is ready\n'; exit 0 ;;
 silent) printf 'operation finished\n'; exit 0 ;;
 *) exit 2 ;;
esac
MOCK
chmod +x "$work/fake"
status=0
api_run_deploy "$work/fake" failed >"$work/fail.log" 2>&1 || status=$?
[[ "$status" == 42 ]] || { echo "deployment failure was masked: $status" >&2; exit 1; }
grep -q 'provider refused deployment' "$work/fail.log"
api_run_deploy "$work/fake" ready >"$work/ready.log"
grep -q 'revision new-revision is ready' "$work/ready.log"
api_run_deploy "$work/fake" silent >"$work/silent.log"
status=0
api_run_deploy "$work/missing" >"$work/missing.log" 2>&1 || status=$?
[[ "$status" == 127 ]] || { echo "missing command status was masked: $status" >&2; exit 1; }
mkdir -p "$work/api/migrations" "$work/bin"
cp "$here/deploy.sh" "$here/deploy-command.sh" "$work/api/"
touch "$work/api/fixture.c" "$work/api/fixture.h" "$work/api/Dockerfile" "$work/api/admins.conf" "$work/api/migrations/fixture.sql"
cat > "$work/api/test.sh" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "$work/bin/git" <<'MOCK'
#!/usr/bin/env bash
printf '0123456\n'
MOCK
cat > "$work/bin/gcloud" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$DEPLOY_FIXTURE_LOG"
if [[ ${1:-} == builds && ${2:-} == submit ]]; then exit 0; fi
if [[ ${1:-} == secrets && ${2:-} == describe ]]; then exit 1; fi
if [[ ${1:-} == run && ${2:-} == services && ${3:-} == describe ]]; then exit 1; fi
if [[ ${1:-} == run && ${2:-} == deploy && ${3:-} == oa-api-staging ]]; then
  echo 'simulated deployment failure' >&2
  exit 42
fi
echo 'unexpected cloud command' >&2
exit 91
MOCK
cat > "$work/bin/curl" <<'MOCK'
#!/usr/bin/env bash
touch "$DEPLOY_FIXTURE_CURL"
printf '{"ok":true}\n'
MOCK
chmod +x "$work/api/test.sh" "$work/bin/git" "$work/bin/gcloud" "$work/bin/curl"
status=0
PATH="$work/bin:$PATH" DEPLOY_FIXTURE_LOG="$work/calls" DEPLOY_FIXTURE_CURL="$work/curl-called" \
  bash "$work/api/deploy.sh" >"$work/callsite.log" 2>&1 || status=$?
[[ "$status" != 0 ]] || { echo 'failed staging deploy returned success' >&2; exit 1; }
grep -q '^run deploy oa-api-staging ' "$work/calls" || { echo 'fixture did not reach staging deploy' >&2; exit 1; }
grep -q 'simulated deployment failure' "$work/callsite.log"
[[ ! -e "$work/curl-called" ]] || { echo 'health check ran after failed deploy' >&2; exit 1; }
if grep -q '^run deploy oa-api ' "$work/calls"; then
  echo 'production promotion ran after failed staging deploy' >&2
  exit 1
fi
echo 'deployment command failures propagate; successful output filtering is non-fatal'
