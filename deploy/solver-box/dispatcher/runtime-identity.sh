#!/usr/bin/env bash
# Configure the runtime identity before deploying oa-solver-box (#11807).
# The dispatcher can mint its token; computers cannot read dispatcher secrets.
set -euo pipefail
solver_project=openagentsgemini
solver_runtime=oa-solver-runtime@${solver_project}.iam.gserviceaccount.com
solver_dispatcher=oa-solver-dispatch@${solver_project}.iam.gserviceaccount.com
solver_role=oaSolverRuntime11807

gcloud services enable iamcredentials.googleapis.com --project="$solver_project" --quiet >/dev/null
if ! gcloud iam service-accounts describe "$solver_runtime" --project="$solver_project" >/dev/null 2>&1; then
  gcloud iam service-accounts create oa-solver-runtime --project="$solver_project" \
    --display-name='OpenAgents Solver runtime' \
    --description='Solver computer runtime (#11807): prediction and image downloads only.' --quiet >/dev/null
fi
solver_action=create
if gcloud iam roles describe "$solver_role" --project="$solver_project" >/dev/null 2>&1; then
  solver_action=update
fi
gcloud iam roles "$solver_action" "$solver_role" --project="$solver_project" \
  --title='OpenAgents Solver runtime' \
  --description='Solver computer runtime (#11807): prediction and quota use.' \
  --permissions=aiplatform.endpoints.predict,serviceusage.services.use --stage=GA --quiet >/dev/null
gcloud projects add-iam-policy-binding "$solver_project" --member="serviceAccount:$solver_runtime" \
  --role="projects/$solver_project/roles/$solver_role" --condition=None --quiet >/dev/null
gcloud artifacts repositories add-iam-policy-binding openagents --project="$solver_project" \
  --location=us-central1 --member="serviceAccount:$solver_runtime" \
  --role=roles/artifactregistry.reader --condition=None --quiet >/dev/null
gcloud iam service-accounts add-iam-policy-binding "$solver_runtime" --project="$solver_project" \
  --member="serviceAccount:$solver_dispatcher" --role=roles/iam.serviceAccountTokenCreator \
  --condition=None --quiet >/dev/null
if [[ -n ${SOLVER_RUNTIME_CALLER:-} ]]; then
  case "$SOLVER_RUNTIME_CALLER" in
    user:*|serviceAccount:*) ;;
    *) printf 'SOLVER_RUNTIME_CALLER must name one user or service account.\n' >&2; exit 2 ;;
  esac
  gcloud iam service-accounts add-iam-policy-binding "$solver_runtime" --project="$solver_project" \
    --member="$SOLVER_RUNTIME_CALLER" --role=roles/iam.serviceAccountTokenCreator \
    --condition=None --quiet >/dev/null
fi
printf 'Solver runtime identity configured: %s\n' "$solver_runtime"
