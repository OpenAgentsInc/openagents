#!/bin/bash
# Keeps clef-decisions-h100 running (docs/inference/clef-decisions-h100.md).
# The VM is spot with termination action STOP, so a preemption leaves it
# TERMINATED with its disk. The Cloud Run job `clef-decisions-restarter`
# runs this every 3 minutes (Cloud Scheduler) as the automation account:
#
#   restarter.sh            # start the VM if it is stopped; move zones on stockout
#   restarter.sh create Z   # create the VM in zone Z (provision.sh uses this)
#
# A start refused for capacity counts on the VM's `stockouts` label; after
# STOCKOUT_MOVE refusals in a row (about 15 minutes) the VM is recreated in
# the next us-central1 zone with the same static internal IP (the gateway's
# clef_url keeps working). Its old boot disk is kept (labelled
# clef-decisions=spare-disk); the one kept before it is deleted. The new
# disk fills itself from GCS at boot in about a minute.
# A VM labelled clef-decisions=paused is left alone (pausing: the ops page).
set -u
PROJECT=${PROJECT:-openagentsgemini}
REGION=us-central1
VM=${VM:-clef-decisions-h100}
ZONES=${ZONES:-us-central1-a us-central1-b us-central1-c}
ADDRESS=clef-decisions-h100
SA=oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
STARTUP=gs://openagentsgemini-decision-throughput/clef-decisions/startup.sh
STOCKOUT_MOVE=${STOCKOUT_MOVE:-5}
say() { echo "restarter: $*"; }
g() { gcloud --project "$PROJECT" --quiet "$@"; }

create() { # zone
    ip=$(g compute addresses describe "$ADDRESS" --region "$REGION" --format='value(address)')
    g compute instances create "$VM" --zone "$1" \
        --machine-type a3-highgpu-1g \
        --provisioning-model SPOT --instance-termination-action STOP \
        --maintenance-policy TERMINATE \
        --image-family common-cu129-ubuntu-2404-nvidia-580 --image-project deeplearning-platform-release \
        --boot-disk-size 200GB --boot-disk-type pd-ssd \
        --network-interface "subnet=default,private-network-ip=$ip" \
        --tags clef-decisions \
        --labels app=clef-decisions,clef-decisions=serving,stockouts=0 \
        --service-account "$SA" --scopes cloud-platform \
        --metadata "startup-script-url=$STARTUP,enable-oslogin=TRUE"
}

if [ "${1:-}" = create ]; then create "${2:?zone}"; exit $?; fi

row=$(g compute instances list --filter="name=$VM" \
    --format='value(zone.basename(),status,labels.clef-decisions,labels.stockouts)')
if [ -z "$row" ]; then
    say "$VM does not exist; creating it"
    for z in $ZONES; do create "$z" && { say "created in $z"; exit 0; }; done
    say "no zone had capacity"; exit 1
fi
read -r zone status mode stockouts <<<"$row"
[ "$mode" = paused ] && { say "$VM is paused ($status); leaving it"; exit 0; }
case $status in
    RUNNING | PROVISIONING | STAGING | REPAIRING) say "$VM is $status in $zone"; exit 0 ;;
    STOPPING | SUSPENDING) say "$VM is $status; next round"; exit 0 ;;
esac
say "$VM is $status in $zone; starting"
if out=$(g compute instances start "$VM" --zone "$zone" 2>&1); then
    g compute instances add-labels "$VM" --zone "$zone" --labels stockouts=0 >/dev/null
    say "started in $zone"; exit 0
fi
say "start refused: $(printf '%s' "$out" | tail -3 | tr '\n' ' ' | cut -c1-300)"
if ! printf '%s' "$out" | grep -qiE 'ZONE_RESOURCE_POOL_EXHAUSTED|does not have enough resources|stockout|resource pool'; then
    exit 1
fi
n=$(( ${stockouts:-0} + 1 ))
g compute instances add-labels "$VM" --zone "$zone" --labels "stockouts=$n" >/dev/null
[ "$n" -lt "$STOCKOUT_MOVE" ] && { say "stockout $n/$STOCKOUT_MOVE in $zone"; exit 1; }

# Move: delete the VM (keeping its disk), then create it in the zones
# after this one. If none has capacity, the next round finds no VM and
# tries every zone.
disk=$(g compute instances describe "$VM" --zone "$zone" --format='value(disks[0].source.basename())')
g compute disks list --filter="labels.clef-decisions=spare-disk" --format='value(name,zone.basename())' |
    while read -r old oldzone; do g compute disks delete "$old" --zone "$oldzone" >/dev/null || true; done
say "moving $VM out of $zone (keeping disk $disk)"
g compute instances delete "$VM" --zone "$zone" --keep-disks boot || exit 1
g compute disks add-labels "$disk" --zone "$zone" --labels clef-decisions=spare-disk >/dev/null || true
set -- $ZONES $ZONES
while [ "$1" != "$zone" ]; do shift; done
shift
for next in "$@"; do
    [ "$next" = "$zone" ] && break
    create "$next" && { say "moved to $next"; exit 0; }
done
say "no zone had capacity; the next round tries again"
exit 1
