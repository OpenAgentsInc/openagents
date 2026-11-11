#!/usr/bin/env bash
# Firm reason: 5, a thin wrapper around gcloud (build, jobs, schedules).
# Builds the backup image and (re)makes its Cloud Run jobs and schedules
# (docs/auth/data-and-backups.md, #13194). Idempotent.
#
#   coax/web/api/backup/deploy.sh             # build, jobs, schedules
#   coax/web/api/backup/deploy.sh --run dump   # then run one: dump or drill, and wait
#
#   oa-db-dump   nightly 09:30 UTC (scheduler oa-db-dump-nightly): backup dump (backup.coax)
#   oa-db-dump-auth  every six hours (scheduler oa-db-dump-auth-6h): the auth schema alone, #13200
#   oa-db-drill  the 3rd of each month 12:00 UTC (scheduler oa-db-drill-monthly): backup drill
#
# Both run as the API's runtime account, read the database as oa_dump
# (secret oa-db-dump-database-url, coax/web/api/db/roles.coax), and write only
# new objects into gs://openagentsgemini-db-backups (until the owner moves
# them to gs://openagents-backups-db), whose 35-day retention
# policy is locked: nobody can delete or overwrite a dump before it's 35 days old.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../../.." && pwd)"
project=openagentsgemini region=us-central1
instance="$project:$region:openagents-production-pg"
sa="157437760789-compute@developer.gserviceaccount.com"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
# The bucket the jobs already write to wins: the owner moves them to the backup
# project's gs://openagents-backups-db (NEEDS_OWNER.md, #13194), and a redeploy keeps that.
bucket="$(gcloud run jobs describe oa-db-dump --project "$project" --region "$region" --format=json 2>/dev/null |
  python3 -c 'import json,sys
try: env = json.load(sys.stdin)["spec"]["template"]["spec"]["template"]["spec"]["containers"][0].get("env", [])
except Exception: env = []
print(next((e.get("value", "") for e in env if e.get("name") == "OA_BACKUP_BUCKET"), ""))' || true)"
bucket="${bucket:-openagentsgemini-db-backups}"
commit="$(git -C "$here" rev-parse --short=12 HEAD)"
tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-db-backup:$commit-$(date +%Y%m%d-%H%M%S)"
# The image's context: backup.coax as C from this checkout's checked compiler (coax/build.sh), and
# the tool platform's C that coax/web/tool.sh links; the Dockerfile compiles them for Linux.
stage="$(mktemp -d)"; trap 'rm -rf "$stage" "$stage.log"' EXIT
mkdir -p "$stage/gen"; cp "$here/Dockerfile" "$here/recipients.txt" "$stage/"
(cd "$repo" && coax/build.sh) > "$stage.log" 2>&1 || { tail -30 "$stage.log"; exit 1; }
"$repo/coax/bin/coax" c "$here/backup.coax" --library > "$stage/gen/backup.c"
"$repo/coax/bin/coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$stage/gen/entropy-policy.h" > "$stage/gen/entropy.c"
for f in tool-native process-native store-file-native read-file-native socket-native entropy-native entropy-foreign agent-signer-none; do
  cp "$repo/coax/platform/$f.c" "$stage/gen/"
done
cp "$repo"/coax/platform/*.h "$stage/gen/"
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$tag" --suppress-logs --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > /dev/null
echo "backup: built $tag; bucket gs://$bucket"

# In the backup project only the owner sets IAM; there these fail and the owner's grants stand.
gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa" --role roles/storage.objectCreator > /dev/null 2>&1 ||
  echo "backup: gs://$bucket's IAM is the owner's"
gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa" --role roles/storage.objectViewer > /dev/null 2>&1 || true
gcloud secrets add-iam-policy-binding oa-db-backup-drill-age-key --project "$project" --member "serviceAccount:$sa" \
  --role roles/secretmanager.secretAccessor --condition=None > /dev/null

job() {  # name mode cpu memory timeout retries secrets [prefix schema]
  gcloud run jobs deploy "$1" --image "$tag" --project "$project" --region "$region" --args "$2" \
    --service-account "$sa" --set-cloudsql-instances "$instance" --cpu "$3" --memory "$4" \
    --task-timeout "$5" --max-retries "$6" --set-secrets "$7" \
    --set-env-vars "OA_BACKUP_BUCKET=$bucket,OA_BACKUP_PREFIX=${8:-oa_analytics},OA_BACKUP_SCHEMA=${9:-}" --quiet > /dev/null
  gcloud run jobs add-iam-policy-binding "$1" --project "$project" --region "$region" \
    --member "serviceAccount:$sa" --role roles/run.invoker > /dev/null
  echo "backup: job $1"
}
job oa-db-dump dump 1 1Gi 1800s 1 "OA_DUMP_DATABASE=oa-db-dump-database-url:latest"
job oa-db-dump-auth dump 1 512Mi 900s 1 "OA_DUMP_DATABASE=oa-db-dump-database-url:latest" oa_analytics-auth auth
job oa-db-drill drill 2 4Gi 3600s 0 "OA_DUMP_DATABASE=oa-db-dump-database-url:latest,OA_DRILL_AGE_KEY=oa-db-backup-drill-age-key:latest"

schedule() {  # name cron job
  local verb=create
  gcloud scheduler jobs describe "$1" --project "$project" --location "$region" > /dev/null 2>&1 && verb=update
  gcloud scheduler jobs "$verb" http "$1" --project "$project" --location "$region" --schedule "$2" --time-zone Etc/UTC \
    --uri "https://run.googleapis.com/v2/projects/$project/locations/$region/jobs/$3:run" --http-method POST \
    --oauth-service-account-email "$sa" --quiet > /dev/null
  echo "backup: schedule $1 ($2 UTC)"
}
schedule oa-db-dump-nightly "30 9 * * *" oa-db-dump
schedule oa-db-dump-auth-6h "15 */6 * * *" oa-db-dump-auth
schedule oa-db-drill-monthly "0 12 3 * *" oa-db-drill

if [[ "${1:-}" == --run ]]; then
  gcloud run jobs execute "oa-db-${2:?dump, dump-auth or drill}" --project "$project" --region "$region" --wait --quiet
fi
