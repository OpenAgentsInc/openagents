#!/usr/bin/env bash
set -euo pipefail

moltbook_api_key() {
  if [[ -n "${MOLTBOOK_API_KEY:-}" ]]; then
    printf '%s' "$MOLTBOOK_API_KEY"
    return 0
  fi

  python3 - <<'PY'
import json, os
path = os.path.expanduser('~/.config/moltbook/credentials.json')
with open(path, 'r', encoding='utf-8') as f:
    data = json.load(f)
print(data['api_key'])
PY
}
