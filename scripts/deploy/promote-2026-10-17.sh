#!/bin/sh
# Production rollout of 2026-10-17, run once by the owner as chris@ (the
# automation account is refused actAs on production's runtime account):
#
#   oa-computers   free cloud allowance and its migration 17 (#11981)
#   web            app visitor chats (#11978), free Our cloud (#11981),
#                  Sign in with Apple's device route (#11980), /layertree
#                  (#11987), first-minute analytics (#11983)
#   gateway        POST /v1/sessions/apple (#11980)
#
# It promotes exactly the images staging serves now, nothing rebuilt, and
# refuses unless staging is healthy and was built from 56932eae75 or later
# (web and gateway: #11991, #11980, /layertree) and 3d30b4ec68 or later
# (oa-computers: #11981). Order matters: oa-computers first, because the web
# turns free cloud on (OPENAGENTS_CLOUD_HOST_FREE_ALLOWANCE=1). Run from an
# openagents checkout:
#
#   sh scripts/deploy/promote-2026-10-17.sh
#
# A failed check after the web shift moves openagents.com back on its own and
# prints the oa-computers rollback.
set -eu

WEB_AT_LEAST=56932eae75
COMPUTERS_AT_LEAST=3d30b4ec68
PROJECT=openagentsgemini
REGION=us-central1
REPO=us-central1-docker.pkg.dev/$PROJECT/openagents
STAGING=openagents-web-1-staging
ORIGIN=https://openagents.com
SA_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}

cd "$(git rev-parse --show-toplevel)"
git fetch -q origin
# Python's own OpenSSL on this Mac has no CA file; the smokes use the system's.
export SSL_CERT_FILE=${SSL_CERT_FILE:-/etc/ssl/cert.pem}
when() { TZ=America/Chicago date '+%H:%M:%S %Z'; }
stop() { echo "$(when) REFUSED: $*" >&2; exit 1; }
g() { CLOUDSDK_CONFIG=$SA_CONFIG gcloud "$@"; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

# The commit an image was built from, by its tag (stg-SHA for the web, SHA
# for the stack), as a full commit ID.
commit_of() { # IMAGE_NAME DIGEST
    tag=$(g artifacts docker tags list "$REPO/$1" --filter="version~$2" --format='value(tag)' 2> /dev/null |
        sed 's/^stg-//' | grep -E '^[0-9a-f]{10}$' | head -1)
    [ -n "$tag" ] || stop "no commit tag for $1@$2"
    git rev-parse --verify -q "$tag^{commit}" || stop "commit $tag of $1 is not in this checkout"
}
at_least() { # COMMIT FLOOR NAME
    git merge-base --is-ancestor "$2" "$1" || stop "staging's $3 was built from $1, before $2"
    git merge-base --is-ancestor "$1" origin/main || stop "staging's $3 commit $1 is not on main"
}

echo "$(when) reading staging"
g run services describe "$STAGING" --region "$REGION" --project "$PROJECT" --format=json > /tmp/oa-promote-staging.json
eval "$(python3 - /tmp/oa-promote-staging.json << 'PY'
import json, sys
d = json.load(open(sys.argv[1]))
ready = next((c["status"] for c in d["status"]["conditions"] if c["type"] == "Ready"), "")
images = {c["name"]: c["image"] for c in d["spec"]["template"]["spec"]["containers"]}
serving = [t for t in d["status"]["traffic"] if t.get("percent") == 100]
print(f'READY={ready}')
print(f'SERVING={serving[0]["revisionName"] if serving else ""}')
print(f'LATEST={d["status"].get("latestReadyRevisionName", "")}')
print(f'WEB={images.get("web", "").split("@")[-1]}')
print(f'STACK={images.get("gateway", "").split("@")[-1]}')
PY
)"
[ "$READY" = True ] || stop "staging is not ready"
[ -n "$SERVING" ] && [ "$SERVING" = "$LATEST" ] || stop "staging's traffic is not all on its latest revision"
case "$WEB$STACK" in sha256:*sha256:*) ;; *) stop "staging's web or gateway image is not pinned by digest" ;; esac
web_commit=$(commit_of openagents-web "$WEB")
stack_commit=$(commit_of openagents-stack "$STACK")
at_least "$web_commit" "$WEB_AT_LEAST" web
at_least "$stack_commit" "$WEB_AT_LEAST" gateway
COMPUTERS=$(g run services describe oa-computers-staging --region "$REGION" --project "$PROJECT" \
    --format='value(spec.template.metadata.labels.openagents-commit)')
at_least "$COMPUTERS" "$COMPUTERS_AT_LEAST" oa-computers
c=$(code -X POST https://staging.openagents.com/v1/visitors)
[ "$c" = 201 ] || stop "staging POST /v1/visitors answered $c"
c=$(code https://staging.openagents.com/)
[ "$c" = 200 ] || stop "staging / answered $c"
COMPUTERS_PREVIOUS=$(g run services describe oa-computers --region "$REGION" --project "$PROJECT" \
    --format='value(status.traffic[0].revisionName)')
echo "  staging $SERVING is healthy"
echo "  web     $WEB ($web_commit)"
echo "  gateway $STACK ($stack_commit)"
echo "  oa-computers $COMPUTERS (production now $COMPUTERS_PREVIOUS)"

echo "$(when) 1/3 oa-computers $COMPUTERS (migration 17, free allowance)"
env -u CLOUDSDK_CONFIG deploy/oa-computers/deploy.sh production "$COMPUTERS"

echo "$(when) 2/3 web and gateway candidate (no traffic)"
scripts/deploy/web.sh promote --stack "$STACK" "$WEB"

echo "$(when) 3/3 traffic to the candidate, then the openagents.com smoke"
fail() {
    echo "$(when) FAILED: $*"
    echo "Moving openagents.com back to the previous revision."
    scripts/deploy/web.sh rollback || true
    echo "oa-computers rollback, if free cloud misbehaves:"
    echo "  gcloud run services update-traffic oa-computers --region $REGION --project $PROJECT --to-revisions $COMPUTERS_PREVIOUS=100"
    exit 1
}
scripts/deploy/web.sh shift || fail "the shift or its openagents.com smoke"

echo "$(when) checks"
c=$(code -X POST "$ORIGIN/v1/visitors"); echo "  POST /v1/visitors: $c"
[ "$c" = 201 ] || fail "POST /v1/visitors answered $c"
# The phone's Apple sign-in goes through the web (the gateway's
# /v1/sessions/apple is first-party and never forwarded, so it is a 404 from
# outside); a bad token comes back from the gateway as 400. Since #12005
# removed Sign in with Apple, a staging image built after it answers 410.
c=$(code -X POST -H 'Content-Type: application/json' \
    -d '{"identity_token":"not-a-token","nonce":"check","app":"openagents-mobile","computer":"deploy-check"}' \
    "$ORIGIN/v1/device/apple")
echo "  POST /v1/device/apple with a bad token: $c"
[ "$c" = 400 ] || [ "$c" = 410 ] || fail "POST /v1/device/apple answered $c"
for path in /layertree /layertree/start.js /layertree/layertree_web.js /layertree/layertree_web_bg.wasm; do
    c=$(code "$ORIGIN$path"); echo "  $path: $c"
    [ "$c" = 200 ] || fail "$path answered $c"
done
c=$(code "$ORIGIN/v1/cloud-host"); echo "  GET /v1/cloud-host signed out: $c (the web's 401)"
[ "$c" = 401 ] || fail "/v1/cloud-host answered $c"
# A throwaway app visitor's question is answered (not the failure line).
python3 - "$ORIGIN" << 'PY' || fail "an app visitor's chat was not answered"
import json, sys, time, uuid, urllib.request
base = sys.argv[1]
def call(path, method="GET", body=None, token=None):
    h = {"Content-Type": "application/json", "X-OpenAgents-Test": "1"}
    if token:
        h["Authorization"] = "Bearer " + token
    data = json.dumps(body).encode() if body is not None else (b"" if method == "POST" else None)
    with urllib.request.urlopen(urllib.request.Request(base + path, data=data, method=method, headers=h), timeout=60) as r:
        return json.loads(r.read() or b"{}")
token = call("/v1/visitors", "POST")["token"]
chat = call("/v1/threads", "POST", {"request_id": str(uuid.uuid4()), "text": "In one short sentence, what is OpenAgents?"}, token)["id"]
deadline = time.time() + 150
while time.time() < deadline:
    time.sleep(3)
    replies = [m for m in call(f"/v1/threads/{chat}", token=token).get("messages", []) if m.get("role") != "user"]
    if replies and replies[-1].get("text", "").strip():
        text = replies[-1]["text"].strip()
        print("  app visitor answer:", text[:100].replace("\n", " "))
        sys.exit(1 if "couldn't answer" in text else 0)
print("  app visitor: no answer in 150 s")
sys.exit(1)
PY
echo "$(when) done. Free cloud is on; NEEDS_OWNER.md says how to turn it off."
