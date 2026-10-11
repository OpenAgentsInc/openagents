#!/bin/sh
# Set up the daily decision-throughput run (docs/inference/decision-throughput.md)
# as the automation account:
#
#   scripts/bench/decision-throughput/cloud/setup.sh image [REF]   # build; prints the image
#   scripts/bench/decision-throughput/cloud/setup.sh access        # SA, secret, bucket grants
#   scripts/bench/decision-throughput/cloud/setup.sh job IMAGE     # create/update the Cloud Run job
#   scripts/bench/decision-throughput/cloud/setup.sh schedule      # daily Cloud Scheduler trigger
#   scripts/bench/decision-throughput/cloud/setup.sh run           # run the job now
#   scripts/bench/decision-throughput/cloud/setup.sh pause | resume
#
# `image` builds oa-att from REF (default origin/main) and takes the harness
# from this checkout's scripts/bench/decision-throughput, so a harness change
# can be tried before it lands.
set -eu
PROJECT=openagentsgemini
REGION=us-central1
JOB=decision-throughput-daily
SA=oa-decision-bench@$PROJECT.iam.gserviceaccount.com
BUCKET=openagentsgemini-decision-throughput
SECRET=decision-throughput-typesafe-key
SCHEDULE=${DT_SCHEDULE:-0 11 * * *}
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
ROOT=$(git rev-parse --show-toplevel)
g() { gcloud --project "$PROJECT" "$@"; }

case ${1:-} in
image)
    ref=${2:-origin/main}
    git -C "$ROOT" fetch -q origin
    sha=$(git -C "$ROOT" rev-parse --short=10 "$ref^{commit}")
    ctx=$(mktemp -d "${TMPDIR:-/tmp}/dt-image.XXXXXX")
    git -C "$ROOT" archive "$sha" | tar -x -C "$ctx"
    rm -rf "$ctx/scripts/bench/decision-throughput"
    cp -R "$ROOT/scripts/bench/decision-throughput" "$ctx/scripts/bench/decision-throughput"
    find "$ctx/scripts/bench/decision-throughput" -name __pycache__ -prune -exec rm -rf {} +
    tag=$sha-$(date -u +%Y%m%d%H%M)
    (cd "$ctx" && g builds submit --region "$REGION" \
        --service-account "projects/$PROJECT/serviceAccounts/oa-mvp-automation@$PROJECT.iam.gserviceaccount.com" \
        --config scripts/bench/decision-throughput/cloud/cloudbuild.yaml \
        --ignore-file scripts/bench/decision-throughput/cloud/image.gcloudignore \
        --substitutions "_TAG=$tag" . >&2)
    rm -rf "$ctx"
    echo "us-central1-docker.pkg.dev/$PROJECT/openagents/decision-throughput:$tag"
    ;;
access)
    g iam service-accounts describe "$SA" > /dev/null 2>&1 ||
        g iam service-accounts create oa-decision-bench --display-name "decision-throughput daily run"
    # Jev key for the bench, from ~/work/.secrets/typesafe.env (read, never printed)
    if ! g secrets describe "$SECRET" > /dev/null 2>&1; then
        (set -a; . "$HOME/work/.secrets/typesafe.env"; printf %s "$TYPESAFE_API_KEY") |
            g secrets create "$SECRET" --replication-policy automatic --data-file=-
    fi
    g secrets add-iam-policy-binding "$SECRET" --member "serviceAccount:$SA" --role roles/secretmanager.secretAccessor > /dev/null
    # GitHub token that pushes the regenerated tables (the pool hosts' token)
    g secrets add-iam-policy-binding coder-pool-git-token --member "serviceAccount:$SA" --role roles/secretmanager.secretAccessor > /dev/null
    gcloud storage buckets add-iam-policy-binding "gs://$BUCKET" --member "serviceAccount:$SA" --role roles/storage.objectAdmin > /dev/null
    # Gemini on Vertex, the fallback judge
    # (the automation account can't set project IAM; this one needs an owner's gcloud)
    g projects add-iam-policy-binding "$PROJECT" --member "serviceAccount:$SA" --role roles/aiplatform.user --condition=None > /dev/null ||
        echo "as an owner: gcloud projects add-iam-policy-binding $PROJECT --member serviceAccount:$SA --role roles/aiplatform.user --condition=None"
    echo "access ready for $SA"
    ;;
job)
    image=$2
    g run jobs deploy "$JOB" --region "$REGION" --image "$image" --service-account "$SA" \
        --set-secrets "TYPESAFE_API_KEY=$SECRET:latest,GH_TOKEN=coder-pool-git-token:latest" \
        --set-env-vars "DT_BUCKET=$BUCKET" \
        --cpu 2 --memory 4Gi --task-timeout 2h --max-retries 0 --tasks 1
    g run jobs add-iam-policy-binding "$JOB" --region "$REGION" --member "serviceAccount:$SA" --role roles/run.invoker > /dev/null
    ;;
schedule)
    uri="https://run.googleapis.com/v2/projects/$PROJECT/locations/$REGION/jobs/$JOB:run"
    if g scheduler jobs describe "$JOB" --location "$REGION" > /dev/null 2>&1; then
        g scheduler jobs update http "$JOB" --location "$REGION" --schedule "$SCHEDULE" --time-zone UTC \
            --uri "$uri" --http-method POST --oauth-service-account-email "$SA"
    else
        g scheduler jobs create http "$JOB" --location "$REGION" --schedule "$SCHEDULE" --time-zone UTC \
            --uri "$uri" --http-method POST --oauth-service-account-email "$SA" \
            --description "Daily decision-throughput run (docs/inference/decision-throughput.md)"
    fi
    ;;
run) g run jobs execute "$JOB" --region "$REGION" --async ;;
pause | resume) g scheduler jobs "$1" "$JOB" --location "$REGION" ;;
*) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
