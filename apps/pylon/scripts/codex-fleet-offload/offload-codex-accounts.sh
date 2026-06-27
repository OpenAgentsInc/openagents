#!/usr/bin/env bash
#
# Copy selected isolated Codex account homes to a Tailnet Mac and optionally
# launch the Codex supervisor there. This is intentionally account-ref scoped:
# it never reads or copies ~/.codex, and it never runs a device login.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

LOCAL_PYLON_HOME="${PYLON_HOME:-$HOME/.pylon-fable}"
REMOTE_PYLON_HOME='~/.pylon-fable'
REMOTE_REPO='~/work/openagents'
REMOTE_TOKEN_ENV='~/work/.secrets/openagents-artanis-agent.env'
SUP_MAX_SLOTS=4
SUP_PER_ACCOUNT=2
EXECUTE=0
START_SUPERVISOR=0
HOST=""
ACCOUNTS=()

usage() {
  cat <<'USAGE'
usage:
  offload-codex-accounts.sh --host <tailnet-host> --accounts <ref,ref...> [options] --execute

options:
  --local-pylon-home <path>    Source Pylon home. Default: $PYLON_HOME or ~/.pylon-fable.
  --remote-pylon-home <path>   Destination Pylon home. Default: ~/.pylon-fable.
  --remote-repo <path>         Remote openagents checkout. Default: ~/work/openagents.
  --remote-token-env <path>    Remote env file that exports OPENAGENTS_AGENT_TOKEN.
                               Default: ~/work/.secrets/openagents-artanis-agent.env.
  --sup-max-slots <n>          Remote SUP_MAX_SLOTS when launching. Default: 4.
  --sup-per-account <n>        Remote SUP_PER_ACCOUNT when launching. Default: 2.
  --start-supervisor           Start remote codex supervisor after import.
  --execute                    Required to copy credentials or launch anything.
  --dry-run                    Print the plan only.

example:
  PYLON_HOME=$HOME/.pylon-fable \
    apps/pylon/scripts/codex-fleet-offload/offload-codex-accounts.sh \
      --host imac-pro-bertha \
      --accounts codex-4,codex-5 \
      --start-supervisor \
      --execute

safety:
  Only <pylon home>/accounts/codex/<ref> is copied. The default ~/.codex home is
  never read, copied, or logged into. Agent tokens are not copied; the remote
  supervisor must source its own token env file.
USAGE
}

fail() {
  printf 'FATAL: %s\n' "$*" >&2
  exit 1
}

shell_quote() {
  printf "%q" "$1"
}

parse_accounts() {
  local raw="$1"
  IFS=',' read -r -a ACCOUNTS <<< "$raw"
  local account
  for account in "${ACCOUNTS[@]}"; do
    [[ "$account" =~ ^[A-Za-z0-9._-]+$ ]] || fail "invalid account ref: $account"
  done
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --host) HOST="${2:-}"; shift 2 ;;
    --accounts) parse_accounts "${2:-}"; shift 2 ;;
    --local-pylon-home) LOCAL_PYLON_HOME="${2:-}"; shift 2 ;;
    --remote-pylon-home) REMOTE_PYLON_HOME="${2:-}"; shift 2 ;;
    --remote-repo) REMOTE_REPO="${2:-}"; shift 2 ;;
    --remote-token-env) REMOTE_TOKEN_ENV="${2:-}"; shift 2 ;;
    --sup-max-slots) SUP_MAX_SLOTS="${2:-}"; shift 2 ;;
    --sup-per-account) SUP_PER_ACCOUNT="${2:-}"; shift 2 ;;
    --start-supervisor) START_SUPERVISOR=1; shift ;;
    --execute) EXECUTE=1; shift ;;
    --dry-run) EXECUTE=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown option: $1" ;;
  esac
done

[ -n "$HOST" ] || fail "--host is required"
[ "${#ACCOUNTS[@]}" -gt 0 ] || fail "--accounts is required"
[[ "$SUP_MAX_SLOTS" =~ ^[0-9]+$ ]] || fail "--sup-max-slots must be an integer"
[[ "$SUP_PER_ACCOUNT" =~ ^[0-9]+$ ]] || fail "--sup-per-account must be an integer"

LOCAL_PYLON_HOME="${LOCAL_PYLON_HOME/#\~/$HOME}"
LOCAL_CODEX_ROOT="$LOCAL_PYLON_HOME/accounts/codex"

[ "$LOCAL_CODEX_ROOT" != "$HOME/.codex" ] || fail "refusing to use ~/.codex"
[ -d "$LOCAL_CODEX_ROOT" ] || fail "missing local Codex account root: $LOCAL_CODEX_ROOT"

for account in "${ACCOUNTS[@]}"; do
  account_home="$LOCAL_CODEX_ROOT/$account"
  [ -d "$account_home" ] || fail "missing local account home: $account_home"
  [ "$account_home" != "$HOME/.codex" ] || fail "refusing to copy ~/.codex"
  [ -f "$account_home/auth.json" ] || fail "missing auth.json for $account"
done

printf 'Codex account offload plan\n'
printf '  host: %s\n' "$HOST"
printf '  local pylon home: %s\n' "$LOCAL_PYLON_HOME"
printf '  remote pylon home: %s\n' "$REMOTE_PYLON_HOME"
printf '  remote repo: %s\n' "$REMOTE_REPO"
printf '  accounts: %s\n' "${ACCOUNTS[*]}"
printf '  start supervisor: %s\n' "$START_SUPERVISOR"

if [ "$EXECUTE" -ne 1 ]; then
  printf '\nDry run only. Re-run with --execute to copy account homes.\n'
  exit 0
fi

tmpdir="$(mktemp -d)"
archive="$tmpdir/codex-account-offload.tgz"
remote_archive="/tmp/openagents-codex-account-offload-$(date +%s)-$$.tgz"
cleanup() { rm -rf "$tmpdir"; }
trap cleanup EXIT

tar_args=()
for account in "${ACCOUNTS[@]}"; do
  tar_args+=("$account")
done

printf '\nCreating local archive with selected isolated account homes...\n'
tar -C "$LOCAL_CODEX_ROOT" -czf "$archive" "${tar_args[@]}"
chmod 600 "$archive"

printf 'Copying archive to %s:%s ...\n' "$HOST" "$remote_archive"
scp -q "$archive" "$HOST:$remote_archive"

remote_account_args=""
for account in "${ACCOUNTS[@]}"; do
  remote_account_args+=" $(shell_quote "$account")"
done

remote_script=$(cat <<REMOTE
set -euo pipefail
REMOTE_PYLON_HOME=$REMOTE_PYLON_HOME
REMOTE_REPO=$REMOTE_REPO
REMOTE_ARCHIVE=$(shell_quote "$remote_archive")
mkdir -p "\$REMOTE_PYLON_HOME/accounts/codex"
tar -C "\$REMOTE_PYLON_HOME/accounts/codex" -xzf "\$REMOTE_ARCHIVE"
chmod -R go-rwx "\$REMOTE_PYLON_HOME/accounts/codex"
rm -f "\$REMOTE_ARCHIVE"
cd "\$REMOTE_REPO"
for account in$remote_account_args; do
  home="\$REMOTE_PYLON_HOME/accounts/codex/\$account"
  PYLON_HOME="\$REMOTE_PYLON_HOME" bun apps/pylon/src/index.ts accounts connect codex \
    --account "\$account" \
    --home "\$home" \
    --skip-device-login \
    --json >/dev/null
done
PYLON_HOME="\$REMOTE_PYLON_HOME" bun apps/pylon/src/index.ts codex accounts list --json
REMOTE
)

if [ "$START_SUPERVISOR" -eq 1 ]; then
  remote_script+=$(cat <<REMOTE

set -a
[ -f $REMOTE_TOKEN_ENV ] || { echo "missing remote token env: $REMOTE_TOKEN_ENV" >&2; exit 1; }
. $REMOTE_TOKEN_ENV
set +a
live_ref=\$(PYLON_HOME="\$REMOTE_PYLON_HOME" bun apps/pylon/src/index.ts provider go-online --json \
  | node -e 'let s="";process.stdin.on("data",c=>s+=c);process.stdin.on("end",()=>{const j=JSON.parse(s); console.log(j.pylonRef || "")})')
[ -n "\$live_ref" ] || { echo "could not resolve live pylon ref" >&2; exit 1; }
PYLON_HOME="\$REMOTE_PYLON_HOME" \
SUP_PYLON_REF="\$live_ref" \
SUP_MAX_SLOTS=$SUP_MAX_SLOTS \
SUP_PER_ACCOUNT=$SUP_PER_ACCOUNT \
bash apps/pylon/scripts/codex-supervisor/launch.sh start
REMOTE
)
fi

printf 'Importing on remote host...\n'
ssh "$HOST" "bash -s" <<< "$remote_script"

printf '\nOffload complete for %s: %s\n' "$HOST" "${ACCOUNTS[*]}"
