#!/bin/sh
# Build the website once, test it on staging, and promote that exact image
# to production (docs/deployment/2026-10-09-faster-deploys.md):
#
#   scripts/deploy/web.sh stage [--keep-spec] [REF]   # default origin/main
#   scripts/deploy/web.sh promote [--stack DIGEST] [--serve DIGEST] [--retire-serve] DIGEST
#                                            # no-traffic revision at the `new` tag
#   scripts/deploy/web.sh shift [REVISION]   # 100% of openagents.com to it
#   scripts/deploy/web.sh shift-staging REVISION [SPEC]  # same handoff on staging
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
# such as its storage, stays as it is). The web container also takes REF's
# launcher (deploy/staging/web.sh) and adds the environment variables,
# mounts, and volumes REF's render.py gives it that the live spec lacks;
# nothing the live spec has is changed or removed.
#
# promote: copies the spec of the revision serving production's traffic
# (service `coder`), swaps only the `web` container's image for DIGEST, and
# applies it as a new revision with no traffic, tagged `new` (a host the
# `web` container already serves). --stack also swaps the `gateway`
# sidecar's image for that openagents-stack digest and refreshes its
# launcher from this checkout's deploy/staging/gateway.sh (its environment
# stays as live), and --serve the `coder-serve` sidecar's for that
# `coder` digest (built by the coder repository's ops/deploy.sh --stage).
# coder-serve always gets CODER_ACCOUNTS_URL=http://127.0.0.1:8791, the
# gateway sidecar's account service, so /mcp admits the sessions
# openagents.com's OAuth sign-in issues (#11084). --retire-serve removes
# the `coder-serve` sidecar instead (docs/deployment/coder-serve-retirement.md):
# the container, the web's OPENAGENTS_WEB_UPSTREAM, the web's start-up
# dependency on it, and its database (coder-pg) from the Cloud SQL list;
# the site then answers /v1 from the gateway and the rest of the old
# server's paths with 404. A no-op once the live spec has no coder-serve.
# JSON, not YAML, so CODER_CHAT_SYNC stays
# the string "on". promote then smokes the tag URL with --candidate:
# production's gateway sidecar waits for its revision to serve traffic
# before it takes the account store (GATEWAY_HOLD=serving, #11154), so the
# 8 checks answered through it print WAIT ("expected until traffic") rather
# than FAIL. Any other failure stops promote with exit 1: do not shift.
# SMOKE=0 skips the smoke. WEB_DEPLOY_TAG selects a separate candidate tag
# and host when another deployment is using the default `new` tag.
#
# shift and rollback move all traffic; their default revisions are the ones
# the last promote recorded under $STATE. shift refuses when the revision
# serving production is no longer the one that promote copied (another
# deploy shifted since, and this one would roll it back; FORCE=1 overrides).
# A temporary tag keeps each outgoing revision's web listener warm for five
# minutes while traffic routing changes reach the edge. Production's gateway
# still requires positive traffic; a yielded staging gateway stays idle. Tags
# are removed on success, failure, or interruption, preserving existing tags.
# After moving the traffic it waits for the gateway to answer, then runs the
# full --production smoke against openagents.com, the 8 gateway checks
# included (SMOKE=0 skips it). Every step prints its time.
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
TRAFFIC_ORIGIN=https://openagents.com
DEPLOY_TAG=${WEB_DEPLOY_TAG:-new}
TAG_URL=https://$DEPLOY_TAG---coder-ezxz4mgdsq-uc.a.run.app
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

wait_candidate() {
    candidate_deadline=$(( $(now) + 420 ))
    while [ "$(now)" -lt "$candidate_deadline" ]; do
        if g run revisions describe "$1" --region "$REGION" --project "$PROJECT" \
            --format='json(status.conditions)' > "$STATE/candidate-status.json" 2> "$STATE/candidate-status.log"; then
            candidate_ready=$(python3 -c 'import json,sys; print(next((c.get("status", "") for c in json.load(open(sys.argv[1])).get("status", {}).get("conditions", []) if c.get("type") == "Ready"), ""))' "$STATE/candidate-status.json")
            case "$candidate_ready" in
                True) return 0 ;;
                False) cat "$STATE/candidate-status.json" >&2; return 1 ;;
            esac
        fi
        sleep 5
    done
    say "The candidate did not become ready: $1"
    return 1
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
            --substitutions "_TAG=stg-$sha,_COMMIT=$full" . > "$STATE/build-web-$sha.log" 2>&1) &
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
        # REF's own web container, for its launcher and what it adds.
        python3 "$ctx/deploy/staging/render.py" --revision "$revision" \
            --web-image "$REPO/openagents-web@$web_digest" --stack-image unused \
            --worker-pubkey "$WORKER_PUBKEY" > "$STATE/ref-$sha.json"
        g run services describe "$STAGING" --region "$REGION" --project "$PROJECT" --format=json |
            python3 -c 'import json, sys
d = json.load(sys.stdin)
image, name, ref = sys.argv[1:4]
drop = ("serving.knative.dev/", "client.knative.dev/", "run.googleapis.com/operation-id",
        "run.googleapis.com/ingress-status", "run.googleapis.com/urls",
        "run.googleapis.com/creator", "run.googleapis.com/lastModifier")
keep = lambda m: {k: v for k, v in (m or {}).items() if not k.startswith(drop)}
t = d["spec"]["template"]
web = next(c for c in t["spec"]["containers"] if c["name"] == "web")
web["image"] = image
r = json.load(open(ref))["spec"]["template"]["spec"]
rweb = next(c for c in r["containers"] if c["name"] == "web")
added = []
if web.get("args") != rweb["args"]:
    web["args"] = rweb["args"]
    added.append("launcher")
names = {e["name"] for e in web.setdefault("env", [])}
for e in rweb.get("env", []):
    if e["name"] not in names:
        web["env"].append(e)
        added.append("env " + e["name"])
mounts = {m["mountPath"] for m in web.setdefault("volumeMounts", [])}
for m in rweb.get("volumeMounts", []):
    if m["mountPath"] not in mounts:
        web["volumeMounts"].append(m)
        added.append("mount " + m["mountPath"])
volumes = {v["name"] for v in t["spec"].setdefault("volumes", [])}
for v in r.get("volumes", []):
    if v["name"] not in volumes:
        t["spec"]["volumes"].append(v)
        added.append("volume " + v["name"])
sys.stderr.write("  web container from REF: " + (", ".join(added) or "nothing new") + "\n")
meta = d["metadata"]
print(json.dumps({"apiVersion": "serving.knative.dev/v1", "kind": "Service",
    "metadata": {"name": meta["name"], "namespace": meta["namespace"],
                 "labels": keep(meta.get("labels")), "annotations": keep(meta.get("annotations"))},
    "spec": {"template": {"metadata": {"name": name, "labels": keep(t["metadata"].get("labels")),
                                       "annotations": keep(t["metadata"].get("annotations"))},
                          "spec": t["spec"]},
             "traffic": [{"latestRevision": True, "percent": 100}]}}, indent=2))' \
            "$REPO/openagents-web@$web_digest" "$revision" "$STATE/ref-$sha.json" > "$spec"
    else
        stack_digest=$(digest "$stack")
        [ -n "$stack_digest" ] || { say "No digest for the stack image"; exit 1; }
        python3 "$ctx/deploy/staging/render.py" --revision "$revision" \
            --web-image "$REPO/openagents-web@$web_digest" \
            --stack-image "$REPO/openagents-stack@$stack_digest" \
            --worker-pubkey "$WORKER_PUBKEY" > "$spec"
    fi
    SERVICE=$STAGING
    TRAFFIC_ORIGIN=https://staging.openagents.com
    traffic_to "$revision" "$spec"
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
    stack_image=
    serve_image=
    retire_serve=
    while [ $# -gt 1 ]; do
        case $1 in
            --stack) stack_image=$REPO/openagents-stack@${2##*@}; shift 2 ;;
            --serve) serve_image=$REPO/coder@${2##*@}; shift 2 ;;
            --retire-serve) retire_serve=1; shift ;;
            *) break ;;
        esac
    done
    [ $# -ge 1 ] || { say "promote needs the staged web image's digest"; exit 2; }
    for extra in $stack_image $serve_image; do
        case ${extra##*@} in sha256:*) ;; *) say "Not a digest: $extra"; exit 2 ;; esac
        g artifacts docker images describe "$extra" --format='value(image_summary.digest)' > /dev/null
    done
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
    python3 - "$STATE/service.json" "$STATE/revision.json" "$image" "$name" \
        "$ROOT/deploy/production/web.sh" "$stack_image" "$serve_image" \
        "$ROOT/deploy/staging/gateway.sh" "$retire_serve" > "$spec" << 'PY'
import json, os, re, sys

service, revision = (json.load(open(p)) for p in sys.argv[1:3])
image, name, launcher_path, stack_image, serve_image, gateway_launcher_path, retire_serve = sys.argv[3:10]
# Our own Boat-compatible service on GCE (crates/oa-boat, #11256).
OA_BOAT_BASE = "https://oa-boat-157437760789.us-central1.run.app/api/v1"
tag = os.environ.get("WEB_DEPLOY_TAG", "new")
if not re.fullmatch(r"[a-z](?:[a-z0-9-]{0,61}[a-z0-9])?", tag):
    sys.exit("Choose a candidate tag with lowercase letters, digits, and hyphens.")
candidate_host = tag + "---coder-ezxz4mgdsq-uc.a.run.app"

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
if candidate_host not in args:
    args += ["--public-host", candidate_host]
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
# The vault (#11240): ciphertext in its own bucket, with no soft delete
# and no versions, so a deleted key index is gone.
if "OPENAGENTS_WEB_VAULT_BUCKET" not in env:
    envs.append({"name": "OPENAGENTS_WEB_VAULT_BUCKET",
                 "value": "openagentsgemini-web-vault-prod"})
if "OPENAGENTS_WEB_ANALYTICS_KEY" not in env:
    envs.append({"name": "OPENAGENTS_WEB_ANALYTICS_KEY",
                 "valueFrom": {"secretKeyRef": {"name": "openagents-web-analytics-key",
                                                "key": "latest"}}})
# Agent work (#11162, docs/deployment/agent-work.md): the launcher that
# turns Environments on (deploy/production/web.sh, the site's own arguments
# kept), Boat's key, the setup agent's model, and the gateway's store
# mounted read-only for its house key. Each is a no-op once the spec has it.
launcher = open(launcher_path).read()
if web.get("command") == ["/bin/sh"] and len(args) >= 2 and args[0] == "-c" and args[1] != launcher:
    args[1] = launcher
    sys.stderr.write("  web launcher refreshed from deploy/production/web.sh\n")
# Environments run on our own Boat-compatible service on GCE (#11256), not
# hosted boat.dev: the key is our service token and the base is explicit,
# so even an older image (whose SDK defaulted to boat.dev) reaches it.
for e in envs:
    if e["name"] == "BOAT_API_KEY" and e.get("valueFrom", {}).get("secretKeyRef", {}).get("name") == "boat-api-key":
        e["valueFrom"]["secretKeyRef"]["name"] = "oa-boat-api-key"
        sys.stderr.write("  web env BOAT_API_KEY -> oa-boat-api-key (our own service)\n")
env = {e["name"] for e in envs}
for e in ({"name": "STACK_STATE", "value": "/stack"},
          # The dedicated Computers test account uses the existing access list.
          {"name": "OPENAGENTS_WEB_AGENT_ACCOUNTS", "value": "acct_83d16a35955ef908"},
          {"name": "ENVIRONMENTS_MODEL", "value": "google/gemini-3.8-flash"},
          {"name": "BOAT_API_KEY",
           "valueFrom": {"secretKeyRef": {"name": "oa-boat-api-key", "key": "latest"}}},
          {"name": "BOAT_API_BASE", "value": OA_BOAT_BASE},
          # Gemini on Vertex for the chat's images and PDFs (#11221).
          {"name": "VERTEX_SA_JSON",
           "valueFrom": {"secretKeyRef": {"name": "openagents-vertex-sa-key", "key": "latest"}}},
          # Work hosts that take "work on this issue" runs (#11258).
          {"name": "OPENAGENTS_WORK_HOST_TOKEN",
           "valueFrom": {"secretKeyRef": {"name": "openagents-work-host-token", "key": "latest"}}},
          # Chats on their own computers through oa-computers (#11624,
          # docs/cloud/computers.md), behind the agent-work gate.
          {"name": "OPENAGENTS_WEB_COMPUTERS_URL",
           "value": "https://oa-computers-157437760789.us-central1.run.app"},
          {"name": "OPENAGENTS_WEB_COMPUTERS_TOKEN",
           "valueFrom": {"secretKeyRef": {"name": "oa-computers-token", "key": "latest"}}},
          # The contact form's mail to us, through Resend (/contact).
          {"name": "OPENAGENTS_RESEND_API_KEY",
           "valueFrom": {"secretKeyRef": {"name": "openagents-resend-api-key", "key": "latest"}}}):
    if e["name"] not in env:
        envs.append(e)
        sys.stderr.write(f"  web env {e['name']} added\n")
# The actor runtime (#11253, docs/deployment/actors.md): the account
# database the gateway already reaches through the Cloud SQL connector.
# Mac jobs use it only when the live spec says OPENAGENTS_WEB_MAC_JOBS_ACTORS=1
# (flipped with `gcloud run services update --container web`); a promote
# keeps whatever the live spec has.
instances = (revision["metadata"].get("annotations") or {}).get("run.googleapis.com/cloudsql-instances", "")
if "openagents-production-pg" in instances and "OPENAGENTS_WEB_ACTORS_DATABASE_URL" not in env:
    envs.append({"name": "OPENAGENTS_WEB_ACTORS_DATABASE_URL",
                 "valueFrom": {"secretKeyRef": {"name": "openagents-production-pg-dsn",
                                                "key": "latest"}}})
    sys.stderr.write("  web env OPENAGENTS_WEB_ACTORS_DATABASE_URL added\n")
# Labs use their own indexes in the existing database and a private bucket prefix.
if "openagents-production-pg" in instances:
    for entry in (
        {"name": "OPENAGENTS_WEB_LABS_DATABASE_URL", "valueFrom": {"secretKeyRef": {"name": "openagents-labs-production-pg-url", "key": "latest"}}},
        {"name": "OPENAGENTS_WEB_LABS_BUCKET", "value": "openagentsgemini-web-chats-prod"},
        {"name": "OPENAGENTS_WEB_LABS_SIGNING_ACCOUNT", "value": "openagents-labs-files@openagentsgemini.iam.gserviceaccount.com"},
    ):
        if entry["name"] not in env:
            envs.append(entry)
            sys.stderr.write("  web env " + entry["name"] + " added\n")
mounts = web.setdefault("volumeMounts", [])
if any(v["name"] == "stack" for v in spec.get("volumes", [])) and \
        not any(m["mountPath"] == "/stack" for m in mounts):
    mounts.append({"name": "stack", "mountPath": "/stack", "readOnly": True})
    sys.stderr.write("  web mount /stack (read-only) added\n")
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
# The sidecars' images, when asked (--stack, --serve); everything else in
# them stays as the live revision has it.
for container, new in (("gateway", stack_image), ("coder-serve", serve_image)):
    if not new:
        continue
    c = next((c for c in spec["containers"] if c["name"] == container), None)
    if c is None:
        sys.exit(f"refusing: the serving revision has no {container} container")
    sys.stderr.write(f"  {container} image: {c['image']}\n      -> {new}\n")
    c["image"] = new
    # A new stack image runs this checkout's gateway launcher too, so its
    # settings (the free allowance, #11264) arrive with it; the launcher's
    # environment stays as the live revision has it.
    if container == "gateway":
        gateway_launcher = open(gateway_launcher_path).read()
        gargs = c.get("args") or []
        if c.get("command") == ["/bin/sh"] and len(gargs) >= 2 and gargs[0] == "-c" \
                and gargs[1] != gateway_launcher:
            gargs[1] = gateway_launcher
            sys.stderr.write("  gateway launcher refreshed from deploy/staging/gateway.sh\n")
# coder-serve's /mcp checks a `sess_` bearer (an MCP app's OAuth sign-in,
# #11084) with the account service on the gateway sidecar. A no-op once
# the live spec has it.
names = {c["name"] for c in spec["containers"]}
if {"coder-serve", "gateway"} <= names:
    serve = next(c for c in spec["containers"] if c["name"] == "coder-serve")
    serve_env = serve.setdefault("env", [])
    if not any(e["name"] == "CODER_ACCOUNTS_URL" for e in serve_env):
        serve_env.append({"name": "CODER_ACCOUNTS_URL", "value": "http://127.0.0.1:8791"})
        sys.stderr.write("  coder-serve env CODER_ACCOUNTS_URL added\n")
# The production test account (#11601): agents' and gates' checks run as
# an account made with the operator sign-up token (Secret Manager
# openagents-gateway-production-smoke-signup-token, never printed), never
# as the owner. The gateway takes the token; the site forwards the
# operator's POST /v1/accounts and a key's POST /v1/sessions. No-ops once
# the live spec has them.
if "gateway" in names:
    gw = next(c for c in spec["containers"] if c["name"] == "gateway")
    gw_env = gw.setdefault("env", [])
    if not any(e["name"] == "SMOKE_SIGNUP_TOKEN" for e in gw_env):
        gw_env.append({"name": "SMOKE_SIGNUP_TOKEN",
                       "valueFrom": {"secretKeyRef": {
                           "name": "openagents-gateway-production-smoke-signup-token",
                           "key": "latest"}}})
        sys.stderr.write("  gateway env SMOKE_SIGNUP_TOKEN added (operator sign-up)\n")
    if not any(e["name"] == "OPENAGENTS_WEB_API_OPERATOR_SIGNUP" for e in envs):
        envs.append({"name": "OPENAGENTS_WEB_API_OPERATOR_SIGNUP", "value": "1"})
        sys.stderr.write("  web env OPENAGENTS_WEB_API_OPERATOR_SIGNUP added\n")
annotations = keep(revision["metadata"].get("annotations"), SYSTEM)
# --retire-serve: no coder-serve sidecar (docs/deployment/coder-serve-retirement.md).
if retire_serve and any(c["name"] == "coder-serve" for c in spec["containers"]):
    if serve_image:
        sys.exit("refusing: --serve and --retire-serve together")
    spec["containers"] = [c for c in spec["containers"] if c["name"] != "coder-serve"]
    web["env"] = [e for e in web.get("env", []) if e["name"] != "OPENAGENTS_WEB_UPSTREAM"]
    depends = json.loads(annotations.get("run.googleapis.com/container-dependencies", "{}"))
    depends = {k: [d for d in v if d != "coder-serve"] for k, v in depends.items()}
    depends = {k: v for k, v in depends.items() if v}
    if depends:
        annotations["run.googleapis.com/container-dependencies"] = json.dumps(depends)
    else:
        annotations.pop("run.googleapis.com/container-dependencies", None)
    instances = [i for i in annotations.get("run.googleapis.com/cloudsql-instances", "").split(",")
                 if i.strip() and not i.strip().endswith(":coder-pg")]
    if instances:
        annotations["run.googleapis.com/cloudsql-instances"] = ",".join(instances)
    else:
        annotations.pop("run.googleapis.com/cloudsql-instances", None)
    sys.stderr.write("  coder-serve sidecar removed (container, OPENAGENTS_WEB_UPSTREAM, "
                     "dependency, coder-pg)\n")
traffic = []
for entry in service["spec"].get("traffic", []):
    if entry.get("latestRevision"):
        sys.exit("refusing: a traffic entry follows the latest revision, which would move traffic")
    entry = dict(entry)
    if entry.get("tag") == tag:
        del entry["tag"]
        if not entry.get("percent"):
            continue
    traffic.append(entry)
traffic.append({"revisionName": name, "tag": tag})
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
                "annotations": annotations,
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
    if [ "$DEPLOY_TAG" = new ]; then
        apply run services replace "$spec" --region "$REGION" --project "$PROJECT"
    else
        apply run services replace "$spec" --region "$REGION" --project "$PROJECT" --async
        [ -n "${DRY_RUN:-}" ] || wait_candidate "$name"
    fi
    took "production revision (no traffic)" "$t"
    [ -z "${DRY_RUN:-}" ] || { say "  spec: $spec"; return 0; }
    printf '%s\n' "$previous" > "$STATE/$SERVICE.previous"
    printf '%s\n' "$name" > "$STATE/$SERVICE.candidate"
    took "promote total" "$started"
    say "Candidate $name at $TAG_URL (no traffic)."
    if [ "${SMOKE:-1}" != 0 ]; then
        t=$(now)
        if ! "$ROOT/scripts/smoke/staging.sh" "$TAG_URL" --candidate; then
            took "candidate smoke" "$t"
            say "The candidate failed checks that do not wait for traffic: do not shift."
            exit 1
        fi
        took "candidate smoke" "$t"
    fi
    say "Next: scripts/deploy/web.sh shift $name"
    say "Rollback: scripts/deploy/web.sh rollback $previous"
}

# After a shift: wait for the gateway to take the store (it polls every 5 s
# for traffic), then smoke openagents.com with every check counting.
after_shift() {
    [ -z "${DRY_RUN:-}" ] && [ "${SMOKE:-1}" != 0 ] || return 0
    t=$(now)
    tries=0
    until [ "$(curl -s -o /dev/null -w '%{http_code}' https://openagents.com/openapi.json)" = 200 ]; do
        tries=$((tries + 1))
        [ "$tries" -lt 36 ] || { say "the gateway did not answer within 3 minutes"; break; }
        sleep 5
    done
    took "gateway answering" "$t"
    t=$(now)
    if ! "$ROOT/scripts/smoke/staging.sh" https://openagents.com --production; then
        took "openagents.com smoke" "$t"
        say "openagents.com failed its smoke after the shift. Rollback: scripts/deploy/web.sh rollback"
        exit 1
    fi
    took "openagents.com smoke" "$t"
}

traffic_to() {
    # Forward a signal sent to this script's PID to the one handoff it owns.
    traffic_handoff "$@" &
    handoff_pid=$!
    interrupt_handoff() {
        signal_result=$1
        trap - INT TERM
        kill -TERM "$handoff_pid" 2> /dev/null || true
        wait "$handoff_pid" 2> /dev/null || true
        exit "$signal_result"
    }
    trap 'interrupt_handoff 130' INT
    trap 'interrupt_handoff 143' TERM
    handoff_result=0
    wait "$handoff_pid" || handoff_result=$?
    trap - INT TERM
    return "$handoff_result"
}

traffic_handoff() {
    # Traffic changes are not instantaneous. Keep the outgoing listeners
    # available while the edge updates, without keeping their gateways active.
    # https://docs.cloud.google.com/run/docs/rollouts-rollbacks-traffic-migration
    # https://docs.cloud.google.com/run/docs/configuring/min-instances
    traffic=$(g run services describe "$SERVICE" --region "$REGION" --project "$PROJECT" \
        --format='json(status.traffic)')
    assignments=$(printf '%s\n' "$traffic" | python3 -c '
import json, re, secrets, sys
traffic = json.load(sys.stdin)["status"]["traffic"]
outgoing = sorted({entry["revisionName"] for entry in traffic
                   if entry.get("percent", 0) > 0 and entry["revisionName"] != sys.argv[1]})
if any(not re.fullmatch(r"[a-z][a-z0-9-]{0,62}", revision) for revision in outgoing):
    sys.exit("Cannot identify the outgoing web revisions.")
used = {entry.get("tag") for entry in traffic}
prefix = "oa-drain-" + secrets.token_hex(5)
tags = [f"{prefix}-{index}" for index in range(len(outgoing))]
if used.intersection(tags):
    sys.exit("The temporary web tags already exist. Try again.")
print(",".join(f"{tag}={revision}" for tag, revision in zip(tags, outgoing)))
' "$1")
    for assignment in $(printf '%s' "$assignments" | tr ',' ' '); do
        revision=${assignment#*=}
        g run revisions describe "$revision" --region "$REGION" --project "$PROJECT" \
            --format='json(metadata.annotations)' | python3 -c '
import json, sys
annotations = json.load(sys.stdin).get("metadata", {}).get("annotations", {})
try:
    warm = int(annotations.get("autoscaling.knative.dev/minScale", "0")) >= 1
except (TypeError, ValueError):
    warm = False
if not warm:
    sys.exit(f"Cannot keep outgoing revision {sys.argv[1]} warm: revision minScale must be at least 1.")
' "$revision"
    done
    drain_tags=$(printf '%s' "$assignments" | sed 's/=[^,]*//g')
    drain_sleep=
    drain_attempted=
    cleanup_drain() {
        result=$?
        trap - 0 INT TERM
        if [ -n "$drain_sleep" ]; then
            kill "$drain_sleep" 2> /dev/null || true
            wait "$drain_sleep" 2> /dev/null || true
        fi
        if [ -n "$drain_tags" ] && [ -n "$drain_attempted" ]; then
            if ! apply run services update-traffic "$SERVICE" --region "$REGION" --project "$PROJECT" \
                --remove-tags "$drain_tags"; then
                say "Could not remove temporary web tags: $drain_tags"
                say "Remove them with: gcloud run services update-traffic $SERVICE --region $REGION --project $PROJECT --remove-tags $drain_tags"
                result=1
            fi
        fi
        exit "$result"
    }
    trap cleanup_drain 0
    trap 'exit 130' INT
    trap 'exit 143' TERM
    t=$(now)
    if [ -n "${2:-}" ]; then
        # A service replacement must carry the tags too. Install the warm
        # tags before replacing, since replace has no tag-before-traffic rule.
        printf '%s\n' "$traffic" | python3 -c '
import json, sys
path, destination, assignments, service = sys.argv[1:]
traffic = json.load(sys.stdin)["status"]["traffic"]
tags = {entry["tag"]: entry["revisionName"] for entry in traffic if entry.get("tag")}
for pair in filter(None, assignments.split(",")):
    tag, revision = pair.split("=")
    tags[tag] = revision
with open(path) as source:
    spec = json.load(source)
if spec["metadata"]["name"] != service or spec["spec"]["template"]["metadata"]["name"] != destination:
    sys.exit("The web service or revision in the replacement spec differs. No traffic was changed.")
spec["spec"]["traffic"] = [{"revisionName": destination, "percent": 100}] + [
    {"tag": tag, "revisionName": revision} for tag, revision in tags.items()]
with open(path, "w") as output:
    json.dump(spec, output)
' "$2" "$1" "$assignments" "$SERVICE"
        if [ -n "$assignments" ]; then
            drain_attempted=1
            apply run services update-traffic "$SERVICE" --region "$REGION" --project "$PROJECT" \
                --update-tags "$assignments"
        fi
        apply run services replace "$2" --region "$REGION" --project "$PROJECT"
    elif [ -n "$assignments" ]; then
        # gcloud applies tags before the traffic percentage in this request.
        drain_attempted=1
        apply run services update-traffic "$SERVICE" --region "$REGION" --project "$PROJECT" \
            --update-tags "$assignments" --to-revisions "$1=100"
    else
        apply run services update-traffic "$SERVICE" --region "$REGION" --project "$PROJECT" \
            --to-revisions "$1=100"
    fi
    took "traffic to $1" "$t"
    if [ -n "$drain_tags" ] && [ -z "${DRY_RUN:-}" ]; then
        say "Keeping outgoing web listeners warm for 300 s."
        drain_now() { python3 -c 'import time; print(time.monotonic_ns() // 1000000000)'; }
        deadline=$(( $(drain_now) + 300 ))
        probe_result=0
        for path in /health /docs /cli/install.sh; do
            code=$(curl -sS --connect-timeout 5 --max-time 10 \
                -H 'X-OpenAgents-Test: 1' -o /dev/null -w '%{http_code}' \
                "$TRAFFIC_ORIGIN$path") || code=000
            if [ "$code" != 200 ]; then
                say "Immediate public read failed: $path returned $code. Keeping the outgoing listeners until the drain ends."
                probe_result=1
            fi
        done
        while :; do
            remaining=$(( deadline - $(drain_now) ))
            [ "$remaining" -gt 0 ] || break
            [ "$remaining" -le 30 ] || remaining=30
            sleep "$remaining" &
            drain_sleep=$!
            wait "$drain_sleep"
            drain_sleep=
        done
        exit "$probe_result"
    fi
}

case ${1:-} in
    stage) shift; stage "$@" ;;
    promote) shift; promote "$@" ;;
    shift)
        rev=${2:-$(cat "$STATE/$SERVICE.candidate" 2> /dev/null || true)}
        [ -n "$rev" ] || { say "shift needs a revision"; exit 2; }
        serving=$(serving_revision)
        copied=$(cat "$STATE/$SERVICE.previous" 2> /dev/null || true)
        candidate=$(cat "$STATE/$SERVICE.candidate" 2> /dev/null || true)
        if [ "$rev" = "$candidate" ] && [ -n "$copied" ] && [ "$serving" != "$copied" ] &&
            [ -z "${FORCE:-}" ]; then
            say "refusing: production now serves $serving, not $copied, which promote copied."
            say "Another deploy shifted since; shifting would roll it back. FORCE=1 overrides."
            exit 1
        fi
        say "previous: $serving (rollback: scripts/deploy/web.sh rollback)"
        traffic_to "$rev"
        after_shift ;;
    rollback)
        rev=${2:-$(cat "$STATE/$SERVICE.previous" 2> /dev/null || true)}
        [ -n "$rev" ] || { say "rollback needs a revision"; exit 2; }
        traffic_to "$rev" ;;
    shift-staging)
        [ -n "${2:-}" ] || { say "shift-staging needs a revision"; exit 2; }
        SERVICE=$STAGING
        TRAFFIC_ORIGIN=https://staging.openagents.com
        traffic_to "$2" "${3:-}" ;;
    *) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
