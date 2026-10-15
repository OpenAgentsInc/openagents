#!/usr/bin/env bash
# Run on the disposable host as space-spike, after install and publication.
set -euo pipefail
source "$HOME/space.env"
export PATH="$HOME/.local/bin:$PATH"
export SPACE_TEMPLATE="oa-coder-runtime@$(jq -er .content_hash "$HOME/evidence/runtime-manifest.json")"
mkdir -p "$HOME/evidence"
case ${1:-} in
  prepare-cold|prepare-snapshot|inspect)
    export SPACE_SPIKE_ACTION=$1
    python3 - <<'PY'
import json, os, pathlib, urllib.request

root = os.environ['SPACE_API_URL'] + '/admin/api/v1/templates/oa-coder-runtime'
evidence = pathlib.Path.home() / 'evidence'
action = os.environ['SPACE_SPIKE_ACTION']

def call(path='', method='GET', body=None):
    request = urllib.request.Request(root + path, method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={'Authorization': 'Bearer ' + os.environ['SPACE_API_TOKEN'],
                 'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)

if action != 'inspect':
    call(method='PATCH', body={'network_policy': {'version': 'v1', 'egress': {
        'default_action': 'deny', 'allow': [{'domain': 'api.github.com'}], 'deny': []}}})
    policy = call('/base-snapshot-policy', 'PATCH', {
        'ensure_base_snapshot': action == 'prepare-snapshot',
        'snapshot_vcpu': 2, 'snapshot_memory_mib': 1024})
    result = call('/base-snapshot-policy:reconcile', 'POST', {})
    (evidence / (action + '.json')).write_text(json.dumps({
        'policy': policy, 'reconcile': result}, indent=2) + '\n')
template = call()
(evidence / (action + '-template.json')).write_text(json.dumps(template, indent=2) + '\n')
print(json.dumps(template, indent=2))
PY
    ;;
  conformance)
    exec computer-conformance --space >"$HOME/evidence/conformance.txt" 2>&1
    ;;
  cold|snapshot)
    # The operator sets the base snapshot policy for the chosen arm and
    # retains backend observations. The arm name is not proof of boot mode.
    export SPACE_EXPECT_GITHUB_ONLY=1
    export SPACE_BOOT_OBSERVER="$HOME/boot-mode.py"
    exec space-spike "$HOME/evidence/$1.json" "$1" >"$HOME/evidence/$1.log" 2>&1
    ;;
  *) echo 'Usage: check.sh conformance|prepare-cold|prepare-snapshot|inspect|cold|snapshot' >&2; exit 2 ;;
esac
