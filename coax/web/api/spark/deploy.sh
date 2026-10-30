#!/usr/bin/env bash
# Deploys the API's Spark wallet (wallet.cjs) to Cloud Run as oa-spark: one
# instance, kept warm, reachable only with an identity token from a service
# account allowed to invoke it (the API's runtime account). Staging and
# production share it: an invoice is credited only by the API database that
# issued it.
#
#   coax/web/api/spark/deploy.sh
#
# The seed: Secret Manager oa-spark-mnemonic (24 English BIP-39 words). The
# first run makes it here, from the checked word list and the OS's random
# bytes, straight into Secret Manager; it is never printed, logged or kept
# on disk. Later runs only read that it exists.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
project=openagentsgemini region=us-central1 service=oa-spark secret=oa-spark-mnemonic
api_sa=157437760789-compute@developer.gserviceaccount.com
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
if ! gcloud secrets describe "$secret" --project "$project" >/dev/null 2>&1; then
  gcloud secrets create "$secret" --project "$project" --replication-policy automatic --quiet >/dev/null
  PYTHONDONTWRITEBYTECODE=1 python3 -I - "$repo/apps/openagents-ios/OpenAgents/Resources/bip39-english.txt" <<'PY' | gcloud secrets versions add "$secret" --project "$project" --data-file=- --quiet >/dev/null
import hashlib, secrets, sys
data = open(sys.argv[1], "rb").read()
assert hashlib.sha256(data).hexdigest() == "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"
words = data.decode().split("\n")[:2048]
assert len(words) == 2048
e = secrets.token_bytes(32)
bits = int.from_bytes(e, "big") << 8 | hashlib.sha256(e).digest()[0]
sys.stdout.write(" ".join(words[(bits >> (11 * (23 - i))) & 2047] for i in range(24)))
PY
  echo "spark: made the seed in Secret Manager ($secret)"
fi
tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/$service:$(git -C "$repo" rev-parse --short=12 HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$here" --project "$project" --region "$region" --tag "$tag" --suppress-logs --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET >/dev/null
gcloud run deploy "$service" --project "$project" --region "$region" --image "$tag" --quiet \
  --no-allow-unauthenticated --port 8080 --cpu 1 --memory 1Gi --min-instances 1 --max-instances 1 --concurrency 16 \
  --set-secrets "SPARK_MNEMONIC=$secret:latest" >/dev/null
gcloud run services add-iam-policy-binding "$service" --project "$project" --region "$region" \
  --member "serviceAccount:$api_sa" --role roles/run.invoker --quiet >/dev/null
url="$(gcloud run services describe "$service" --project "$project" --region "$region" --format 'value(status.url)')"
tok="$(gcloud auth print-identity-token --audiences "$url")"
for i in 1 2 3 4 5 6; do
  if out="$(curl -fsS -m 60 -H "Authorization: Bearer $tok" "$url/health")"; then echo "spark: $url $out"; exit 0; fi
  sleep 10
done
echo "spark: $url did not answer /health" >&2; exit 1
