#!/bin/bash
# Keeps the decision GPUs running (docs/inference/clef-decisions-h100.md):
#   clef-decisions-h100  a3-highgpu-1g, spot, termination action STOP
#   clef-decisions-a100  a2-highgpu-1g, on-demand (never preempted; host
#                        maintenance stops it and GCE restarts it)
# A stopped VM is TERMINATED with its disk. The Cloud Run job
# `clef-decisions-restarter` runs this every 3 minutes (Cloud Scheduler) as
# the automation account:
#
#   restarter.sh            # start each VM if it is stopped; move zones on stockout
#   restarter.sh create Z [VM]   # create VM (default clef-decisions-h100) in zone Z
#
# A start refused for capacity counts on the VM's `stockouts` label; after
# STOCKOUT_MOVE refusals in a row (about 15 minutes) the VM is recreated in
# the next us-central1 zone with the same static internal IP (the gateway's
# clef_url keeps working). Its old boot disk is kept (labelled
# clef-decisions=spare-disk); the one kept before it (of the same VM) is deleted. The new
# disk fills itself from GCS at boot in about a minute.
# A VM labelled clef-decisions=paused is left alone (pausing: the ops page).
set -u
PROJECT=${PROJECT:-openagentsgemini}
REGION=us-central1
VMS=${VMS:-clef-decisions-h100 clef-decisions-a100}
SA=oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
STARTUP=gs://openagentsgemini-decision-throughput/clef-decisions/startup.sh
STOCKOUT_MOVE=${STOCKOUT_MOVE:-5}
say() { echo "restarter: $*"; }
g() { gcloud --project "$PROJECT" --quiet "$@"; }

# shape VM: the machine, pricing, zones and bundle of each decision VM. Its
# reserved internal IP is the address of the same name.
shape() {
    VM=$1; ADDRESS=$1
    case $1 in
        clef-decisions-h100)
            MACHINE=a3-highgpu-1g
            PRICING="--provisioning-model SPOT --instance-termination-action STOP"
            ZONES="us-central1-a us-central1-b us-central1-c"
            BUNDLE_META="" ;;  # follows bin/latest (sm_90)
        clef-decisions-a100)
            MACHINE=a2-highgpu-1g
            PRICING="--provisioning-model STANDARD --restart-on-failure"
            ZONES="us-central1-a us-central1-b us-central1-c us-central1-f"
            BUNDLE_META=",clef-bundle=661e02d3ed-sm80" ;;  # an sm_80 build; bin/latest is sm_90
        *) say "unknown VM $1"; return 1 ;;
    esac
}

create() { # zone
    ip=$(g compute addresses describe "$ADDRESS" --region "$REGION" --format='value(address)')
    g compute instances create "$VM" --zone "$1" \
        --machine-type "$MACHINE" $PRICING \
        --maintenance-policy TERMINATE \
        --image-family common-cu129-ubuntu-2404-nvidia-580 --image-project deeplearning-platform-release \
        --boot-disk-size 200GB --boot-disk-type pd-ssd \
        --network-interface "subnet=default,private-network-ip=$ip" \
        --tags clef-decisions \
        --labels app=clef-decisions,clef-decisions=serving,stockouts=0 \
        --service-account "$SA" --scopes cloud-platform \
        --metadata "startup-script-url=$STARTUP,enable-oslogin=TRUE$BUNDLE_META"
}

keep() { # VM
    shape "$1" || return 1

    row=$(g compute instances list --filter="name=$VM" \
        --format='value(zone.basename(),status,labels.clef-decisions,labels.stockouts)')
    if [ -z "$row" ]; then
        say "$VM does not exist; creating it"
        for z in $ZONES; do create "$z" && { say "created in $z"; return 0; }; done
        say "no zone had capacity"; return 1
    fi
    read -r zone status mode stockouts <<<"$row"
    [ "$mode" = paused ] && { say "$VM is paused ($status); leaving it"; return 0; }
    case $status in
        RUNNING | PROVISIONING | STAGING | REPAIRING) say "$VM is $status in $zone"; return 0 ;;
        STOPPING | SUSPENDING) say "$VM is $status; next round"; return 0 ;;
    esac
    say "$VM is $status in $zone; starting"
    if out=$(g compute instances start "$VM" --zone "$zone" 2>&1); then
        g compute instances add-labels "$VM" --zone "$zone" --labels stockouts=0 >/dev/null
        say "started in $zone"; return 0
    fi
    say "start refused: $(printf '%s' "$out" | tail -3 | tr '\n' ' ' | cut -c1-300)"
    if ! printf '%s' "$out" | grep -qiE 'ZONE_RESOURCE_POOL_EXHAUSTED|does not have enough resources|stockout|resource pool'; then
        return 1
    fi
    n=$(( ${stockouts:-0} + 1 ))
    g compute instances add-labels "$VM" --zone "$zone" --labels "stockouts=$n" >/dev/null
    [ "$n" -lt "$STOCKOUT_MOVE" ] && { say "stockout $n/$STOCKOUT_MOVE in $zone"; return 1; }

    # Move: delete the VM (keeping its disk), then create it in the zones
    # after this one. If none has capacity, the next round finds no VM and
    # tries every zone.
    disk=$(g compute instances describe "$VM" --zone "$zone" --format='value(disks[0].source.basename())')
    g compute disks list --filter="labels.clef-decisions=spare-disk AND name~^$VM" --format='value(name,zone.basename())' |
        while read -r old oldzone; do g compute disks delete "$old" --zone "$oldzone" >/dev/null || true; done
    say "moving $VM out of $zone (keeping disk $disk)"
    g compute instances delete "$VM" --zone "$zone" --keep-disks boot || return 1
    g compute disks add-labels "$disk" --zone "$zone" --labels clef-decisions=spare-disk >/dev/null || true
    set -- $ZONES $ZONES
    while [ "$1" != "$zone" ]; do shift; done
    shift
    for next in "$@"; do
        [ "$next" = "$zone" ] && break
        create "$next" && { say "moved to $next"; return 0; }
    done
    say "no zone had capacity; the next round tries again"
    return 1
}

if [ "${1:-}" = create ]; then shape "${3:-clef-decisions-h100}" && create "${2:?zone}"; exit $?; fi
rc=0
for vm in $VMS; do keep "$vm" || rc=1; done
exit $rc
