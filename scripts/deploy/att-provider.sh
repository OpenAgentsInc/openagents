#!/bin/sh
# The NIP-ATT attested decision provider (docs/security/private-inference.md):
# build its image, publish its release, and run it on one Confidential
# Space VM (Intel TDX) in openagentsgemini.
#
#   scripts/deploy/att-provider.sh build [REF]        # default origin/main; prints the digest
#   scripts/deploy/att-provider.sh release DIGEST REF # publish the 3202 release and the 30202 head
#   scripts/deploy/att-provider.sh start RELEASE_ID DIGEST
#   scripts/deploy/att-provider.sh replace RELEASE_ID DIGEST  # delete and start again
#   scripts/deploy/att-provider.sh stop | resume | delete | status | logs
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
VM=${ATT_VM:-oa-att-tdx-1}
MACHINE=${ATT_MACHINE:-c3-standard-8}
REPO=us-central1-docker.pkg.dev/$PROJECT/openagents/att-provider
WORKLOAD_SA=oa-att-workload@$PROJECT.iam.gserviceaccount.com
BUILD_SA=projects/$PROJECT/serviceAccounts/oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
SA_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
KEY=${ATT_PUBLISHER_KEY:-$HOME/work/.secrets/att-publisher.key}
NOTICE=${NOTICE:-600}
ROOT=$(git rev-parse --show-toplevel)
STATE=${XDG_STATE_HOME:-$HOME/.local/state}/openagents-deploy
mkdir -p "$STATE"

g() { CLOUDSDK_CONFIG=$SA_CONFIG gcloud "$@"; }
say() { printf '%s\n' "$*" >&2; }

build() {
    ref=${1:-origin/main}
    git -C "$ROOT" fetch -q origin
    full=$(git -C "$ROOT" rev-parse "$ref^{commit}")
    sha=$(printf '%s' "$full" | cut -c1-10)
    ctx=$(mktemp -d "${TMPDIR:-/tmp}/oa-att-$sha.XXXXXX")
    git -C "$ROOT" archive "$full" | tar -x -C "$ctx"
    say "building $REPO:$sha from $full"
    (cd "$ctx" && g builds submit --project "$PROJECT" --service-account "$BUILD_SA" \
        --config deploy/att/cloudbuild.yaml --ignore-file deploy/att/att.gcloudignore \
        --substitutions "_TAG=$sha" . > "$STATE/att-build-$sha.log" 2>&1) ||
        { say "the build failed; see $STATE/att-build-$sha.log"; exit 1; }
    rm -rf "$ctx"
    digest=$(g artifacts docker images describe "$REPO:$sha" --format='value(image_summary.digest)')
    printf '%s %s\n' "$full" "$digest" > "$STATE/att-built-$sha"
    say "image: $REPO@$digest"
    printf '%s\n' "$digest"
}

release() {
    digest=$1
    full=$(git -C "$ROOT" rev-parse "$2^{commit}")
    build_id=$(g builds list --project "$PROJECT" --filter="substitutions._TAG=$(printf '%s' "$full" | cut -c1-10) AND status=SUCCESS" --format='value(id)' --limit 1)
    sums=$(g logging read "resource.type=\"build\" AND resource.labels.build_id=\"$build_id\" AND textPayload:\"/usr/local/bin\" OR (resource.labels.build_id=\"$build_id\" AND textPayload:\"  /p\")" \
        --project "$PROJECT" --limit 200 --format='value(textPayload)' | grep -Eo '[0-9a-f]{64}  /(psionic-openai-server|pylon)' | sort -u)
    psionic=$(printf '%s\n' "$sums" | awk '$2 == "/psionic-openai-server" { print $1; exit }')
    pylon=$(printf '%s\n' "$sums" | awk '$2 == "/pylon" { print $1; exit }')
    [ -n "$psionic" ] && [ -n "$pylon" ] || { say "no binary digests in build $build_id's log"; exit 1; }
    tmp=$(mktemp -d)
    git -C "$ROOT" show "$full:deploy/att/Dockerfile" > "$tmp/Dockerfile"
    oa="$ROOT/target/debug/oa-att"
    "$oa" release --key "$KEY" --image "$REPO@$digest" \
        --model "clef-flash=sha256:fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c" \
        --component "psionic-openai-server=sha256:$psionic" --component "pylon=sha256:$pylon" \
        --commit "$full" --recipe "$tmp/Dockerfile" --recipe-path deploy/att/Dockerfile \
        --changes "${CHANGES:-Psionic (OpenAgents) serving Clef-Flash Q4_K_M on the CPU inside Intel TDX in Google Confidential Space, answering sealed NIP-DEC decisions. Built from $full with deploy/att/Dockerfile.}" \
        --publish > "$tmp/release.json"
    id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$tmp/release.json")
    published=$(python3 -c 'import json,sys; print(json.loads(json.load(open(sys.argv[1]))["event"]["content"])["published_at"])' "$tmp/release.json")
    say "release $id published at $published"
    # Keep the releases already admitted (and not retired), and add this one.
    keep=""
    for r in $(cat "$STATE/att-admitted" 2> /dev/null || true); do keep="$keep --release $r"; done
    # shellcheck disable=SC2086
    "$oa" head --key "$KEY" --release "$id" $keep --generation "$(date +%s)" \
        --notice "$NOTICE" --effective-at $((published + NOTICE)) --publish > "$tmp/head.json"
    say "head $(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$tmp/head.json") admits it at $((published + NOTICE))"
    printf '%s\n' "$id" > "$STATE/att-admitted"
    rm -rf "$tmp"
    printf '%s\n' "$id"
}

start() {
    release_id=$1
    digest=$2
    g compute instances create "$VM" --project "$PROJECT" --zone "$ZONE" \
        --machine-type "$MACHINE" --confidential-compute-type=TDX \
        --maintenance-policy=TERMINATE --shielded-secure-boot \
        --image-project=confidential-space-images --image-family=confidential-space \
        --boot-disk-size=40GB --service-account="$WORKLOAD_SA" --scopes=cloud-platform \
        --labels=app=oa-att,workload=clef-decisions \
        --metadata="^~^tee-image-reference=$REPO@$digest~tee-env-OA_ATT_RELEASE=$release_id~tee-container-log-redirect=true~tee-restart-policy=Always"
}

case ${1:-} in
    build) shift; build "$@" ;;
    release) shift; release "$@" ;;
    start) shift; start "$@" ;;
    replace) shift; g compute instances delete "$VM" --project "$PROJECT" --zone "$ZONE" --quiet || true; start "$@" ;;
    stop) g compute instances stop "$VM" --project "$PROJECT" --zone "$ZONE" ;;
    resume) g compute instances start "$VM" --project "$PROJECT" --zone "$ZONE" ;;
    delete) g compute instances delete "$VM" --project "$PROJECT" --zone "$ZONE" --quiet ;;
    status) g compute instances describe "$VM" --project "$PROJECT" --zone "$ZONE" \
        --format='value(status,machineType.basename(),confidentialInstanceConfig.confidentialInstanceType,metadata.items[0].value)' ;;
    logs) g logging read "resource.type=\"gce_instance\" AND logName:\"confidential-space-launcher\" AND labels.\"compute.googleapis.com/resource_name\"=\"$VM\"" \
        --project "$PROJECT" --limit "${2:-50}" --format='value(timestamp,jsonPayload.MESSAGE,textPayload)' --freshness=2h ;;
    *) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
