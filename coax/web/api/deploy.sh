#!/usr/bin/env bash
# Deploys the API service (api.c) to Cloud Run: staging first, then
# production with the same image.
#
#   coax/web/api/deploy.sh            # build, staging, smoke, production, smoke
#   coax/web/api/deploy.sh --staging  # stop after staging
#
# No test pass gates the deploy (owner, 2026-11-01: the 11-minute test.sh held
# every API change back). Staging's smoke still gates production. test.sh runs
# after the rollout, in the background on its own worktree, as a report:
# ${TMPDIR:-/tmp}/api-tests-<commit>.log.
# OA_API_DEPLOY_TESTS=1 runs it first and gates on it, as before.
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
# Publishing from a drop (rc-publish, drops.c): Secret Manager oa-project-kek-staging and
# oa-project-kek-production (64 hex digits each, made once, never printed) seal each project's
# key; while one is missing that environment's PUBLISH only freezes the draft.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
commit="$(git -C "$repo" rev-parse --verify HEAD)"
[[ -z "$(git -C "$repo" status --porcelain --untracked-files=normal -- coax/web/api)" ]] || { echo 'api: commit API changes before deployment' >&2; exit 1; }
work="$(mktemp -d)"
cleanup() { git -C "$repo" worktree remove --force "$work/source" >/dev/null 2>&1 || true; rm -rf "$work"; }
trap cleanup EXIT
git -C "$repo" worktree add --detach "$work/source" "$commit" >/dev/null
here="$work/source/coax/web/api"
# shellcheck source=deploy-command.sh
source "$here/deploy-command.sh"
# shellcheck source=deploy-rollout.sh
source "$here/deploy-rollout.sh"
case "${1:-}" in '') prod=true ;; --staging) prod=false ;; *) echo 'usage: deploy.sh [--staging]' >&2; exit 1 ;; esac
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
for m in nip98 mesh-policy caller-policy anime-director-policy seat-policy progress-policy worldbuilder-policy auth-policy auth-seal agent-wire agent-attest auth-recover-policy auth-recover-crypto auth-life-policy passkey-policy passkey-verify lnaddr-policy auth-device-policy auth-device-crypto ticket-sign; do
  "$work/source/coax/bin/coax" c "$here/$m.coax" --run-steps 250000000 --library --header "$stage/gen/$m.generated.h" > "$stage/gen/$m.generated.c"
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
  local spark
  spark="$(gcloud run services describe oa-spark --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$spark" ]] && env+="~OA_SPARK_URL=$spark"
  wake="$(gcloud run services describe "$5" --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$wake" ]] && env+="~OA_AGENT_WAKE_URL=$wake/wake"
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
if [[ -z "${OA_API_DEPLOY_TESTS:-}" ]]; then
  report="${TMPDIR:-/tmp}/api-tests-${commit:0:12}.log"
  nohup bash -c 'd="$(mktemp -d)"; git -C "$1" worktree add --detach "$d/s" "$2" >/dev/null 2>&1 && "$d/s/coax/web/api/test.sh"; echo "test.sh exit $?"; git -C "$1" worktree remove --force "$d/s" >/dev/null 2>&1; rm -rf "$d"' _ "$repo" "$commit" > "$report" 2>&1 < /dev/null &
  echo "api: test.sh runs after the deploy as a report: $report"
fi
