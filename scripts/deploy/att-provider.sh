#!/bin/sh
# The NIP-ATT attested decision provider (docs/security/private-inference.md):
# build its image, publish its release, and run it on one Confidential
# Space VM (Intel TDX) in openagentsgemini.
#
#   scripts/deploy/att-provider.sh build [REF]        # default origin/main; prints the digest
#   scripts/deploy/att-provider.sh release DIGEST REF # publish the 3202 release and the 30202 head
#   scripts/deploy/att-provider.sh start RELEASE_ID DIGEST
#   scripts/deploy/att-provider.sh replace RELEASE_ID DIGEST  # delete and start again
#   scripts/deploy/att-provider.sh retarget RELEASE_ID DIGEST # a stopped VM's next image
#   scripts/deploy/att-provider.sh stop | resume | delete | status | logs
#
# The GPU lane (#11241): the same commands prefixed `gpu-` (gpu-build,
# gpu-release, gpu-start, gpu-replace, gpu-stop, gpu-resume, gpu-delete,
# gpu-status, gpu-logs) build deploy/att/Dockerfile.gpu (Clef on CUDA) into
# att-provider-gpu, publish releases for workload clef-decisions-gpu that
# require an H100 in confidential-computing mode, and run them on one Spot
# a3-highgpu-1g (Intel TDX + H100) Confidential Space VM, oa-att-h100-1, that
# stops itself after ATT_MAX_RUN (default 1h) and on preemption, and keeps
# its disk while stopped. ATT_ZONE picks the zone (default us-central1-a;
# confidential H100s are also in us-east5-a and europe-west4-c).
#
# build exports REF with `git archive` and runs deploy/att/cloudbuild.yaml
# (the reproducible recipe) as the automation account. release reads the
# publisher key from ~/work/.secrets/att-publisher.key (Secret Manager:
# att-publisher-key), the binaries' digests from the build log, and needs
# target/debug/oa-att (cargo build -p oa-att --features net). The release
# takes effect after NOTICE seconds (default 600); the head's generation is
# the Unix time, so it only grows.
set -eu

PROJECT=openagentsgemini
REGION=us-central1
ZONE=${ATT_ZONE:-us-central1-a}
case ${1:-} in
    gpu-*)
        LANE=gpu
        cmd=${1#gpu-}
        VM=${ATT_VM:-oa-att-h100-1}
        MACHINE=${ATT_MACHINE:-a3-highgpu-1g}
        REPO=us-central1-docker.pkg.dev/$PROJECT/openagents/att-provider-gpu
        RECIPE=deploy/att/Dockerfile.gpu
        CONFIG=deploy/att/cloudbuild-gpu.yaml
        WORKLOAD=clef-decisions-gpu
        ;;
    *)
        LANE=cpu
        cmd=${1:-}
        VM=${ATT_VM:-oa-att-tdx-1}
        MACHINE=${ATT_MACHINE:-c3-standard-8}
        REPO=us-central1-docker.pkg.dev/$PROJECT/openagents/att-provider
        RECIPE=deploy/att/Dockerfile
        CONFIG=deploy/att/cloudbuild.yaml
        WORKLOAD=clef-decisions
        ;;
esac
MAX_RUN=${ATT_MAX_RUN:-1h}
WORKLOAD_SA=oa-att-workload@$PROJECT.iam.gserviceaccount.com
BUILD_SA=projects/$PROJECT/serviceAccounts/oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
SA_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
KEY=${ATT_PUBLISHER_KEY:-$HOME/work/.secrets/att-publisher.key}
NOTICE=${NOTICE:-600}
ROOT=$(git rev-parse --show-toplevel)
STATE=${XDG_STATE_HOME:-$HOME/.local/state}/openagents-deploy
OA_ATT=${OA_ATT:-$ROOT/target/debug/oa-att}
mkdir -p "$STATE"
# The CPU lane keeps its original state file names.
if [ "$LANE" = gpu ]; then SUFFIX=-gpu; else SUFFIX=; fi

g() { CLOUDSDK_CONFIG=$SA_CONFIG gcloud "$@"; }
say() { printf '%s\n' "$*" >&2; }

build() {
    ref=${1:-origin/main}
    git -C "$ROOT" fetch -q origin
    full=$(git -C "$ROOT" rev-parse "$ref^{commit}")
    sha=$(printf '%s' "$full" | cut -c1-10)
    ctx=$(mktemp -d "${TMPDIR:-/tmp}/oa-att$SUFFIX-$sha.XXXXXX")
    git -C "$ROOT" archive "$full" | tar -x -C "$ctx"
    say "building $REPO:$sha from $full"
    (cd "$ctx" && g builds submit --project "$PROJECT" --service-account "$BUILD_SA" \
        --config "$CONFIG" --ignore-file deploy/att/att.gcloudignore \
        --substitutions "_TAG=$sha" . > "$STATE/att-build$SUFFIX-$sha.log" 2>&1) ||
        { say "the build failed; see $STATE/att-build$SUFFIX-$sha.log"; exit 1; }
    rm -rf "$ctx"
    digest=$(g artifacts docker images describe "$REPO:$sha" --format='value(image_summary.digest)')
    printf '%s %s\n' "$full" "$digest" > "$STATE/att-built$SUFFIX-$sha"
    say "image: $REPO@$digest"
    printf '%s\n' "$digest"
}

release() {
    digest=$1
    full=$(git -C "$ROOT" rev-parse "$2^{commit}")
    tag=$(printf '%s' "$full" | cut -c1-10)
    if [ "$LANE" = gpu ]; then
        filter="substitutions._TAG=$tag AND substitutions._LANE=gpu AND status=SUCCESS"
    else
        filter="substitutions._TAG=$tag AND -substitutions._LANE:* AND status=SUCCESS"
    fi
    build_id=$(g builds list --project "$PROJECT" --filter="$filter" --format='value(id)' --limit 1)
    sums=$(g logging read "resource.type=\"build\" AND resource.labels.build_id=\"$build_id\" AND textPayload:\"  /p\"" \
        --project "$PROJECT" --limit 50 --format='value(textPayload)' | grep -Eo '[0-9a-f]{64}  /(psionic-openai-server|pylon)' | sort -u)
    psionic=$(printf '%s\n' "$sums" | awk '$2 == "/psionic-openai-server" { print $1; exit }')
    pylon=$(printf '%s\n' "$sums" | awk '$2 == "/pylon" { print $1; exit }')
    [ -n "$psionic" ] && [ -n "$pylon" ] || { say "no binary digests in build $build_id's log"; exit 1; }
    tmp=$(mktemp -d)
    git -C "$ROOT" show "$full:$RECIPE" > "$tmp/Dockerfile"
    oa=$OA_ATT
    if [ "$LANE" = gpu ]; then
        set -- --workload "$WORKLOAD" --gpu nvidia:cc:H100
        changes="${CHANGES:-Psionic (OpenAgents) serving Clef-Flash Q4_K_M with CUDA on one NVIDIA H100 in confidential-computing mode, inside Intel TDX in Google Confidential Space, answering sealed NIP-DEC decisions; the workload refuses to start unless the Google token says the GPU is in CC mode. Built from $full with $RECIPE.}"
    else
        set --
        changes="${CHANGES:-Psionic (OpenAgents) serving Clef-Flash Q4_K_M on the CPU inside Intel TDX in Google Confidential Space, answering sealed NIP-DEC decisions. Built from $full with $RECIPE.}"
    fi
    "$oa" release "$@" --key "$KEY" --image "$REPO@$digest" \
        --model "clef-flash=sha256:fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c" \
        --component "psionic-openai-server=sha256:$psionic" --component "pylon=sha256:$pylon" \
        --commit "$full" --recipe "$tmp/Dockerfile" --recipe-path "$RECIPE" \
        --changes "$changes" \
        --publish > "$tmp/release.json"
    id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$tmp/release.json")
    published=$(python3 -c 'import json,sys; print(json.loads(json.load(open(sys.argv[1]))["event"]["content"])["published_at"])' "$tmp/release.json")
    say "release $id published at $published"
    # Keep the releases already admitted (and not retired), and add this one.
    keep=""
    for r in $(cat "$STATE/att-admitted$SUFFIX" 2> /dev/null || true); do keep="$keep --release $r"; done
    if [ "$LANE" = gpu ]; then set -- --workload "$WORKLOAD"; else set --; fi
    # shellcheck disable=SC2086
    "$oa" head "$@" --key "$KEY" --release "$id" $keep --generation "$(date +%s)" \
        --notice "$NOTICE" --effective-at $((published + NOTICE)) --publish > "$tmp/head.json"
    say "head $(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$tmp/head.json") admits it at $((published + NOTICE))"
    printf '%s\n' "$id" > "$STATE/att-admitted$SUFFIX"
    rm -rf "$tmp"
    printf '%s\n' "$id"
}

start() {
    release_id=$1
    digest=$2
    if [ "$LANE" = gpu ]; then
        # Spot, stopped (disk kept) on preemption or after MAX_RUN, so it
        # can never run unbounded; the gateway starts it on demand.
        g compute instances create "$VM" --project "$PROJECT" --zone "$ZONE" \
            --machine-type "$MACHINE" --confidential-compute-type=TDX \
            --provisioning-model=SPOT --instance-termination-action=STOP \
            --max-run-duration="$MAX_RUN" --discard-local-ssds-at-termination-timestamp=true \
            --maintenance-policy=TERMINATE --shielded-secure-boot \
            --image-project=confidential-space-images --image-family=confidential-space \
            --boot-disk-size=60GB --service-account="$WORKLOAD_SA" --scopes=cloud-platform \
            --labels=app=oa-att,workload=$WORKLOAD \
            --metadata="^~^tee-image-reference=$REPO@$digest~tee-env-OA_ATT_RELEASE=$release_id~tee-container-log-redirect=true~tee-restart-policy=Always~tee-install-gpu-driver=true"
        return
    fi
    g compute instances create "$VM" --project "$PROJECT" --zone "$ZONE" \
        --machine-type "$MACHINE" --confidential-compute-type=TDX \
        --maintenance-policy=TERMINATE --shielded-secure-boot \
        --image-project=confidential-space-images --image-family=confidential-space \
        --boot-disk-size=40GB --service-account="$WORKLOAD_SA" --scopes=cloud-platform \
        --labels=app=oa-att,workload=clef-decisions \
        --metadata="^~^tee-image-reference=$REPO@$digest~tee-env-OA_ATT_RELEASE=$release_id~tee-container-log-redirect=true~tee-restart-policy=Always"
}

case $cmd in
    build) shift; build "$@" ;;
    release) shift; release "$@" ;;
    start) shift; start "$@" ;;
    retarget) # a stopped VM runs RELEASE_ID's image DIGEST on its next start
        g compute instances add-metadata "$VM" --project "$PROJECT" --zone "$ZONE" \
            --metadata="tee-image-reference=$REPO@$3,tee-env-OA_ATT_RELEASE=$2" ;;
    replace) shift; g compute instances delete "$VM" --project "$PROJECT" --zone "$ZONE" --quiet || true; start "$@" ;;
    stop) if [ "$LANE" = gpu ]; then set -- --discard-local-ssd=true; else set --; fi
        g compute instances stop "$VM" --project "$PROJECT" --zone "$ZONE" "$@" ;;
    resume) g compute instances start "$VM" --project "$PROJECT" --zone "$ZONE" ;;
    delete) g compute instances delete "$VM" --project "$PROJECT" --zone "$ZONE" --quiet ;;
    status) g compute instances describe "$VM" --project "$PROJECT" --zone "$ZONE" \
        --format='value(status,machineType.basename(),confidentialInstanceConfig.confidentialInstanceType,metadata.items[0].value)' ;;
    logs) g logging read "resource.type=\"gce_instance\" AND logName:\"confidential-space-launcher\" AND labels.\"compute.googleapis.com/resource_name\"=\"$VM\"" \
        --project "$PROJECT" --limit "${2:-50}" --format='value(timestamp,jsonPayload.MESSAGE,textPayload)' --freshness=2h ;;
    *) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
