#!/usr/bin/env bash
#
# Measure an `oa-coder-host` image on a fresh VM, the way a pool host would
# start from it.
#
#   scripts/cloud/measure-coder-host-image.sh [IMAGE] [--machine TYPE] [--rev REV]
#
# IMAGE defaults to the newest image in family oa-coder-host; TYPE to
# c3-standard-8 (the pool host shape); REV, the commit the builds check out,
# to the freshly fetched origin/main (the image's own commit is HEAD). The VM is spot, has no external
# address, and is deleted on exit. Its startup script prints, as user coder:
#   ready              boot to OA_CODER_HOST_READY (measured here, from create)
#   warm_clone         cargo build -p openagents-cli in the baked clone after
#                      checking out the fetched origin/main, on the warm slot
#   warm_tests_clone   its test targets there (cargo build --tests)
#   warm_worktree      the same build in a fresh `git worktree` of origin/main
#                      on the warm slot (a Coder task's path)
#   edit_rebuild       one edit to openagents-cli's main.rs, then the build
#   cold_sccache       the same build into an empty target, sccache on
#   cold_no_sccache    the same build into an empty target, sccache off
# and the result is one JSON object on stdout.
set -euo pipefail

PROJECT="${OA_PROJECT:-openagentsgemini}"
ZONES="${OA_ZONES:-us-central1-a us-central1-b us-central1-c us-central1-f}"
ZONE="${ZONES%% *}"
HOST_SA="oa-coder-host@${PROJECT}.iam.gserviceaccount.com"
machine="c3-standard-8"
image=""
rev="origin/main"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --machine) machine="${2:?}"; shift 2 ;;
    --rev) rev="${2:?}"; shift 2 ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) image="$1"; shift ;;
  esac
done
g() { gcloud --project "$PROJECT" --quiet "$@"; }
say() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
if [[ -z "$image" ]]; then
  image="$(g compute images describe-from-family oa-coder-host --format='value(name)')"
fi
vm="oa-coder-host-measure-$(date -u +%Y%m%d%H%M%S)"
trap 'g compute instances delete "$vm" --zone "$ZONE" >/dev/null 2>&1 || true' EXIT
# (ZONE is set to the zone the VM was created in.)

script="$(mktemp)"
cat >"$script" <<'GUEST'
#!/bin/bash
set -uo pipefail
out() { echo "OA_CODER_HOST_MEASURE $*" >/dev/ttyS0; }
until [[ -e /run/oa-coder-host/ready ]]; do sleep 1; done
manifest=/home/coder/.openagents/coder-host.json
slot="$(jq -r .warm_target.slot "$manifest")"
as_coder() { runuser -u coder -- env HOME=/home/coder PATH=/home/coder/.cargo/bin:/usr/local/bin:/usr/bin:/bin "$@"; }
timed() {
  local name="$1"; shift
  local t0 t1 rc
  t0="$(date +%s.%N)"
  as_coder bash -c "$*" >"/var/tmp/measure-$name.log" 2>&1
  rc=$?
  t1="$(date +%s.%N)"
  if [[ $rc != 0 ]]; then
    grep -E '^(error|warning: build failed)|-->' "/var/tmp/measure-$name.log" | head -6 | while IFS= read -r l; do out "$name log $l"; done
  fi
  out "$name seconds=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}') rc=$rc"
}
repo=/home/coder/openagents
gib() { awk '{s+=$1} END {printf "%.1f", s/1073741824}'; }
out "sizes slot_gib=$(du -sb "$slot" | cut -f1 | gib) incremental_gib=$(du -sb "$slot/debug/incremental" 2>/dev/null | cut -f1 | gib) build_scripts_gib=$(du -sb "$slot/debug/build" 2>/dev/null | cut -f1 | gib) deps_executables_gib=$(find "$slot/debug/deps" -maxdepth 1 -type f -executable ! -name '*.so' -printf '%s\n' | gib) deps_rlib_gib=$(find "$slot/debug/deps" -maxdepth 1 -name '*.rlib' -printf '%s\n' | gib) deps_rmeta_gib=$(find "$slot/debug/deps" -maxdepth 1 -name '*.rmeta' -printf '%s\n' | gib) bin_gib=$(find "$slot/debug" -maxdepth 1 -type f -printf '%s\n' | gib)"
behind="$(as_coder git -C $repo rev-list --count HEAD..REV_PLACEHOLDER)"
out "baked_behind_rev commits=$behind"
timed warm_clone "cd $repo && git checkout -q --detach REV_PLACEHOLDER && CARGO_TARGET_DIR=$slot cargo build --locked -p openagents-cli"
timed warm_tests_clone "cd $repo && CARGO_TARGET_DIR=$slot cargo build --locked --keep-going --tests -p openagents-cli"
timed worktree_add "git -C $repo worktree add -q --detach /home/coder/wt REV_PLACEHOLDER"
timed warm_worktree "cd /home/coder/wt && CARGO_TARGET_DIR=$slot cargo build --locked -p openagents-cli"
timed edit_rebuild "cd /home/coder/wt && echo '// measure' >> crates/openagents-cli/src/main.rs && CARGO_TARGET_DIR=$slot cargo build --locked -p openagents-cli"
timed cold_sccache "cd /home/coder/wt && CARGO_TARGET_DIR=/home/coder/cold-a cargo build --locked -p openagents-cli"
timed cold_no_sccache "cd /home/coder/wt && OA_SCCACHE=0 CARGO_TARGET_DIR=/home/coder/cold-b cargo build --locked -p openagents-cli"
out "sccache_stats $(as_coder sccache --show-stats 2>/dev/null | grep -E 'Compile requests executed|Cache hits  |Cache misses  ' | tr -s ' ' | tr '\n' ';')"
out "done"
GUEST
sed -i.bak "s#REV_PLACEHOLDER#$rev#g" "$script" && rm -f "$script.bak"
say "measuring $image on $vm ($machine spot, rev $rev)"
t0="$(date +%s)"
for zone in $ZONES; do
  if g compute instances create "$vm" --zone "$zone" --machine-type "$machine" \
      --provisioning-model=SPOT --instance-termination-action=DELETE \
      --image "$image" --image-project "$PROJECT" --boot-disk-type pd-balanced --boot-disk-size 200GB \
      --no-address --service-account "$HOST_SA" --scopes cloud-platform \
      --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
      --labels openagents-managed=coder-host-measure \
      --metadata serial-port-enable=TRUE,block-project-ssh-keys=TRUE \
      --metadata-from-file "startup-script=$script" >/dev/null 2>&1; then
    ZONE="$zone"; break
  fi
  say "no spot $machine in $zone; trying the next zone"
  t0="$(date +%s)"
done
rm -f "$script"

start=0; ready=""; results=()
deadline=$(( t0 + 7200 ))
while (( $(date +%s) < deadline )); do
  out="$(g compute instances get-serial-port-output "$vm" --zone "$ZONE" --start "$start" --format=json 2>/dev/null || true)"
  if [[ -n "$out" ]]; then
    start="$(jq -r '.next' <<<"$out")"
    while IFS= read -r line; do
      case "$line" in
        *OA_CODER_HOST_READY*)
          if [[ -z "$ready" ]]; then ready="$(( $(date +%s) - t0 ))"; say "ready after ${ready}s: ${line#*OA_CODER_HOST_READY }"; fi ;;
        "OA_CODER_HOST_MEASURE done"*) break 2 ;;
        OA_CODER_HOST_MEASURE*) results+=("${line#OA_CODER_HOST_MEASURE }"); say "${line#OA_CODER_HOST_MEASURE }" ;;
      esac
    done < <(jq -r '.contents' <<<"$out" | tr -d '\r')
  fi
  sleep 5
done
printf '%s\n' "${results[@]}" | jq -R . | jq -s --arg image "$image" --arg machine "$machine" --arg ready "$ready" \
  '{image:$image, machine:$machine, create_to_ready_seconds_poll5:($ready|tonumber? // null), results:.}'
