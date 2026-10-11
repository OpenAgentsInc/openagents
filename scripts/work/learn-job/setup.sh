#!/bin/sh
# Set up the weekly learning-loop run (#11312, docs/inference/filefind-promotions.md)
# on our own GCP, as the automation account:
#
#   scripts/work/learn-job/setup.sh vm         # create oa-learn-1 (stopped between runs)
#   scripts/work/learn-job/setup.sh install    # install run.sh and its boot unit on the VM
#   scripts/work/learn-job/setup.sh schedule   # Cloud Scheduler: start the VM weekly
#   scripts/work/learn-job/setup.sh run        # start a run now
#   scripts/work/learn-job/setup.sh pause | resume
#
# The VM (image oa-coder-host, the automation account) runs one cycle at
# boot (oa-learn.service: scripts/work/learn-job/run.sh) and powers itself
# off; a stopped VM bills only its disk. The cycle's only metered spend is
# the finder's embedding calls, held to OA_LEARN_SPEND_CAP_USD (default 5)
# by filefind's spend ledger; the unit's 4-hour limit bounds the VM's time.
set -eu
PROJECT=openagentsgemini
REGION=us-central1
ZONE=${OA_LEARN_ZONE:-us-central1-b}
VM=${OA_LEARN_VM:-oa-learn-1}
JOB=oa-learn-weekly
SA=oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
BUCKET=openagentsgemini-filefind-learn
SCHEDULE=${OA_LEARN_SCHEDULE:-0 6 * * 1}
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
ROOT=$(git rev-parse --show-toplevel)
g() { gcloud --project "$PROJECT" "$@"; }
ssh_vm() { g compute ssh "coder@$VM" --zone "$ZONE" --tunnel-through-iap --command "$1"; }

case ${1:-} in
vm)
    g storage buckets describe "gs://$BUCKET" > /dev/null 2>&1 ||
        g storage buckets create "gs://$BUCKET" --location "$REGION" --uniform-bucket-level-access
    key=${OA_LEARN_SSH_KEY:-$HOME/.ssh/google_compute_engine.pub}
    g compute instances create "$VM" --zone "$ZONE" \
        --machine-type "${OA_LEARN_MACHINE:-e2-highmem-8}" \
        --image-family oa-coder-host --image-project "$PROJECT" \
        --boot-disk-type pd-balanced --boot-disk-size 200GB --no-address \
        --service-account "$SA" --scopes cloud-platform \
        --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
        --labels openagents-managed=learn \
        --metadata "block-project-ssh-keys=TRUE,enable-oslogin=FALSE,ssh-keys=coder:$(cat "$key")"
    ;;
install)
    g compute instances start "$VM" --zone "$ZONE" > /dev/null 2>&1 || true
    g compute scp "$ROOT/scripts/work/learn-job/run.sh" "coder@$VM:/tmp/oa-learn-run.sh" --zone "$ZONE" --tunnel-through-iap
    ssh_vm 'set -e
install -D -m 755 /tmp/oa-learn-run.sh ~/learn/run.sh
sudo -n tee /etc/systemd/system/oa-learn.service > /dev/null <<UNIT
[Unit]
Description=Weekly filefind learning loop (#11312): one cycle, then power off
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=coder
Environment=OA_LEARN_POWEROFF=1
ExecStart=/bin/bash /home/coder/learn/run.sh
TimeoutStartSec=4h
ExecStopPost=/bin/bash /home/coder/learn/run.sh --poweroff

[Install]
WantedBy=multi-user.target
UNIT
sudo -n systemctl daemon-reload
sudo -n systemctl enable oa-learn.service
echo installed'
    ;;
schedule)
    uri="https://compute.googleapis.com/compute/v1/projects/$PROJECT/zones/$ZONE/instances/$VM/start"
    if g scheduler jobs describe "$JOB" --location "$REGION" > /dev/null 2>&1; then
        g scheduler jobs update http "$JOB" --location "$REGION" --schedule "$SCHEDULE" --time-zone UTC \
            --uri "$uri" --http-method POST --oauth-service-account-email "$SA"
    else
        g scheduler jobs create http "$JOB" --location "$REGION" --schedule "$SCHEDULE" --time-zone UTC \
            --uri "$uri" --http-method POST --oauth-service-account-email "$SA" \
            --description "Weekly filefind learning loop (#11312): starts $VM, which runs one cycle and stops"
    fi
    ;;
run) g compute instances start "$VM" --zone "$ZONE" ;;
pause | resume) g scheduler jobs "$1" "$JOB" --location "$REGION" ;;
*) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
