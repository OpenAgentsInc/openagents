#!/usr/bin/env bash
#
# Bake the `oa-coder-host` GCE image: Debian 12, the toolchains, the engine
# CLIs (not logged in), a clone of OpenAgentsInc/openagents at origin/main and
# a warm Cargo target for it. Runbook: docs/deployment/coder-host-image.md.
#
#   scripts/cloud/build-coder-host-image.sh            # dry run: print the plan
#   scripts/cloud/build-coder-host-image.sh --apply    # bake, smoke, promote, prune
#       [--local-setup]   use this checkout's coder-host-setup.sh and bake guest
#                         script instead of the copy at origin/main (testing)
#       [--image-name N]  override oa-coder-host-YYYYMMDD
#
# Steps with --apply:
#   1. Resolve origin/main to a commit. The image is oa-coder-host-YYYYMMDD
#      (UTC); if a READY image of that name exists, exit 0.
#   2. Create a spot builder VM (no external address, Cloud NAT for egress,
#      service account oa-coder-host for the sccache bucket only) whose
#      startup script is scripts/cloud/coder-host-bake-guest.sh.
#   3. Follow its serial console until OA_CODER_HOST_BAKE_OK or _FAILED. A
#      preempted spot builder is retried once on demand.
#   4. Stop the builder, create the image in family oa-coder-host from its
#      disk, delete the builder.
#   5. Boot a smoke VM from the image and wait for OA_CODER_HOST_READY; on
#      failure the new image is deleted. Delete the smoke VM.
#   6. Delete all but the newest KEEP (3) images in the family.
#
# Works from a laptop (CLOUDSDK_CONFIG pointing at the automation account)
# and from Cloud Build (the scheduled job, scripts/cloud/coder-host-image-schedule.sh).
# Every VM it creates is deleted on exit, success or not.
set -euo pipefail

PROJECT="${OA_PROJECT:-openagentsgemini}"
# Zones to try, in order, when one is out of spot (or on-demand) capacity.
# The image is global; only the builder and smoke VMs are zonal.
ZONES="${OA_ZONES:-us-central1-a us-central1-b us-central1-c us-central1-f}"
ZONE="${ZONES%% *}"
FAMILY="oa-coder-host"
KEEP="${OA_KEEP_IMAGES:-3}"
BUILDER_MACHINE="${OA_BUILDER_MACHINE:-c3-standard-22}"
SMOKE_MACHINE="${OA_SMOKE_MACHINE:-e2-standard-4}"
DISK_GB="${OA_DISK_GB:-200}"
HOST_SA="oa-coder-host@${PROJECT}.iam.gserviceaccount.com"
SCCACHE_BUCKET="${OA_SCCACHE_BUCKET:-openagentsgemini-autopilot-rust-sccache}"
REPO_URL="https://github.com/OpenAgentsInc/openagents.git"
BAKE_TIMEOUT_S="${OA_BAKE_TIMEOUT_S:-7200}"
SMOKE_TIMEOUT_S=600

apply="false"
image_name=""
local_setup="false"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply) apply="true"; shift ;;
    --image-name) image_name="${2:?}"; shift 2 ;;
    --local-setup) local_setup="true"; shift ;;
    -h|--help) sed -n '2,28p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
guest_script="$here/coder-host-bake-guest.sh"
g() { gcloud --project "$PROJECT" --quiet "$@"; }
now() { date -u +%s; }
say() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }

rev="$(git ls-remote "$REPO_URL" refs/heads/main | cut -f1)"
[[ "$rev" =~ ^[0-9a-f]{40}$ ]] || { echo "could not resolve origin/main" >&2; exit 1; }
image_name="${image_name:-${FAMILY}-$(date -u +%Y%m%d)}"
stamp="$(date -u +%Y%m%d%H%M%S)"
builder="oa-coder-host-builder-${stamp}"
smoke="oa-coder-host-smoke-${stamp}"

if [[ "$apply" != "true" ]]; then
  cat <<PLAN
oa-coder-host bake (dry run)
  project:   $PROJECT  zones: $ZONES
  image:     $image_name  family: $FAMILY  keep: $KEEP
  revision:  $rev
  builder:   $builder ($BUILDER_MACHINE spot, ${DISK_GB} GB pd-balanced, sa $HOST_SA)
  smoke:     $smoke ($SMOKE_MACHINE spot)
  sccache:   gs://$SCCACHE_BUCKET
PLAN
  exit 0
fi

if [[ "$(g compute images describe "$image_name" --format='value(status)' 2>/dev/null || true)" == "READY" ]]; then
  say "image $image_name already exists and is READY; nothing to do"
  exit 0
fi

image_created="false"
image_admitted="false"
cleanup() {
  local vm
  local vm zone
  for vm in "$builder" "$smoke"; do
    zone="$(g compute instances list --filter="name=$vm" --format='value(zone.basename())' 2>/dev/null || true)"
    if [[ -n "$zone" ]]; then
      g compute instances delete "$vm" --zone "$zone" >/dev/null 2>&1 || say "could not delete $vm"
    fi
  done
  if [[ "$image_created" == "true" && "$image_admitted" != "true" ]]; then
    say "deleting unadmitted image $image_name"
    g compute images delete "$image_name" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# Follow an instance's serial console until a line matches OK or FAIL.
# Prints the matching line on stdout; returns 0 on OK, 1 on FAIL or timeout,
# 2 when the instance stopped (a spot preemption).
follow_serial() {
  local instance="$1" ok="$2" fail="$3" timeout_s="$4" start=0 deadline out next status line
  deadline=$(( $(now) + timeout_s ))
  while (( $(now) < deadline )); do
    out="$(g compute instances get-serial-port-output "$instance" --zone "$ZONE" \
      --start "$start" --format=json 2>/dev/null || true)"
    if [[ -n "$out" ]]; then
      next="$(jq -r '.next // empty' <<<"$out")"
      [[ -n "$next" ]] && start="$next"
      while IFS= read -r line; do
        # The guest writes its markers straight to the serial port, so they
        # start a line; the journal's copy of the same text is ignored.
        [[ "$line" == OA_* ]] || continue
        case "$line" in
          OA_CODER_HOST_SETUP*|OA_CODER_HOST_BAKE\ phase*|OA_CODER_HOST_BAKE_LOG*) say "$line" ;;
        esac
        if [[ "$line" == *"$ok"* ]]; then printf '%s\n' "${line#*"$ok"}"; return 0; fi
        if [[ "$line" == *"$fail"* ]]; then say "$line"; return 1; fi
      done < <(jq -r '.contents // empty' <<<"$out" | tr -d '\r')
    fi
    status="$(g compute instances describe "$instance" --zone "$ZONE" --format='value(status)' 2>/dev/null || echo GONE)"
    case "$status" in STOPPING|TERMINATED|SUSPENDED|GONE) return 2 ;; esac
    sleep 2
  done
  say "timed out after ${timeout_s}s waiting for $ok on $instance"
  return 1
}

# Create an instance in the first zone with capacity: spot in every zone,
# then (for the builder) on demand in every zone. Sets ZONE.
#   create_vm NAME MACHINE MODEL [extra gcloud args...]
create_vm() {
  local name="$1" machine="$2" models="$3" model zone err
  shift 3
  for model in $models; do
    for zone in $ZONES; do
      local extra=()
      if [[ "$model" == "SPOT" ]]; then
        extra=(--provisioning-model=SPOT "--instance-termination-action=$SPOT_ACTION")
      fi
      if err="$(g compute instances create "$name" --zone "$zone" --machine-type "$machine" \
          "${extra[@]}" "$@" 2>&1 >/dev/null)"; then
        ZONE="$zone"
        say "created $name in $zone ($machine, $model)"
        return 0
      fi
      case "$err" in
        *ZONE_RESOURCE_POOL_EXHAUSTED*|*stockout*|*does\ not\ have\ enough\ resources*|*QUOTA*)
          say "no $model capacity for $machine in $zone; trying the next zone" ;;
        *) say "creating $name failed: $err"; return 1 ;;
      esac
    done
  done
  say "no zone in [$ZONES] had capacity for $machine"
  return 1
}

create_builder() {
  local models="$1" files="startup-script=$guest_script"
  if [[ "$local_setup" == "true" ]]; then
    files="$files,oa-setup-script=$here/coder-host-setup.sh"
  fi
  SPOT_ACTION=STOP create_vm "$builder" "$BUILDER_MACHINE" "$models" \
    --image-family debian-12 --image-project debian-cloud \
    --boot-disk-size "${DISK_GB}GB" --boot-disk-type pd-balanced \
    --no-address \
    --service-account "$HOST_SA" --scopes cloud-platform \
    --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
    --labels "openagents-managed=coder-host-builder" \
    --metadata "oa-rev=$rev,oa-repo-url=$REPO_URL,oa-sccache-bucket=$SCCACHE_BUCKET,serial-port-enable=TRUE,block-project-ssh-keys=TRUE" \
    --metadata-from-file "$files"
}

t0="$(now)"
say "baking $image_name from $rev on $builder ($BUILDER_MACHINE spot)"
create_builder "SPOT STANDARD"
set +e
result="$(follow_serial "$builder" OA_CODER_HOST_BAKE_OK OA_CODER_HOST_BAKE_FAILED "$BAKE_TIMEOUT_S")"
rc=$?
set -e
if [[ $rc == 2 ]]; then
  say "spot builder was preempted; retrying once on demand"
  g compute instances delete "$builder" --zone "$ZONE" >/dev/null
  t0="$(now)"
  create_builder STANDARD
  set +e
  result="$(follow_serial "$builder" OA_CODER_HOST_BAKE_OK OA_CODER_HOST_BAKE_FAILED "$BAKE_TIMEOUT_S")"
  rc=$?
  set -e
fi
[[ $rc == 0 ]] || { say "bake failed (rc=$rc)"; exit 1; }
t_baked="$(now)"
manifest="$(sed 's/^ *//' <<<"$result")"
say "bake done in $(( t_baked - t0 ))s"

g compute instances stop "$builder" --zone "$ZONE" >/dev/null
g compute images create "$image_name" \
  --source-disk "$builder" --source-disk-zone "$ZONE" \
  --family "$FAMILY" --storage-location us-central1 \
  --labels "openagents-managed=coder-host-image,openagents-source-revision=$rev,openagents-boot-smoke=pending" \
  --description "Coder host: openagents ${rev:0:12}, warm target for openagents-cli microcoder coder, engine CLIs not logged in" >/dev/null
image_created="true"
t_imaged="$(now)"
g compute instances delete "$builder" --zone "$ZONE" >/dev/null
say "image $image_name created in $(( t_imaged - t_baked ))s"

# Boot smoke: a VM from the image must reach OA_CODER_HOST_READY.
t_smoke="$(now)"
SPOT_ACTION=DELETE create_vm "$smoke" "$SMOKE_MACHINE" "SPOT STANDARD" \
  --image "$image_name" --image-project "$PROJECT" \
  --boot-disk-type pd-balanced \
  --no-address \
  --service-account "$HOST_SA" --scopes cloud-platform \
  --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
  --labels "openagents-managed=coder-host-smoke" \
  --metadata "serial-port-enable=TRUE,block-project-ssh-keys=TRUE"
set +e
ready="$(follow_serial "$smoke" OA_CODER_HOST_READY OA_CODER_HOST_NEVER "$SMOKE_TIMEOUT_S")"
rc=$?
set -e
[[ $rc == 0 ]] || { say "boot smoke failed (rc=$rc)"; exit 1; }
t_ready="$(now)"
g compute instances delete "$smoke" --zone "$ZONE" >/dev/null
g compute images add-labels "$image_name" --labels openagents-boot-smoke=passed >/dev/null
image_admitted="true"
say "boot smoke passed in $(( t_ready - t_smoke ))s:$ready"

# Keep the newest KEEP images in the family.
g compute images list --filter="family=$FAMILY" \
  --sort-by=~creationTimestamp --format='value(name)' | tail -n +$(( KEEP + 1 )) \
  | while IFS= read -r name; do
      [[ -n "$name" ]] || continue
      say "pruning $name"
      g compute images delete "$name" >/dev/null
    done

size="$(g compute images describe "$image_name" --format='value(archiveSizeBytes,diskSizeGb)')"
jq -n --arg image "$image_name" --arg rev "$rev" --arg size "$size" --arg ready "$ready" \
  --argjson bake $(( t_baked - t0 )) --argjson imaging $(( t_imaged - t_baked )) \
  --argjson boot_to_ready $(( t_ready - t_smoke )) --argjson total $(( t_ready - t0 )) \
  --arg guest "$manifest" \
  '{image:$image, rev:$rev, archive_size_bytes:($size|split("\t")[0]|tonumber? // null),
    disk_size_gb:($size|split("\t")[1]|tonumber? // null),
    seconds:{bake:$bake, imaging:$imaging, smoke_boot_to_ready:$boot_to_ready, total:$total},
    smoke_ready:($ready|ltrimstr(" ")), guest:($guest|try fromjson catch null)}'
