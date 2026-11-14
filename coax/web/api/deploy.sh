#!/usr/bin/env bash
# Deploys the API service (api.c) to Cloud Run: staging first, then
# production with the same image.
#
#   coax/web/api/deploy.sh            # build, staging, smoke, production, smoke
#   coax/web/api/deploy.sh --staging  # stop after staging
#   coax/web/api/deploy.sh --lock TOKEN [--staging]   # the release that holds staging (below)
#
# Every deploy puts the API on staging first, so it asks for staging's lock before anything else
# (#13398, coax/web/room/vm-deploy.sh lock): while a release holds staging, a deploy by anyone else
# stops here, naming the holder. The release itself passes its token (--lock, or OA_STAGING_LOCK).
#
# No test pass gates the deploy (owner, 2026-11-01: the 11-minute test.sh held
# every API change back). Staging's smoke still gates production. The suite's
# report for a commit is main's after-landing check, which runs it in two parts
# on the build hosts whenever a landing reaches the API (coax/remote/after.sh
# state; coax/remote/check.sh api I/N). This script starts no test run of its
# own: the suite never runs on this computer (AGENTS.md, "Build and test").
# OA_API_DEPLOY_TESTS=1 runs test.sh first, here, and gates on it.
#
# Services: oa-api-staging (database oa_analytics_staging) and oa-api
# (oa_analytics), both on the Cloud SQL instance openagents-production-pg,
# serving as the roles oa_api_staging and oa_api (rows only, no DDL), after the
# jobs oa-api-migrate-staging and oa-api-migrate-production apply migrations as
# the owner role oa_analytics (coax/web/api/db/roles.coax). Both on the instance,
# reachable only through the openagents.com load balancer (path rule /api/
# on hosts staging.openagents.com and openagents.com; coax/web/api/lb.sh).
#
# The project agent (docs/project-agent.md): the approved projects are server
# config, never in this repository: Secret Manager oa-agent-projects-staging
# and oa-agent-projects (comma-separated slugs) become OA_AGENT_PROJECTS when
# they exist, and the runner services oa-agent-staging and oa-agent, when
# deployed (coax/web/agent/deploy.sh), become OA_AGENT_WAKE_URL.
#
# Inference credit (inference.c, NIP-X402 over Spark): OA_SPARK_URL is oa-spark's URL when that
# service exists; the API's runtime account may invoke it (coax/web/api/spark/deploy.sh).
#
# Realm scripts (scripts.c): OA_SCRIPT_JOB_URL is oa-script-job's URL when that service exists
# (coax/web/script-job/deploy.sh); until then the compile route answers 503 no-compiler.
#
# Publishing from a drop (rc-publish, drops.c): Secret Manager oa-project-kek-staging and
# oa-project-kek-production (64 hex digits each, made once, never printed) seal each project's
# key; while one is missing that environment's PUBLISH only freezes the draft.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
prod=true; lock="${OA_STAGING_LOCK:-}"
while (( $# )); do
  case "$1" in
    --staging) prod=false ;;
    --lock) lock="${2:?usage: deploy.sh [--lock TOKEN] [--staging]}"; shift ;;
    *) echo 'usage: deploy.sh [--lock TOKEN] [--staging]' >&2; exit 1 ;;
  esac
  shift
done
"$here/../room/vm-deploy.sh" lock check ${lock:+--lock "$lock"} ||
  { echo "api: REFUSED. Staging is held by a release (coax/web/room/vm-deploy.sh lock status): the API is not deployed to staging or production" >&2; exit 1; }
# the lock's own check (coax/web/room/test-vm-deploy.sh): nothing is deployed from a stand-in
[[ -z "${OA_ROOM_VM_STANDIN:-}" ]] || { echo "api: staging's lock allows this deploy (a stand-in: nothing is deployed)"; exit 0; }
commit="$(git -C "$repo" rev-parse --verify HEAD)"
[[ -z "$(git -C "$repo" status --porcelain --untracked-files=normal -- coax/web/api pay)" ]] || { echo 'api: commit API changes before deployment' >&2; exit 1; }
work="$(mktemp -d)"
cleanup() { git -C "$repo" worktree remove --force "$work/source" >/dev/null 2>&1 || true; rm -rf "$work"; }
trap cleanup EXIT
git -C "$repo" worktree add --detach "$work/source" "$commit" >/dev/null
here="$work/source/coax/web/api"
# shellcheck source=deploy-command.sh
source "$here/deploy-command.sh"
# shellcheck source=deploy-rollout.sh
source "$here/deploy-rollout.sh"
project=openagentsgemini region=us-central1
instance="$project:$region:openagents-production-pg"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
if [[ -n "${OA_API_DEPLOY_TESTS:-}" ]]; then "$here/test.sh"; fi

stage="$work/context"
mkdir -p "$stage/src"
cp "$here"/*.c "$here"/*.h "$stage/src/"; cp -R "$here/migrations" "$here/Dockerfile" "$stage/"
# The API's Coax modules become C here, with this commit's checked compiler:
# coax/build.sh installs the release-cached build for these compiler sources
# (seconds) or bootstraps and gates it. The image used to bootstrap the whole
# compiler with every mutant to emit three files, about 35 minutes of every
# deploy; release.sh already gates the compiler before it ships.
(cd "$work/source" && coax/build.sh) > "$work/coax-build.log" 2>&1 || { tail -30 "$work/coax-build.log"; exit 1; }
mkdir -p "$stage/gen"
for m in nip98 mesh-policy mesh-gemini caller-policy anime-director-policy seat-policy progress-policy item-policy moderation-policy character-policy worldbuilder-policy npc-personas auth-policy account-chat-wire chat-history-wire auth-seal agent-wire agent-attest auth-recover-policy auth-recover-crypto auth-life-policy passkey-policy passkey-verify auth-device-policy auth-device-crypto auth-push ticket-sign service-policy service-wire realm-wire seat-wire jobs-wire email-wire packs-wire error-wire mesh-wire auth-wire auth-recover-wire auth-life-wire auth-device-wire passkeys-wire chat-wire player-wire rooms-wire lnaddr-wire makers-wire worldbuilder-wire world-policy script-store; do
  "$work/source/coax/bin/coax" c "$here/$m.coax" --run-steps 250000000 --library --header "$stage/gen/$m.generated.h" > "$stage/gen/$m.generated.c"
done
# The money rules are the Payments context's (pay/, #13320): SOURCE:GENERATED.
for m in lnaddr-policy:lnaddr-policy wallet-policy:wallet-policy wallet-vault-store:wallet-vault; do
  "$work/source/coax/bin/coax" c "$work/source/pay/${m%%:*}.coax" --run-steps 250000000 --library --header "$stage/gen/${m##*:}.generated.h" > "$stage/gen/${m##*:}.generated.c"
done
# A world's own rules are its realm's Coax, the code its page runs (metaverse/splice-bots/, #13412): SOURCE:GENERATED from the root.
for m in metaverse/splice-bots/world-api:splice-world; do
  "$work/source/coax/bin/coax" c "$work/source/${m%%:*}.coax" --run-steps 250000000 --library --header "$stage/gen/${m##*:}.generated.h" > "$stage/gen/${m##*:}.generated.c"
done
tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-api:${commit:0:12}-$(date +%Y%m%d-%H%M%S)"
# Capture the authenticated build result, not the mutable tag's later value.
# The image compiles C only (the Coax modules arrive as C in gen/).
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$tag" --format=json --suppress-logs --quiet \
  --machine-type e2-highcpu-32 --timeout 7200s \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$work/build.json" 2> "$work/build.log" || { tail -30 "$work/build.log"; exit 1; }
image="$(python3 "$here/deploy_verify.py" build-image "$work/build.json" "$tag")"
admins="$(grep -v '^#' "$here/admins.conf" | grep -E '^[0-9a-f]{64}$' | paste -sd, - || true)"
runners="$(grep -v '^#' "$here/runners.conf" 2>/dev/null | grep -E '^[0-9a-f]{64}$' | paste -sd, - || true)"
# Our team (team.conf): kind:value,... ; its visits are our own testing, not usage.
team="$(grep -v '^#' "$here/team.conf" 2>/dev/null | awk 'NF == 2 && ($1 == "player" || $1 == "email_sha256" || $1 == "pubkey") { print $1 ":" $2 }' | paste -sd, - || true)"

# A variable's value on the live service, or nothing: OA_REPO_URL is set by
# coax/web/repo/vm-deploy.sh connect (docs/deployment/repo-vm.md), and --set-env-vars
# would drop it, so each deploy carries it over.
live_env() {  # service name
  gcloud run services describe "$1" --project "$project" --region "$region" --format=json 2>/dev/null |
    python3 -I -c 'import json, sys; e = json.load(sys.stdin)["spec"]["template"]["spec"]["containers"][0].get("env", []); print(next((x.get("value", "") for x in e if x["name"] == sys.argv[1]), ""))' "$2" 2>/dev/null || true
}
deploy() {  # service runtime-secret host agent-projects-secret runner-service drops-folder owner-secret
  # Migrations first, as the owner role, in a job with this image; the service's own role has no
  # DDL and owns nothing (coax/web/api/db/roles.coax, docs/auth/data-and-backups.md). A failed
  # migration stops the deploy before the service changes.
  # The job keeps the account it has: the owner moves it to oa-api-migrator (NEEDS_OWNER.md, #13194).
  mkdir -p "$work/$1"
  local job_sa=()
  gcloud run jobs describe "oa-api-migrate-$6" --project "$project" --region "$region" > /dev/null 2>&1 ||
    job_sa=(--service-account "oa-mvp-automation@$project.iam.gserviceaccount.com")
  if ! gcloud run jobs deploy "oa-api-migrate-$6" --image "$image" --project "$project" --region "$region" \
      --command api --args=--migrations,/migrations,--migrate-only \
      --set-secrets "OA_API_DATABASE=$7:latest" --set-cloudsql-instances "$instance" ${job_sa[@]+"${job_sa[@]}"} \
      --cpu 1 --memory 512Mi --max-retries 0 --task-timeout 900s --execute-now --wait --quiet > "$work/$1/migrate.log" 2>&1; then
    tail -20 "$work/$1/migrate.log" >&2; echo "api: migrations failed on $6; $1 not deployed" >&2; return 1
  fi
  echo "  ok    migrations ($6)"
  local secrets="OA_API_DATABASE=$2:latest" env="^~^OA_ADMIN_PUBKEYS=$admins~OA_AGENT_RUNNER_PUBKEYS=$runners~OA_TEAM=$team~OA_API_ORIGIN=https://$3~OA_DROPS_BUCKET=openagentsgemini-project-drops~OA_DROPS_PREFIX=$6" wake
  # Service alerts (service.c, #13297): production's, opened or resolved, are mailed to the owner's
  # phone. Staging's reach only the log, /status and the agents' inbox (pipeline-lib.sh pl_service_watch).
  [[ "$6" != production ]] || env+="~OA_ALERT_TO=chris@openagents.com"
  # The load checks' slow query (api.c fault_slow, #13300): staging answers it, signed as a room's
  # `fault` write; production never has the route.
  [[ "$6" != staging ]] || env+="~OA_API_FAULTS=1"
  if gcloud secrets describe "$4" --project "$project" >/dev/null 2>&1; then secrets+=",OA_AGENT_PROJECTS=$4:latest"; fi
  # Publishing a project from a drop (drops.c, rc-publish): the key that seals project keys, the relay,
  # the site's bucket; staging's projects go on the relay as private, so no list shows them.
  local kek="oa-project-kek-$6" site="$project-openagents-web" vis=public
  [[ "$6" == staging ]] && site="$project-openagents-web-staging" vis=private
  if gcloud secrets describe "$kek" --project "$project" >/dev/null 2>&1; then
    secrets+=",OA_PROJECT_KEK=$kek:latest"
    env+="~OA_PUBLISH_RELAY=wss://relay.openagents.com~OA_SITE_BUCKET=$site~OA_PUBLISH_VISIBILITY=$vis"
  fi
  # Inference credit (inference.c): our Spark wallet, oa-spark (coax/web/api/spark/deploy.sh), when deployed.
  # Authenticated free SystemOne mesh routes are deployment-owned pinned JSON.
  # Secret Manager avoids separator/escaping drift and survives future deploys.
  local mesh="oa-mesh-routes-$6"
  if gcloud secrets describe "$mesh" --project "$project" >/dev/null 2>&1; then secrets+=",OA_MESH_ROUTES=$mesh:latest"; fi
  # Decisions (mesh.c, docs/research/2026-11-13-gemini-decision-1.md): Gemini Decision 1 answers first (owner,
  # 2026-11-13: "the new primary for everything"), Elephant behind it. A request that names a model still
  # goes only to that model. The key is Google's (Secret Manager oa-gemini-decisions-key); without it the
  # order is not set and the routes answer as before.
  if gcloud secrets describe oa-gemini-decisions-key --project "$project" >/dev/null 2>&1; then
    secrets+=",OA_GEMINI_DECISIONS_KEY=oa-gemini-decisions-key:latest"
    env+="~OA_DECISION_ORDER=gemini-decision-1,elephant-1"
  fi
  # Scheduled jobs (jobs.c, schedules.sh): Cloud Scheduler's caller.
  env+="~OA_JOBS_CALLER=oa-api-jobs@$project.iam.gserviceaccount.com"
  # Character checkpoints (player.c ticket_resume, docs/protocol/character-checkpoint-v2.md): v4 seat
  # tickets carry the last accepted capture, so a room that dies brings its players back where they
  # stood (#13184). Sticky per character from its first v4 ticket: never turn it off again.
  env+="~OA_ROOM_CHECKPOINTS=1"
  # Email through Resend (email.c): the key, and on staging the only address mail may go to (the owner's).
  if gcloud secrets describe openagents-resend-api-key --project "$project" >/dev/null 2>&1; then secrets+=",OA_RESEND_API_KEY=openagents-resend-api-key:latest"; fi
  # Seat tickets (player.c room_ticket): the key oa-room checks them with (coax/web/room/room-ticket.coax).
  if gcloud secrets describe oa-room-ticket-key --project "$project" >/dev/null 2>&1; then secrets+=",OA_ROOM_TICKET_KEY=oa-room-ticket-key:latest"; fi
  # Room keys (#13260, docs/deployment/room-keys.md): the v5 ticket signing key and the room-write master
  # (one, or two joined by "," while one rotates). Each is used once the owner creates it.
  if gcloud secrets describe oa-room-ticket-signing-key --project "$project" >/dev/null 2>&1; then secrets+=",OA_ROOM_TICKET_SIGNING_KEY=oa-room-ticket-signing-key:latest"; fi
  if gcloud secrets describe oa-room-write-master --project "$project" >/dev/null 2>&1; then secrets+=",OA_ROOM_WRITE_MASTER=oa-room-write-master:latest"; fi
  # Staging's allowlist: the owner and the login checks' test inbox (secret oa-email-allow-staging), else the owner.
  if [[ "$6" == staging ]]; then
    if gcloud secrets describe oa-email-allow-staging --project "$project" >/dev/null 2>&1; then secrets+=",OA_EMAIL_ALLOW=oa-email-allow-staging:latest"
    else env+="~OA_EMAIL_ALLOW=chris@openagents.com"; fi
  fi
  # Our own login (auth.c, docs/auth/data-and-backups.md): the key that seals emails and the pepper
  # of their lookups, one pair per environment. Without them, email login says it isn't set up.
  local sfx=""; [[ "$6" == staging ]] && sfx="-staging"
  if gcloud secrets describe "oa-auth-seal-key$sfx" --project "$project" >/dev/null 2>&1; then secrets+=",OA_AUTH_SEAL_KEY=oa-auth-seal-key$sfx:latest"; fi
  if gcloud secrets describe "oa-auth-lookup-pepper$sfx" --project "$project" >/dev/null 2>&1; then secrets+=",OA_AUTH_LOOKUP_PEPPER=oa-auth-lookup-pepper$sfx:latest"; fi
  # Security notices as NIP-17 messages (auth-recover.c): our notice key, one per environment.
  if gcloud secrets describe "oa-auth-notice-nsec$sfx" --project "$project" >/dev/null 2>&1; then secrets+=",OA_AUTH_NOTICE_NSEC=oa-auth-notice-nsec$sfx:latest"; fi
  # Security notices as push to the apps (push.c, #13233): Apple's push key (the .p8 file's text) and
  # its key id, and the Firebase project's id for Android. Without them no push goes, and nothing else changes.
  if gcloud secrets describe oa-apns-key --project "$project" >/dev/null 2>&1; then secrets+=",OA_APNS_KEY=oa-apns-key:latest"; fi
  if gcloud secrets describe oa-apns-key-id --project "$project" >/dev/null 2>&1; then secrets+=",OA_APNS_KEY_ID=oa-apns-key-id:latest"; fi
  if gcloud secrets describe oa-fcm-project --project "$project" >/dev/null 2>&1; then secrets+=",OA_FCM_PROJECT=oa-fcm-project:latest"; fi
  local spark
  spark="$(gcloud run services describe oa-spark --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$spark" ]] && env+="~OA_SPARK_URL=$spark"
  # Realm scripts other people wrote (scripts.c, #13467): the compile job, oa-script-job (coax/web/script-job/deploy.sh),
  # when deployed; the API's runtime account may invoke it. Without it the compile route answers 503 no-compiler.
  local script_job
  script_job="$(gcloud run services describe oa-script-job --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$script_job" ]] && env+="~OA_SCRIPT_JOB_URL=$script_job"
  wake="$(gcloud run services describe "$5" --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$wake" ]] && env+="~OA_AGENT_WAKE_URL=$wake/wake"
  local repo_url; repo_url="$(live_env "$1" OA_REPO_URL)"
  [[ -n "$repo_url" ]] && env+="~OA_REPO_URL=$repo_url"
  local release
  release="$(python3 -c 'import uuid; print(uuid.uuid4())')"
  env+="~OA_BUILD_SHA=$commit~OA_RELEASE_ID=$release"
  api_verified_rollout "$1" "$3" "$image" "$commit" "$release" "$work/$1" \
    --allow-unauthenticated --ingress internal-and-cloud-load-balancing \
    --port 8080 --cpu 1 --memory 512Mi --min-instances 0 --max-instances 4 --concurrency 40 \
    --add-cloudsql-instances "$instance" --set-secrets "$secrets" --set-env-vars "$env"
}
deploy oa-api-staging oa-api-staging-database-url staging.openagents.com oa-agent-projects-staging oa-agent-staging staging oa-analytics-staging-database-url
$prod && deploy oa-api oa-api-database-url openagents.com oa-agent-projects oa-agent production oa-analytics-database-url
echo "api: deployed $image"
