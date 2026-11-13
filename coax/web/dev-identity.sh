#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: asks the openagents CLI for a profile's public key)
# Who a dev server on this computer logs you in as (coax/web/serve.sh, coax/web/api/test.sh --serve):
# your `openagents` login, so the page, the CLI and Claude's realm tools on this computer are one
# account, the same on every restart.
#   coax/web/dev-identity.sh    prints "PROFILE PUBKEY" (hex), or nothing and exit 1 with no login here
# The profile is the first with a key of OA_SERVE_PROFILE, OPENAGENTS_PROFILE (the CLI's own choice),
# "default" and "admin". It reads only public keys (`openagents nostr show`); no secret leaves the CLI.
# The CLI: OA_SERVE_CLI, else ~/.openagents/bin/openagents, else this checkout's build.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cli="${OA_SERVE_CLI:-$HOME/.openagents/bin/openagents}"
[[ -x "$cli" ]] || cli="$here/../../apps/openagents-cli/build/openagents"
[[ -x "$cli" ]] || exit 1
for p in "${OA_SERVE_PROFILE:-}" "${OPENAGENTS_PROFILE:-}" default admin; do
  [[ "$p" =~ ^[a-z0-9_-]{1,32}$ ]] || continue
  pk="$(BROWSER=/usr/bin/true "$cli" --profile "$p" nostr show --json 2>/dev/null </dev/null |
        python3 -I -c 'import json, sys; d = json.load(sys.stdin); print(d.get("pubkey", "") if d.get("ok") else "")' 2>/dev/null || true)"
  if [[ "$pk" =~ ^[0-9a-f]{64}$ ]]; then printf '%s %s\n' "$p" "$pk"; exit 0; fi
done
exit 1
