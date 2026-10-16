#!/usr/bin/env bash
# Permit one signet research image to wrap and release envelope keys.
set -euo pipefail
project=openagentsgemini
keyring=layertree-signet
key=enclave-envelope-v2
pool=layertree-signet
region=us-central1
case ${1:-} in
  setup)
    gcloud kms keyrings describe "$keyring" --location "$region" --project "$project" >/dev/null 2>&1 ||
      gcloud kms keyrings create "$keyring" --location "$region" --project "$project"
    gcloud kms keys describe "$key" --keyring "$keyring" --location "$region" --project "$project" >/dev/null 2>&1 ||
      gcloud kms keys create "$key" --keyring "$keyring" --location "$region" --purpose encryption --protection-level software --project "$project"
    gcloud iam workload-identity-pools describe "$pool" --location global --project "$project" >/dev/null 2>&1 ||
      gcloud iam workload-identity-pools create "$pool" --location global --display-name 'LayerTree signet key recovery' --project "$project"
    gcloud iam workload-identity-pools providers describe attestation-verifier --location global --workload-identity-pool "$pool" --project "$project" >/dev/null 2>&1 ||
      gcloud iam workload-identity-pools providers create-oidc attestation-verifier --location global --workload-identity-pool "$pool" \
        --issuer-uri=https://confidentialcomputing.googleapis.com --allowed-audiences=https://sts.googleapis.com \
        --attribute-mapping='google.subject="gcpcs::"+assertion.submods.container.image_digest+"::"+assertion.submods.gce.project_number+"::"+assertion.submods.gce.instance_id,attribute.image_digest=assertion.submods.container.image_digest' \
        --attribute-condition="assertion.swname == 'CONFIDENTIAL_SPACE' && 'STABLE' in assertion.submods.confidential_space.support_attributes && assertion.hwmodel == 'GCP_INTEL_TDX' && assertion.dbgstat == 'disabled-since-boot' && assertion.submods.gce.project_id == 'openagentsgemini'" \
        --project "$project"
    ;;
  status)
    gcloud iam workload-identity-pools providers describe attestation-verifier --workload-identity-pool "$pool" --location global --project "$project"
    gcloud kms keys get-iam-policy "$key" --keyring "$keyring" --location "$region" --project "$project"
    ;;
  admit)
    digest=${2:?provide sha256:DIGEST}
    [[ $digest =~ ^sha256:[a-f0-9]{64}$ ]] || { echo 'Use an immutable image digest.' >&2; exit 2; }
    member="principalSet://iam.googleapis.com/projects/157437760789/locations/global/workloadIdentityPools/$pool/attribute.image_digest/$digest"
    # An upgrade needs a new key and policy, rather than silently adding
    # another image that could recover the previous release's enclave key.
    current=$(gcloud kms keys get-iam-policy "$key" --keyring "$keyring" --location "$region" --project "$project" --format=json)
    python3 - "$member" "$current" <<'PY'
import json,sys
expected=sys.argv[1]
for binding in json.loads(sys.argv[2]).get('bindings',[]):
    if binding['role'].startswith('roles/cloudkms.cryptoKey'):
        if binding.get('condition') or any(member!=expected for member in binding.get('members',[])):
            raise SystemExit('This research key already admits another identity; use a new key for an upgrade.')
PY
    gcloud kms keys add-iam-policy-binding "$key" --keyring "$keyring" --location "$region" --project "$project" \
      --member "$member" --role roles/cloudkms.cryptoKeyEncrypterDecrypter --quiet
    ;;
  *) echo 'Use setup, status, or admit sha256:DIGEST.' >&2; exit 2 ;;
esac
