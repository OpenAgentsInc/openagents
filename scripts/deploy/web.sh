#!/bin/sh
# Build the website once, test it on staging, and promote that exact image
# to production (docs/deployment/2026-10-09-faster-deploys.md):
#
#   scripts/deploy/web.sh stage [--keep-spec] [REF]   # default origin/main
#   scripts/deploy/web.sh promote DIGEST     # no-traffic revision at the `new` tag
#   scripts/deploy/web.sh shift [REVISION]   # 100% of openagents.com to it
#   scripts/deploy/web.sh rollback [REVISION]
#
# stage: exports REF with `git archive` (a dirty checkout never leaks into
# the image), builds the web image (crates/openagents-web/cloudbuild.yaml,
# tag stg-SHA) and the staging stack image (deploy/staging/cloudbuild.yaml,
# tag SHA) in parallel, skipping either when that tag is already in
# Artifact Registry, deploys both by digest to openagents-web-1-staging with
# REF's deploy/staging/render.py, and runs scripts/smoke/staging.sh.
# --keep-spec builds only the web image and swaps it into the live staging
# spec, keeping everything else (another change being tried on staging,
# such as its storage, stays as it is).
#
# promote: copies the spec of the revision serving production's traffic
# (service `coder`), swaps only the `web` container's image for DIGEST, and
# applies it as a new revision with no traffic, tagged `new` (a host the
# `web` container already serves). JSON, not YAML, so CODER_CHAT_SYNC stays
# the string "on". Then run the smoke against the tag URL:
#   scripts/smoke/staging.sh https://new---coder-ezxz4mgdsq-uc.a.run.app --production
#
# shift and rollback move all traffic; their default revisions are the ones
# the last promote recorded under $STATE. Every step prints its time.
#
# Reads run as the automation account (CLOUDSDK_CONFIG, default
# ~/work/.secrets/gcloud-sa-config). Where that account is refused actAs on
# production's runtime account, the write is retried once with the default
# gcloud account (chris@), as docs/deployment/openagents-web.md describes.
# DRY_RUN=1 writes the production spec and prints the writes without them.
set -eu

PROJECT=openagentsgemini
REGION=us-central1
REPO=us-central1-docker.pkg.dev/$PROJECT/openagents
SERVICE=coder
STAGING=openagents-web-1-staging
TAG_URL=https://new---coder-ezxz4mgdsq-uc.a.run.app
BUILD_SA=projects/$PROJECT/serviceAccounts/oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
# The staging chat worker's public key (deploy/staging/README.md).
WORKER_PUBKEY=${STAGING_WORKER_PUBKEY:-be4c57cadade24f6a3dd95a6e5a7414cbeab2d0df219c31157b850d4b6718b9c}
SA_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
STATE=${XDG_STATE_HOME:-$HOME/.local/state}/openagents-deploy
ROOT=$(git rev-parse --show-toplevel)
mkdir -p "$STATE"

g() { CLOUDSDK_CONFIG=$SA_CONFIG gcloud "$@"; }
now() { date +%s; }
say() { printf '%s\n' "$*" >&2; }
took() { say "  $1: $(( $(now) - $2 )) s"; }

# A write that may need chris@: the automation account first, then once
# with the default gcloud configuration if it was refused.
apply() {
    if [ -n "${DRY_RUN:-}" ]; then say "  DRY_RUN: gcloud $*"; return 0; fi
    log=$STATE/apply.log
    if g "$@" > "$log" 2>&1; then cat "$log" >&2; return 0; fi
    cat "$log" >&2
    if grep -qi 'actAs\|PERMISSION_DENIED\|does not have permission' "$log"; then
        say "The automation account was refused; retrying with the default gcloud account."
        env -u CLOUDSDK_CONFIG gcloud "$@"
    else
        return 1
    fi
}

digest() { # IMAGE:TAG -> sha256:..., or nothing
    g artifacts docker images describe "$1" --format='value(image_summary.digest)' 2> /dev/null || true
}

serving_revision() {
    g run services describe "$SERVICE" --region "$REGION" --project "$PROJECT" --format=json |
        python3 -c 'import json,sys
t=[e for e in json.load(sys.stdin)["status"]["traffic"] if e.get("percent")]
t.sort(key=lambda e: -e["percent"])
print(t[0]["revisionName"] if t else "")'
}

stage() {
    keep=
    if [ "${1:-}" = --keep-spec ]; then keep=1; shift; fi
    ref=${1:-origin/main}
    started=$(now)
    git -C "$ROOT" fetch -q origin
    full=$(git -C "$ROOT" rev-parse "$ref^{commit}")
    sha=$(printf '%s' "$full" | cut -c1-10)
    ctx=$(mktemp -d "${TMPDIR:-/tmp}/oa-stage-$sha.XXXXXX")
    git -C "$ROOT" archive "$full" | tar -x -C "$ctx"
    say "stage $sha ($ref) from $ctx"
    web=$REPO/openagents-web:stg-$sha
    stack=$REPO/openagents-stack:$sha
    t=$(now)
    pids=
    if [ -z "$(digest "$web")" ]; then
        (cd "$ctx" && g builds submit --project "$PROJECT" --service-account "$BUILD_SA" \
            --config crates/openagents-web/cloudbuild.yaml \
            --ignore-file crates/openagents-web/web.gcloudignore \
            --substitutions "_TAG=stg-$sha" . > "$STATE/build-web-$sha.log" 2>&1) &
        pids="$pids $!"
        say "  building $web (log: $STATE/build-web-$sha.log)"
    else
        say "  $web is already built"
    fi
    if [ -n "$keep" ]; then
        say "  --keep-spec: the live staging stack image stays"
    elif [ -z "$(digest "$stack")" ]; then
        (cd "$ctx" && g builds submit --project "$PROJECT" --service-account "$BUILD_SA" \
            --config deploy/staging/cloudbuild.yaml \
            --ignore-file deploy/staging/stack.gcloudignore \
            --substitutions "_TAG=$sha" . > "$STATE/build-stack-$sha.log" 2>&1) &
        pids="$pids $!"
        say "  building $stack (log: $STATE/build-stack-$sha.log)"
    else
        say "  $stack is already built"
    fi
    for pid in $pids; do
        wait "$pid" || { say "A build failed; see the logs above."; exit 1; }
    done
    took build "$t"
    web_digest=$(digest "$web")
    [ -n "$web_digest" ] || { say "No digest for the web image"; exit 1; }
    t=$(now)
    spec=$STATE/staging-$sha.json
    revision=$STAGING-$sha-$(date -u +%H%M%S)
    if [ -n "$keep" ]; then
        g run services describe "$STAGING" --region "$REGION" --project "$PROJECT" --format=json |
            python3 -c 'import json, sys
d = json.load(sys.stdin)
image, name = sys.argv[1:3]
drop = ("serving.knative.dev/", "client.knative.dev/", "run.googleapis.com/operation-id",
        "run.googleapis.com/ingress-status", "run.googleapis.com/urls",
        "run.googleapis.com/creator", "run.googleapis.com/lastModifier")
keep = lambda m: {k: v for k, v in (m or {}).items() if not k.startswith(drop)}
t = d["spec"]["template"]
next(c for c in t["spec"]["containers"] if c["name"] == "web")["image"] = image
meta = d["metadata"]
print(json.dumps({"apiVersion": "serving.knative.dev/v1", "kind": "Service",
    "metadata": {"name": meta["name"], "namespace": meta["namespace"],
                 "labels": keep(meta.get("labels")), "annotations": keep(meta.get("annotations"))},
    "spec": {"template": {"metadata": {"name": name, "labels": keep(t["metadata"].get("labels")),
                                       "annotations": keep(t["metadata"].get("annotations"))},
                          "spec": t["spec"]},
             "traffic": [{"latestRevision": True, "percent": 100}]}}, indent=2))' \
            "$REPO/openagents-web@$web_digest" "$revision" > "$spec"
    else
        stack_digest=$(digest "$stack")
        [ -n "$stack_digest" ] || { say "No digest for the stack image"; exit 1; }
        python3 "$ctx/deploy/staging/render.py" --revision "$revision" \
            --web-image "$REPO/openagents-web@$web_digest" \
            --stack-image "$REPO/openagents-stack@$stack_digest" \
            --worker-pubkey "$WORKER_PUBKEY" > "$spec"
    fi
    g run services replace "$spec" --region "$REGION" --project "$PROJECT" >&2
    took "staging deploy" "$t"
    printf '%s\n' "$web_digest" > "$STATE/staged-$sha"
    t=$(now)
    smoke=0
    "$ctx/scripts/smoke/staging.sh" || smoke=$?
    took smoke "$t"
    took "stage total" "$started"
    rm -rf "$ctx"
    say "web image: $REPO/openagents-web@$web_digest"
    [ "$smoke" -eq 0 ] || { say "The staging smoke failed; do not promote."; exit 1; }
    say "Promote with: scripts/deploy/web.sh promote $web_digest"
}

promote() {
    [ $# -ge 1 ] || { say "promote needs the staged web image's digest"; exit 2; }
    want=${1##*@}
    case $want in sha256:*) ;; *) say "Not a digest: $1"; exit 2 ;; esac
    started=$(now)
    image=$REPO/openagents-web@$want
    g artifacts docker images describe "$image" --format='value(image_summary.digest)' > /dev/null
    previous=$(serving_revision)
    [ -n "$previous" ] || { say "No revision serves $SERVICE's traffic"; exit 1; }
    name=$SERVICE-web-$(printf '%s' "$want" | cut -c8-17)-$(date -u +%Y%m%d%H%M%S)
    spec=$STATE/$name.json
    g run services describe "$SERVICE" --region "$REGION" --project "$PROJECT" --format=json > "$STATE/service.json"
    g run revisions describe "$previous" --region "$REGION" --project "$PROJECT" --format=json > "$STATE/revision.json"
    python3 - "$STATE/service.json" "$STATE/revision.json" "$image" "$name" > "$spec" << 'PY'
import json, sys

service, revision = (json.load(open(p)) for p in sys.argv[1:3])
image, name = sys.argv[3:5]

def keep(entries, drop):
    return {k: v for k, v in (entries or {}).items() if not k.startswith(drop)}

SYSTEM = ("serving.knative.dev/", "client.knative.dev/", "run.googleapis.com/operation-id",
          "run.googleapis.com/ingress-status", "run.googleapis.com/urls",
          "run.googleapis.com/creator", "run.googleapis.com/lastModifier")
meta = service["metadata"]
spec = revision["spec"]
web = next(c for c in spec["containers"] if c["name"] == "web")
old = json.loads(json.dumps(web))
web["image"] = image
# Arguments the current site binary needs that older production specs
# lack; each is a no-op once the live spec carries it. A public host
# requires a chat bucket (production's own, which the runtime account may
# write), and the image ships the chat and Grow Little Bunny builds.
args = web.setdefault("args", [])
env = {e["name"] for e in web.get("env", [])}
if "--chat-bucket" not in args and "OPENAGENTS_WEB_CHAT_BUCKET" not in env:
    args += ["--chat-bucket", "openagentsgemini-web-chats-prod"]
for flag, path in (("--chat-build", "/srv/chat"), ("--bunny", "/srv/bunny")):
    if flag not in args:
        args += [flag, path]
# First-party analytics (#11153, docs/deployment/analytics.md): production's
# private bucket and the dashboard key.
envs = web.setdefault("env", [])
if "OPENAGENTS_WEB_ANALYTICS_BUCKET" not in env:
    envs.append({"name": "OPENAGENTS_WEB_ANALYTICS_BUCKET",
                 "value": "openagentsgemini-web-analytics-prod"})
if "OPENAGENTS_WEB_ANALYTICS_KEY" not in env:
    envs.append({"name": "OPENAGENTS_WEB_ANALYTICS_KEY",
                 "valueFrom": {"secretKeyRef": {"name": "openagents-web-analytics-key",
                                                "key": "latest"}}})
# The coder-serve sidecar's secrets come from Secret Manager, never as
# plain values in the spec (same values; a no-op once the live spec has it).
SIDECAR_SECRETS = {"CODER_GITHUB_CLIENT_SECRET": "coder-github-client-secret"}
# No third-party analytics (privacy policy section 5): the sidecar gets no
# PostHog token or host, so it sends no PostHog events (a no-op once the
# live spec lacks them).
THIRD_PARTY_ANALYTICS = ("POSTHOG_PROJECT_TOKEN", "POSTHOG_HOST")
for c in spec["containers"]:
    if c["name"] != "coder-serve":
        continue
    dropped = [e["name"] for e in c.get("env", []) if e["name"] in THIRD_PARTY_ANALYTICS]
    c["env"] = [e for e in c.get("env", []) if e["name"] not in THIRD_PARTY_ANALYTICS]
    for var in dropped:
        sys.stderr.write(f"  coder-serve {var} removed (no third-party analytics)\n")
    for e in c.get("env", []):
        if e["name"] in SIDECAR_SECRETS and "value" in e:
            del e["value"]
            e["valueFrom"] = {"secretKeyRef": {"name": SIDECAR_SECRETS[e["name"]], "key": "latest"}}
            sys.stderr.write(f"  coder-serve {e['name']} -> Secret Manager\n")
traffic = []
for entry in service["spec"].get("traffic", []):
    if entry.get("latestRevision"):
        sys.exit("refusing: a traffic entry follows the latest revision, which would move traffic")
    entry = dict(entry)
    if entry.get("tag") == "new":
        del entry["tag"]
        if not entry.get("percent"):
            continue
    traffic.append(entry)
traffic.append({"revisionName": name, "tag": "new"})
out = {
    "apiVersion": "serving.knative.dev/v1",
    "kind": "Service",
    "metadata": {
        "name": meta["name"],
        "namespace": meta["namespace"],
        "labels": keep(meta.get("labels"), SYSTEM),
        "annotations": keep(meta.get("annotations"), SYSTEM),
    },
    "spec": {
        "template": {
            "metadata": {
                "name": name,
                "labels": keep(revision["metadata"].get("labels"), SYSTEM + ("cloud.googleapis.com/",)),
                "annotations": keep(revision["metadata"].get("annotations"), SYSTEM),
            },
            "spec": spec,
        },
        "traffic": traffic,
    },
}
print(json.dumps(out, indent=2))
sys.stderr.write(f"  web image: {old['image']}\n          -> {image}\n")
if old.get("args") != args:
    sys.stderr.write(f"  web args added: {' '.join(args[len(old.get('args') or []):])}\n")
PY
    say "  serving now: $previous"
    t=$(now)
    apply run services replace "$spec" --region "$REGION" --project "$PROJECT"
    took "production revision (no traffic)" "$t"
    [ -z "${DRY_RUN:-}" ] || { say "  spec: $spec"; return 0; }
    printf '%s\n' "$previous" > "$STATE/$SERVICE.previous"
    printf '%s\n' "$name" > "$STATE/$SERVICE.candidate"
    took "promote total" "$started"
    say "Candidate $name at $TAG_URL (no traffic). Next:"
    say "  scripts/smoke/staging.sh $TAG_URL --production"
    say "  scripts/deploy/web.sh shift $name"
    say "Rollback: scripts/deploy/web.sh rollback $previous"
}

traffic_to() {
    t=$(now)
    apply run services update-traffic "$SERVICE" --region "$REGION" --project "$PROJECT" \
        --to-revisions "$1=100"
    took "traffic to $1" "$t"
}

case ${1:-} in
    stage) shift; stage "$@" ;;
    promote) shift; promote "$@" ;;
    shift)
        rev=${2:-$(cat "$STATE/$SERVICE.candidate" 2> /dev/null || true)}
        [ -n "$rev" ] || { say "shift needs a revision"; exit 2; }
        say "previous: $(serving_revision) (rollback: scripts/deploy/web.sh rollback)"
        traffic_to "$rev" ;;
    rollback)
        rev=${2:-$(cat "$STATE/$SERVICE.previous" 2> /dev/null || true)}
        [ -n "$rev" ] || { say "rollback needs a revision"; exit 2; }
        traffic_to "$rev" ;;
    *) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
