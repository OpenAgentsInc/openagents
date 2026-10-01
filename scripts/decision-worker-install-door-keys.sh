#!/usr/bin/env bash
# Install Jev's fallback door keys on oa-coder-worker-1 and restart the two
# services that read them: decision-worker (its backup doors) and
# coder-worker-chat (its router judge's fallback doors). See
# docs/deployment/decision-worker.md, "Turning the backup doors on".
#
# Reads AI_GATEWAY_API_KEY from ~/work/.secrets/ai-gateway.env and
# OPENROUTER_API_KEY from ~/work/.secrets/openrouter.env (either may be
# missing; that door stays as it is). Copies only those variables, never
# prints them, keeps every other line of each environment file, and prints
# the startup lines that say which doors are on. It never touches
# coder-worker.service or /opt/coder-worker/current.
#
# Usage: scripts/decision-worker-install-door-keys.sh [--dry-run]
set -euo pipefail

SECRETS="${OPENAGENTS_SECRETS_DIR:-$HOME/work/.secrets}"
GCLOUD_CONFIG="${CLOUDSDK_CONFIG:-$SECRETS/gcloud-sa-config}"
VM="oa-coder-worker-1"
ZONE="us-central1-a"
PROJECT="openagentsgemini"
DRY_RUN=0
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=1

# One variable's line from one secrets file, or nothing.
line_of() {
    local file="$1" name="$2"
    [[ -f "$file" ]] || return 0
    grep -E "^(export +)?${name}=" "$file" | tail -n 1 | sed -E 's/^export +//' || true
}

staged="$(mktemp -d)"
trap 'rm -rf "$staged"' EXIT
chmod 700 "$staged"
keys="$staged/door-keys.env"
: >"$keys"
chmod 600 "$keys"

found=()
for pair in "ai-gateway.env:AI_GATEWAY_API_KEY" "openrouter.env:OPENROUTER_API_KEY"; do
    file="$SECRETS/${pair%%:*}"
    name="${pair##*:}"
    line="$(line_of "$file" "$name")"
    value="${line#*=}"
    value="${value%\"}"; value="${value#\"}"
    if [[ -n "$line" && -n "${value// /}" ]]; then
        printf '%s=%s\n' "$name" "$value" >>"$keys"
        found+=("$name")
    else
        echo "no $name in $file: that door is left as it is"
    fi
done
if [[ ${#found[@]} -eq 0 ]]; then
    echo "no door keys to install" >&2
    exit 1
fi
echo "installing ${found[*]} on $VM (values not shown)"
if [[ $DRY_RUN -eq 1 ]]; then
    echo "dry run: nothing copied"
    exit 0
fi

remote_script="$staged/install.sh"
cat >"$remote_script" <<'REMOTE'
set -euo pipefail
keys="$HOME/door-keys/door-keys.env"
for target in /etc/decision-worker/decision-worker.env /etc/coder-worker/coder-worker-chat.env; do
    sudo test -f "$target" || { echo "missing $target"; exit 1; }
    sudo cp -p "$target" "$target.bak-$(date -u +%Y%m%dT%H%M%SZ)"
    while IFS= read -r line; do
        name="${line%%=*}"
        sudo sed -i -E "/^(export +)?${name}=/d" "$target"
        printf '%s\n' "$line" | sudo tee -a "$target" >/dev/null
    done <"$keys"
    sudo chmod 600 "$target"
done
rm -rf "$HOME/door-keys"
sudo systemctl restart decision-worker
sudo systemctl restart coder-worker-chat
sleep 5
sudo journalctl -u decision-worker -n 30 --no-pager -o cat | grep -E "backup door|upstream" || true
sudo journalctl -u coder-worker-chat -n 60 --no-pager -o cat | grep -E "^judge" || true
REMOTE

gc() { CLOUDSDK_CONFIG="$GCLOUD_CONFIG" gcloud "$@"; }
gc compute ssh "$VM" --zone "$ZONE" --project "$PROJECT" --tunnel-through-iap \
    --command 'mkdir -p -m 700 "$HOME/door-keys"'
gc compute scp --zone "$ZONE" --project "$PROJECT" --tunnel-through-iap \
    "$keys" "$remote_script" "$VM:door-keys/"
gc compute ssh "$VM" --zone "$ZONE" --project "$PROJECT" --tunnel-through-iap \
    --command 'chmod 600 "$HOME/door-keys/door-keys.env"; bash "$HOME/door-keys/install.sh"'
