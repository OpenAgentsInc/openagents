#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: stages the image's files and runs gcloud, as every other deploy.sh)
# Deploys the compile job for realm scripts other people wrote (GitHub #13467; docs/protocol/world-scripts-v1.md,
# "The compile job") to Cloud Run as oa-script-job: the job behind a socket (serve.c), in an image that holds
# the Coax compiler, the Wasm toolchain and only the repository files the job reads (Dockerfile).
#
#   coax/web/script-job/deploy.sh               build the image, deploy oa-script-job, smoke it
#   coax/web/script-job/deploy.sh --stage DIR   only write the image's build context into DIR, for
#                                               `docker build DIR` on a build host; no gcloud
#
# What holds the job in (docs/protocol/world-scripts-v1.md has the table):
#   - its own service account, oa-script-job, with no role: the metadata server answers the job, but only
#     with that account, which can do nothing. No secret is mounted and no variable names one.
#   - no way out: every packet goes into the network oa-script-job (Direct VPC egress, all traffic), which
#     has no Cloud NAT, no Private Google Access and a firewall rule denying all egress. The job checks it
#     (OA_SCRIPT_JOB_SANDBOX=closed, coax/web/script-job.sh): where it reaches an address outside, it refuses
#     to run, so a service deployed with egress open compiles nothing.
#   - one request at a time an instance (--concurrency 1), 75 s a request, 2 GiB, as the user nobody.
#   - only the API's runtime account may invoke it (IAM). Ingress stays open to authenticated calls, as for
#     oa-spark: "internal" ingress would need the API itself on VPC egress.
# After this, coax/web/api/deploy.sh sets the API's OA_SCRIPT_JOB_URL to this service.
#
# The account is made once by someone who may make accounts (the automation account may not):
#   gcloud iam service-accounts create oa-script-job --project openagentsgemini --display-name "Realm script compile job (no roles)"
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"

# The image's build context: the Dockerfile, the compiler's sources (to name and check its published
# build), the files the job reads at their repository paths, and the host's C.
stage() {
  local out="$1" f
  [[ -x "$coax" ]] || { echo "script-job: no compiler at $coax (run coax/build.sh)" >&2; exit 1; }
  rm -rf "$out"; mkdir -p "$out/compiler" "$out/repo" "$out/host"
  cp "$here/Dockerfile" "$out/"
  cp "$here/serve.c" "$repo/coax/web/api/script-job-run.c" "$repo/coax/web/api/script-job-run.h" "$out/host/"
  # the compiler's inputs as coax/provenance.sh source-fingerprint names them, and the two scripts that fetch and check it
  (cd "$repo" && tar -cf - coax/src coax/bootstrap coax/lib/compiler coax/lib/proto coax/*.sh coax/*.c coax/*.coax \
     coax/lib/crypto/sha256-kernel.coax scripts/coax-release.sh) | tar -xf - -C "$out/compiler"
  # the job: its wrapper, the tool runner, the toolchain's paths, the platform's C, the script runtime, and
  # every Coax file the job's tools and the sample script read (coax deps), with nothing else of the repository
  {
    printf '%s\n' coax/web/script-job.sh coax/web/tool.sh coax/web/toolchain.sh coax/lib/crypto/entropy.coax
    (cd "$repo" && find coax/platform metaverse-engine/script/rt -type f)
    for f in metaverse-engine/tools/script-job.coax metaverse-engine/tools/script-build.coax metaverse-engine/script/examples/pacer.coax coax/lib/crypto/entropy.coax; do
      "$coax" deps "$repo/$f" 2> /dev/null | grep -o '"sources":\[[^]]*\]' | grep -o '"/[^"]*"' | tr -d '"' | sed "s|^$repo/||"
    done
  } | sort -u > "$out/files"
  grep -q '^metaverse-engine/tools/script-build.coax$' "$out/files" || { echo "script-job: coax deps named no files" >&2; exit 1; }
  (cd "$repo" && tar -cf - -T "$out/files") | tar -xf - -C "$out/repo"
  rm "$out/files"
}

if [[ "${1:-}" == --stage ]]; then stage "${2:?usage: deploy.sh --stage DIR}"; echo "script-job: the build context is in $2"; exit 0; fi
[[ $# -eq 0 ]] || { echo 'usage: deploy.sh [--stage DIR]' >&2; exit 1; }

project=openagentsgemini region=us-central1 service=oa-script-job network=oa-script-job
sa="oa-script-job@$project.iam.gserviceaccount.com"
api_sa=157437760789-compute@developer.gserviceaccount.com
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
[[ -z "$(git -C "$repo" status --porcelain --untracked-files=normal -- coax/web/script-job coax/web/api/script-job-run.c coax/web/api/script-job-run.h coax/web/script-job.sh metaverse-engine/tools metaverse-engine/script)" ]] ||
  { echo 'script-job: commit the job'"'"'s changes before deployment' >&2; exit 1; }
gcloud iam service-accounts describe "$sa" --project "$project" > /dev/null 2>&1 || {
  echo "script-job: no account $sa. Someone who may make accounts runs, once:" >&2
  echo "  gcloud iam service-accounts create oa-script-job --project $project --display-name \"Realm script compile job (no roles)\"" >&2
  exit 1; }

# The network with no way out: one subnet for Direct VPC egress, no NAT, no Private Google Access, egress denied.
if ! gcloud compute networks describe "$network" --project "$project" > /dev/null 2>&1; then
  gcloud compute networks create "$network" --project "$project" --subnet-mode custom --quiet > /dev/null
fi
if ! gcloud compute networks subnets describe "$network" --project "$project" --region "$region" > /dev/null 2>&1; then
  gcloud compute networks subnets create "$network" --project "$project" --region "$region" --network "$network" \
    --range 10.73.0.0/26 --no-enable-private-ip-google-access --quiet > /dev/null
fi
if ! gcloud compute firewall-rules describe "$network-deny-egress" --project "$project" > /dev/null 2>&1; then
  gcloud compute firewall-rules create "$network-deny-egress" --project "$project" --network "$network" \
    --direction EGRESS --action DENY --rules all --destination-ranges 0.0.0.0/0 --priority 100 --quiet > /dev/null
fi

(cd "$repo" && coax/build.sh) > /dev/null 2>&1 || { echo 'script-job: coax/build.sh failed' >&2; exit 1; }
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
stage "$work/context"
tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/$service:$(git -C "$repo" rev-parse --short=12 HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$work/context" --project "$project" --region "$region" --tag "$tag" --suppress-logs --quiet \
  --machine-type e2-highcpu-8 --timeout 3600s \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$work/build.log" 2>&1 || { tail -30 "$work/build.log"; exit 1; }
gcloud run deploy "$service" --project "$project" --region "$region" --image "$tag" --quiet \
  --service-account "$sa" --no-allow-unauthenticated --ingress all \
  --network "$network" --subnet "$network" --vpc-egress all-traffic --execution-environment gen2 \
  --port 8080 --cpu 1 --memory 2Gi --concurrency 1 --timeout 75 --min-instances 0 --max-instances 4 > /dev/null
gcloud run services add-iam-policy-binding "$service" --project "$project" --region "$region" \
  --member "serviceAccount:$api_sa" --role roles/run.invoker --quiet > /dev/null
url="$(gcloud run services describe "$service" --project "$project" --region "$region" --format 'value(status.url)')"

# The smoke: the sample script compiles; a source with a #foreign is refused with the job's words.
tok="$(gcloud auth print-identity-token --audiences "$url")"
code="$(curl -sS -m 90 -o "$work/pacer.wasm" -D "$work/pacer.head" -w '%{http_code}' -H "Authorization: Bearer $tok" \
  --data-binary @"$repo/metaverse-engine/script/examples/pacer.coax" "$url/compile/pacer")"
[[ "$code" == 200 ]] && grep -qi '^x-script-manifest: {' "$work/pacer.head" ||
  { echo "script-job: $url answered $code for the sample script:" >&2; head -c 600 "$work/pacer.wasm" >&2; exit 1; }
code="$(curl -sS -m 90 -o "$work/refused" -w '%{http_code}' -H "Authorization: Bearer $tok" \
  --data-binary $'#script "s";\n#load "api.coax";\nsecret :: () -> s32 #foreign;\n' "$url/compile/s")"
[[ "$code" == 422 ]] || { echo "script-job: $url answered $code for a #foreign source" >&2; exit 1; }
echo "script-job: $url compiles (pacer: $(wc -c < "$work/pacer.wasm" | tr -d ' ') bytes) and refuses a #foreign source; next, coax/web/api/deploy.sh sets OA_SCRIPT_JOB_URL"
