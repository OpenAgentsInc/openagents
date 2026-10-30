#!/usr/bin/env bash
# Platform effects only. All target, readiness, cadence and recovery decisions are Coax.
set -euo pipefail
watch_here=$(cd "$(dirname "$0")" && pwd)
watch_bin=${OA_BACKEND_WATCH_BIN:-$watch_here/backend-watch}
watch_state=${OA_BACKEND_WATCH_STATE:-/var/lib/openagents-backend-watch}
watch_boot_file=${OA_BACKEND_WATCH_BOOT_FILE:-/proc/sys/kernel/random/boot_id}
watch_start=${OA_BACKEND_WATCH_START:-}
watch_end=${OA_BACKEND_WATCH_END:-}
watch_timeout=$(command -v timeout || command -v gtimeout)
clock=$("$watch_bin" clock)
wall=$(jq -er '.wall' <<< "$clock")
if [[ $("$watch_bin" window "$wall" "$watch_start" "$watch_end") != true ]]; then
  printf '{"event":"backend-watch","active":false}\n';exit 0
fi
mkdir -p "$watch_state"
exec 9>"$watch_state/lock"
flock -n 9 || exit 0
work=$(mktemp -d "$watch_state/probe.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
boot=$(cat "$watch_boot_file")
observe() {
  local id=$1 unit=$2 port=$3 prior=$4 invocation started loaded http clock wall mono
  clock=$("$watch_bin" clock);wall=$(jq -er '.wall' <<< "$clock");mono=$(jq -er '.mono' <<< "$clock")
  [[ $("$watch_bin" window "$wall" "$watch_start" "$watch_end") == true ]] || return 75
  invocation=$("$watch_timeout" -k 1 3 systemctl show "$unit" --property=InvocationID --value) || return 1
  started=$("$watch_timeout" -k 1 3 systemctl show "$unit" --property=ExecMainStartTimestampMonotonic --value) || return 1
  loaded=$("$watch_timeout" -k 1 3 systemctl show "$unit" --property=LoadState --value) || return 1
  : > "$work/health"
  http=$(curl --silent --show-error --noproxy '*' --max-time 3 --connect-timeout 1 --max-filesize 4096 --output "$work/health" --write-out '%{http_code}' "http://127.0.0.1:$port/readyz" 2>/dev/null || true)
  "$watch_bin" observe "$id" "$wall" "$mono" "$watch_start" "$watch_end" "$boot" "$invocation" "$started" "$loaded" "$http" "$work/health" "$prior"
}
for id in 0 1 2; do
  description=$("$watch_bin" describe "$id")
  unit=$(jq -er '.unit' <<< "$description");port=$(jq -er '.port' <<< "$description")
  prior="$watch_state/$id.json"
  if ! plan=$(observe "$id" "$unit" "$port" "$prior"); then printf '{"event":"backend-watch","observation_failed":true,"unit_index":%s}\n' "$id";continue;fi
  # Re-read process identity and readiness before a planned restart. The held
  # state is still unchanged, so a healthy/busy response cancels that plan.
  if jq -e '.restart==true' >/dev/null <<< "$plan"; then
    if ! plan=$(observe "$id" "$unit" "$port" "$prior");then continue;fi
  fi
  printf '%s\n' "$plan"
  jq -e '.valid==true' >/dev/null <<< "$plan" || continue
  jq '.state' <<< "$plan" > "$work/state"
  chmod 600 "$work/state";mv "$work/state" "$prior"
  if jq -e '.restart==true' >/dev/null <<< "$plan"; then
    clock=$("$watch_bin" clock);wall=$(jq -er '.wall' <<< "$clock")
    [[ $("$watch_bin" window "$wall" "$watch_start" "$watch_end") == true ]] || continue
    target=$(jq -er '.unit' <<< "$plan")
    "$watch_timeout" -k 2 20 systemctl restart --job-mode=fail -- "$target" || printf '{"event":"backend-watch","restart_failed":true,"unit_index":%s}\n' "$id"
  fi
done
