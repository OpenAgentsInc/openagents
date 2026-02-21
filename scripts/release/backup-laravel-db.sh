#!/usr/bin/env bash
set -euo pipefail

PROJECT="${PROJECT:-openagentsgemini}"
INSTANCE="${INSTANCE:-l402-aperture-db}"
BACKUP_BUCKET="${BACKUP_BUCKET:-gs://openagentsgemini_cloudbuild}"
BACKUP_PREFIX="${BACKUP_PREFIX:-backups/laravel}"
DATABASES_CSV="${DATABASES_CSV:-openagents_web,openagents_web_staging}"
TIMESTAMP="${TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
GRANT_BUCKET_IAM="${GRANT_BUCKET_IAM:-1}"

if ! command -v gcloud >/dev/null 2>&1; then
  echo "error: gcloud is required" >&2
  exit 1
fi

if [[ "${BACKUP_BUCKET}" != gs://* ]]; then
  echo "error: BACKUP_BUCKET must start with gs://" >&2
  exit 1
fi

BUCKET_NO_SCHEME="${BACKUP_BUCKET#gs://}"
SQL_SA="$(gcloud sql instances describe "${INSTANCE}" --project="${PROJECT}" --format='value(serviceAccountEmailAddress)')"

if [[ -z "${SQL_SA}" ]]; then
  echo "error: could not resolve Cloud SQL service account for instance ${INSTANCE}" >&2
  exit 1
fi

if [[ "${GRANT_BUCKET_IAM}" == "1" ]]; then
  echo "[backup] ensuring Cloud SQL service account can write to ${BACKUP_BUCKET}"
  gcloud storage buckets add-iam-policy-binding "${BACKUP_BUCKET}" \
    --member="serviceAccount:${SQL_SA}" \
    --role="roles/storage.objectAdmin" \
    --project="${PROJECT}" >/dev/null
fi

IFS=',' read -r -a RAW_DATABASES <<<"${DATABASES_CSV}"
if [[ ${#RAW_DATABASES[@]} -eq 0 ]]; then
  echo "error: DATABASES_CSV is empty" >&2
  exit 1
fi

SUMMARY_FILE="/tmp/laravel-db-backups-${TIMESTAMP}.tsv"
printf 'database\turi\tbytes\n' >"${SUMMARY_FILE}"

for raw_db in "${RAW_DATABASES[@]}"; do
  db="$(printf '%s' "${raw_db}" | xargs)"
  if [[ -z "${db}" ]]; then
    continue
  fi

  uri="gs://${BUCKET_NO_SCHEME%/}/${BACKUP_PREFIX#/}/${db}-${TIMESTAMP}.sql.gz"
  echo "[backup] exporting ${db} -> ${uri}"
  gcloud sql export sql "${INSTANCE}" "${uri}" \
    --project="${PROJECT}" \
    --database="${db}" \
    --offload

  gcloud storage ls "${uri}" >/dev/null
  size_bytes="$(gcloud storage ls -L "${uri}" | awk '/Content-Length:/ {print $2; exit}')"
  printf '%s\t%s\t%s\n' "${db}" "${uri}" "${size_bytes:-unknown}" >>"${SUMMARY_FILE}"
done

echo "[backup] completed exports"
cat "${SUMMARY_FILE}"
