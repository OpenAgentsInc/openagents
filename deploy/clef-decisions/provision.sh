#!/bin/sh
# Provision clef-decisions-h100, the always-on H100 behind the decision
# gateway's hosted Clef door (docs/inference/clef-decisions-h100.md).
# Idempotent; run from a checkout as the automation account:
#
#   CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config sh deploy/clef-decisions/provision.sh [STEP]
#
# Steps (default: all, in order):
#   upload     startup.sh and restarter.sh to gs://openagentsgemini-decision-throughput/clef-decisions/
#   address    the static internal IP clef-decisions-h100 (us-central1, default subnet)
#   firewall   tcp:18096,18098 from the gateway's subnets only; IAP SSH; deny the rest
#   vm         create the VM in the first us-central1 zone with capacity
#   restarter  the Cloud Run job clef-decisions-restarter and its 3-minute Cloud Scheduler trigger
set -eu
PROJECT=openagentsgemini
REGION=us-central1
B=gs://openagentsgemini-decision-throughput/clef-decisions
SA=oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
HERE=$(cd "$(dirname "$0")" && pwd)
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
g() { gcloud --project "$PROJECT" --quiet "$@"; }
# The gateway's Direct VPC egress subnets: production (openagents-web-production)
# and staging (openagents-web-staging).
GATEWAY_RANGES=10.42.27.0/26,10.42.26.0/26

upload() {
    g storage cp "$HERE/startup.sh" "$HERE/restarter.sh" "$B/"
}
address() {
    g compute addresses describe clef-decisions-h100 --region $REGION >/dev/null 2>&1 ||
        g compute addresses create clef-decisions-h100 --region $REGION --subnet default \
            --description "Internal IP of clef-decisions-h100 (the gateway's clef_url)"
    g compute addresses describe clef-decisions-h100 --region $REGION --format='value(address)'
}
rule() { # name priority action args...
    name=$1; shift
    if g compute firewall-rules describe "$name" >/dev/null 2>&1; then return 0; fi
    g compute firewall-rules create "$name" --network default --direction INGRESS --target-tags clef-decisions "$@"
}
firewall() {
    rule clef-decisions-from-gateway --priority 900 --action ALLOW --rules tcp:18096,tcp:18098 \
        --source-ranges "$GATEWAY_RANGES" --description "The decision gateway (Cloud Run Direct VPC egress) to the Clef servers"
    rule clef-decisions-iap-ssh --priority 900 --action ALLOW --rules tcp:22,tcp:18096,tcp:18098 \
        --source-ranges 35.235.240.0/20 --description "IAP (authenticated) SSH and tunnels to clef-decisions-h100"
    rule clef-decisions-deny-other-ingress --priority 910 --action DENY --rules all \
        --source-ranges 0.0.0.0/0 --description "Nothing else reaches clef-decisions-h100 (overrides default-allow-*)"
}
vm() {
    if g compute instances list --filter=name=clef-decisions-h100 --format='value(name)' | grep -q .; then
        g compute instances list --filter=name=clef-decisions-h100 --format='table(name,zone.basename(),status,networkInterfaces[0].networkIP)'
        return 0
    fi
    for z in us-central1-a us-central1-b us-central1-c; do
        if CLOUDSDK_CONFIG=$CLOUDSDK_CONFIG bash "$HERE/restarter.sh" create "$z"; then return 0; fi
    done
    echo "no us-central1 zone had a spot H100" >&2; return 1
}
restarter() {
    g run jobs describe clef-decisions-restarter --region $REGION >/dev/null 2>&1 && verb=update || verb=create
    g run jobs $verb clef-decisions-restarter --region $REGION \
        --image gcr.io/google.com/cloudsdktool/cloud-sdk:slim \
        --command bash --args=-c,"gcloud storage cat $B/restarter.sh | bash" \
        --service-account "$SA" --max-retries 0 --task-timeout 900s --cpu 1 --memory 512Mi
    uri="https://run.googleapis.com/v2/projects/$PROJECT/locations/$REGION/jobs/clef-decisions-restarter:run"
    if g scheduler jobs describe clef-decisions-restarter --location $REGION >/dev/null 2>&1; then
        verb=update
    else
        verb=create
    fi
    g scheduler jobs $verb http clef-decisions-restarter --location $REGION \
        --schedule "*/3 * * * *" --time-zone UTC --uri "$uri" --http-method POST \
        --oauth-service-account-email "$SA" --oauth-token-scope https://www.googleapis.com/auth/cloud-platform \
        --attempt-deadline 60s --max-retry-attempts 0 \
        --description "Starts clef-decisions-h100 when a spot preemption stopped it (deploy/clef-decisions)"
}

case ${1:-all} in
    all) upload; address; firewall; vm; restarter ;;
    upload | address | firewall | vm | restarter) "$1" ;;
    *) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
