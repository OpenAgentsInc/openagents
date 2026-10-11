#!/bin/sh
# The decision-throughput H100 run (docs/inference/decision-throughput.md):
# one plain Spot a3-highgpu-1g that measures Psionic Clef-Flash and Clef 27B
# (and, with --sealed, the sealed GPU oa-att-h100-1 over NIP-ATT), appends
# the results to gs://openagentsgemini-decision-throughput/runs/DAY.jsonl and
# deletes itself. It can never run unbounded: max run 75 minutes, then
# Compute Engine deletes it.
#
#   h100-run.sh bundle [REF]   # Cloud Build: the GPU image's psionic + CUDA libs + oa-att -> bin/<sha>/
#   h100-run.sh upload         # harness, targets and startup.sh to the bucket
#   h100-run.sh start [--sealed] [--keep]   # create the VM (zones: us-east5-a, us-central1-a, europe-west4-c)
#   h100-run.sh status | logs | delete
#   h100-run.sh schedule [CRON]  # Cloud Scheduler: the same create, weekly (default Sat 10:00 UTC)
#
# Runs as the automation account (CLOUDSDK_CONFIG, default
# ~/work/.secrets/gcloud-sa-config); the VM runs as that account too.
set -eu
PROJECT=openagentsgemini
B=gs://openagentsgemini-decision-throughput
SA=oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
VM=${DT_VM:-oa-decision-h100}
ZONES=${DT_ZONES:-us-east5-a us-central1-a europe-west4-c}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
say() { printf '%s\n' "$*" >&2; }

# The instance body (Compute API instances.insert), shared by `start` and
# the scheduler job so both create exactly the same VM.
body() { # zone sealed keep run
    python3 - "$PROJECT" "$1" "$VM" "$SA" "$2" "$3" "$4" "$B" <<'EOF'
import json, sys
project, zone, vm, sa, sealed, keep, run, bucket = sys.argv[1:]
region = zone.rsplit("-", 1)[0]
meta = [{"key": "startup-script-url", "value": f"{bucket}/h100/startup.sh"},
        {"key": "dt-sealed", "value": sealed}, {"key": "dt-keep", "value": keep}]
if run:
    meta.append({"key": "dt-run", "value": run})
print(json.dumps({
    "name": vm,
    "machineType": f"zones/{zone}/machineTypes/a3-highgpu-1g",
    "labels": {"app": "decision-throughput"},
    "scheduling": {"provisioningModel": "SPOT", "instanceTerminationAction": "DELETE",
                   "onHostMaintenance": "TERMINATE", "automaticRestart": False,
                   "maxRunDuration": {"seconds": 4500}},
    "disks": [{"boot": True, "autoDelete": True, "initializeParams": {
        "sourceImage": "projects/deeplearning-platform-release/global/images/family/common-cu129-ubuntu-2404-nvidia-580",
        "diskSizeGb": "100", "diskType": f"zones/{zone}/diskTypes/pd-balanced"}}],
    "networkInterfaces": [{"network": "global/networks/default",
                           "accessConfigs": [{"type": "ONE_TO_ONE_NAT", "name": "External NAT"}]}],
    "serviceAccounts": [{"email": sa, "scopes": ["https://www.googleapis.com/auth/cloud-platform"]}],
    "metadata": {"items": meta},
}))
EOF
}

insert() { # zone body -> 0 on success
    tok=$(gcloud auth print-access-token)
    out=$(curl -s -X POST -H "Authorization: Bearer $tok" -H 'Content-Type: application/json' \
        "https://compute.googleapis.com/compute/v1/projects/$PROJECT/zones/$1/instances" -d "$2")
    op=$(printf '%s' "$out" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("name",""))')
    [ -n "$op" ] || { say "insert in $1 refused: $(printf '%s' "$out" | head -c 300)"; return 1; }
    for i in $(seq 60); do
        st=$(curl -s -H "Authorization: Bearer $tok" \
            "https://compute.googleapis.com/compute/v1/projects/$PROJECT/zones/$1/operations/$op")
        if printf '%s' "$st" | grep -q '"status": "DONE"'; then
            if printf '%s' "$st" | grep -q '"error"'; then
                say "create in $1 failed: $(printf '%s' "$st" | python3 -c 'import json,sys; print(json.load(sys.stdin)["error"]["errors"][0]["message"][:200])')"
                return 1
            fi
            return 0
        fi
        sleep 3
    done
    return 1
}

zone_of() { gcloud compute instances list --project $PROJECT --filter="name=$VM" --format='value(zone.basename())'; }

case ${1:-} in
    bundle)
        ref=${2:-origin/main}
        git -C "$ROOT" fetch -q origin
        full=$(git -C "$ROOT" rev-parse "$ref^{commit}"); sha=$(printf '%s' "$full" | cut -c1-10)
        image=us-central1-docker.pkg.dev/$PROJECT/openagents/att-provider-gpu
        digest=$(gcloud artifacts docker images describe "$image:$sha" --format='value(image_summary.digest)') ||
            { say "no att-provider-gpu:$sha; run scripts/deploy/att-provider.sh gpu-build $ref first"; exit 1; }
        ctx=$(mktemp -d); git -C "$ROOT" archive "$full" | tar -x -C "$ctx"
        cp "$HERE/cloudbuild-bundle.yaml" "$ctx/.dt-bundle.yaml"  # this checkout's recipe
        (cd "$ctx" && gcloud builds submit --project $PROJECT \
            --service-account "projects/$PROJECT/serviceAccounts/$SA" \
            --config .dt-bundle.yaml \
            --ignore-file deploy/att/att.gcloudignore \
            --substitutions "_TAG=$sha,_IMAGE=$image@$digest" .)
        rm -rf "$ctx"
        say "bundle: $B/bin/$sha/bundle.tar.gz (bin/latest = $sha)" ;;
    upload)
        gcloud storage cp "$HERE/../harness.py" "$HERE/../gcs.py" "$HERE/../targets.json" "$B/harness/"
        gcloud storage cp "$HERE/startup.sh" "$B/h100/startup.sh" ;;
    start)
        shift; sealed=0; keep=0
        for a in "$@"; do case $a in --sealed) sealed=1 ;; --keep) keep=1 ;; esac; done
        run=$(date -u +%Y%m%dT%H%M%SZ)
        for z in $ZONES; do
            say "creating $VM in $z (run $run)"
            if insert "$z" "$(body "$z" $sealed $keep "$run")"; then say "created in $z"; echo "$run"; exit 0; fi
        done
        say "no zone had capacity"; exit 1 ;;
    status) gcloud compute instances list --project $PROJECT --filter="name=$VM" --format='table(name,zone.basename(),status,creationTimestamp)'
        gcloud storage ls "$B/h100/" 2>/dev/null | tail -3 ;;
    logs) z=$(zone_of); gcloud compute instances get-serial-port-output "$VM" --zone "$z" --project $PROJECT 2>/dev/null |
        grep -a 'startup-script' | tail -"${2:-40}" ;;
    delete) z=$(zone_of); [ -n "$z" ] && gcloud compute instances delete "$VM" --zone "$z" --project $PROJECT --quiet ;;
    schedule)
        cron=${2:-0 10 * * 6}
        z=$(printf '%s' "$ZONES" | cut -d' ' -f1)
        # The job posts the same body; Compute Engine names the run by the
        # VM's creation (no dt-run), and the sealed GPU is measured too.
        body "$z" 1 0 "" > /tmp/dt-h100-body.json
        uri="https://compute.googleapis.com/compute/v1/projects/$PROJECT/zones/$z/instances"
        if gcloud scheduler jobs describe decision-throughput-h100-weekly --location us-central1 --project $PROJECT >/dev/null 2>&1; then
            verb=update; hdr=--update-headers
        else
            verb=create; hdr=--headers
        fi
        gcloud scheduler jobs $verb http decision-throughput-h100-weekly --location us-central1 --project $PROJECT \
            --schedule "$cron" --time-zone UTC --uri "$uri" --http-method POST \
            "$hdr" Content-Type=application/json --message-body-from-file /tmp/dt-h100-body.json \
            --oauth-service-account-email "$SA" --oauth-token-scope https://www.googleapis.com/auth/cloud-platform \
            --attempt-deadline 60s --max-retry-attempts 0 \
            --description "Weekly decision-throughput H100 run (scripts/bench/decision-throughput/h100); the VM deletes itself"
        rm -f /tmp/dt-h100-body.json ;;
    *) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
