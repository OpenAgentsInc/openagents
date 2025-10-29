#!/usr/bin/env bash
set -euo pipefail

# Triage open GitHub issues and close ones about legacy tunneling (cloudburrow/cloudflare/bore)
# Usage:
#   scripts/triage-issues-tailscale.sh [--repo owner/name] [--apply]
#
# Notes:
# - Requires GitHub CLI (gh) and authentication: gh auth status
# - By default, runs in dry-run mode and prints matching issues.

REPO=""
APPLY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$2"; shift 2 ;;
    --apply)
      APPLY=1; shift ;;
    *)
      echo "Unknown arg: $1" >&2; exit 2 ;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  echo "error: gh CLI is required (https://cli.github.com)" >&2
  exit 1
fi

# Detect repo when not provided
if [[ -z "$REPO" ]]; then
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    url=$(git config --get remote.origin.url || echo "")
    if [[ "$url" =~ github.com[:/](.+/.+)(\.git)?$ ]]; then
      REPO="${BASH_REMATCH[1]}"
    fi
  fi
fi
if [[ -z "$REPO" ]]; then
  echo "error: could not detect repo; pass --repo owner/name" >&2
  exit 1
fi

echo "[triage] repo=$REPO (dry-run=$((1-APPLY)))" >&2

# Fetch open issues (first 200)
mapfile -t issues < <(gh issue list -R "$REPO" --state open --limit 200 --json number,title --jq '.[] | [.number, .title] | @tsv')

if [[ ${#issues[@]} -eq 0 ]]; then
  echo "[triage] no open issues found" >&2
  exit 0
fi

match_any() {
  local text="$1"
  shopt -s nocasematch
  if [[ "$text" =~ cloudburrow|cloudflare|bore\.pub|\bngrok\b|\btunnel\b|oa-tunnel ]]; then
    shopt -u nocasematch
    return 0
  fi
  shopt -u nocasematch
  return 1
}

comment_body=$(cat <<'EOF'
We’re moving to a Tailscale‑first approach for remote connectivity.

Summary:
- No Cloudflare/Cloudburrow/bore tunnels in this flow.
- Desktop runs the bridge locally (cargo bridge) and exposes ws:// on your Tailnet IP.
- Mobile connects directly to that Tailscale IP (e.g., 100.x.x.x) on port 8787.

If this issue was specifically about cloudburrow/cloudflare/bore, we’re closing it in favor of the Tailscale flow. If there’s a remaining gap that Tailscale doesn’t cover, please comment and we can reconsider or file a focused follow‑up.

Relevant helper:
- `npx tricoder` now detects Tailscale, prints your Desktop IP, and can launch the bridge: `tricoder --run-bridge`.
EOF
)

closed_any=0
for row in "${issues[@]}"; do
  num=${row%%$'\t'*}
  title=${row#*$'\t'}
  # Load body for better matching
  body=$(gh issue view -R "$REPO" "$num" --json body --jq .body || echo "")
  text="$title
$body"
  if match_any "$text"; then
    echo "[triage] matches #$num: $title" >&2
    if [[ $APPLY -eq 1 ]]; then
      gh issue comment -R "$REPO" "$num" --body "$comment_body" >/dev/null
      gh issue close -R "$REPO" "$num" >/dev/null
      echo "[triage] closed #$num" >&2
      closed_any=1
    fi
  fi
done

if [[ $APPLY -eq 0 ]]; then
  echo "[triage] dry run complete. Re-run with --apply to comment & close." >&2
else
  if [[ $closed_any -eq 1 ]]; then
    echo "[triage] completed; matching issues were commented on and closed." >&2
  else
    echo "[triage] no matching issues to close." >&2
  fi
fi

exit 0

