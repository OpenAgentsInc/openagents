#!/usr/bin/env bash
# Fixed-resource infrastructure only. Uses the caller's existing GCloud account;
# never creates Google keys, a VM, reservation, GPU, or broader runtime role.
set -euo pipefail
recovery_here=$(cd "$(dirname "$0")" && pwd)
recovery_root=$(cd "$recovery_here/../.." && pwd)
recovery_project=openagentsgemini recovery_region=us-central1 recovery_zone=us-central1-b
recovery_vm=psionic-rtx6000-pylon recovery_job=pylon-spot-recovery
recovery_runtime=pylon-spot-runtime@openagentsgemini.iam.gserviceaccount.com
recovery_invoker=pylon-spot-scheduler@openagentsgemini.iam.gserviceaccount.com
recovery_marker='OpenAgents fixed Mammoth Spot recovery'
gc(){ gcloud --project="$recovery_project" --quiet "$@"; }
ensure_sa(){
  local recovery_email=$1 recovery_id=$2 recovery_description
  if recovery_description=$(gc iam service-accounts describe "$recovery_email" --format='value(description)' 2>/dev/null);then
    [[ $recovery_description == "$recovery_marker" ]] || { echo 'refusing a service account not owned by this controller' >&2;exit 1; }
  else gc iam service-accounts create "$recovery_id" --display-name="$recovery_marker" --description="$recovery_marker";fi
}
ensure_role(){
  local recovery_role=$1 recovery_file=$2 recovery_permissions=$3 recovery_observed
  if recovery_observed=$(gc iam roles describe "$recovery_role" --format='value(includedPermissions)' 2>/dev/null);then
    [[ $recovery_observed == "$recovery_permissions" ]] || { echo 'refusing an existing role with different permissions' >&2;exit 1; }
  else gc iam roles create "$recovery_role" --file="$recovery_file";fi
}
pause_and_drain(){
  gc scheduler jobs pause "$recovery_job" --location="$recovery_region"
  gc run jobs update "$recovery_job" --region="$recovery_region" --update-env-vars=PYLON_RECOVERY_DRY_RUN=1
  local recovery_active recovery_execution recovery_quiet=0
  recovery_active=$(gc run jobs executions list --job="$recovery_job" --region="$recovery_region" --format=json | jq -r '.[] | select(.status.completionTime == null) | .metadata.name')
  for recovery_execution in $recovery_active;do gc run jobs executions cancel "$recovery_execution" --region="$recovery_region";done
  for ((recovery_poll=0;recovery_poll<45;recovery_poll++));do
    recovery_active=$(gc run jobs executions list --job="$recovery_job" --region="$recovery_region" --format=json | jq -r '.[] | select(.status.completionTime == null) | .metadata.name')
    if [[ -z $recovery_active ]];then recovery_quiet=$((recovery_quiet+1));else recovery_quiet=0;for recovery_execution in $recovery_active;do gc run jobs executions cancel "$recovery_execution" --region="$recovery_region";done;fi
    if ((recovery_poll>=30 && recovery_quiet>=5));then echo 'recovery paused and dry; no active execution after drain window';return;fi
    sleep 1
  done
  echo 'recovery pause requested but an execution is still active; do not stop the VM yet' >&2;exit 1
}
case ${1:-status} in
 deploy)
  (cd "$recovery_root" && shasum -a 256 -c psionic/platform/spot-recovery.sha256)
  ensure_sa "$recovery_runtime" pylon-spot-runtime
  ensure_sa "$recovery_invoker" pylon-spot-scheduler
  ensure_role pylonSpotRecoveryStartOnly "$recovery_here/spot-recovery-runtime-role.yaml" 'compute.instances.get;compute.instances.start'
  ensure_role pylonSpotRecoveryInvokeOnly "$recovery_here/spot-recovery-invoke-role.yaml" 'run.jobs.run'
  gc compute instances add-iam-policy-binding "$recovery_vm" --zone="$recovery_zone" --member="serviceAccount:$recovery_runtime" --role="projects/$recovery_project/roles/pylonSpotRecoveryStartOnly"
  recovery_context=$(mktemp -d);trap 'rm -rf "$recovery_context"' EXIT
  cp "$recovery_here"/spot-recovery.generated.c "$recovery_here"/spot-recovery.h "$recovery_here"/spot-recovery-face.c "$recovery_here"/spot-recovery-run.sh "$recovery_context/"
  cp "$recovery_here/spot-recovery.Dockerfile" "$recovery_context/Dockerfile"
  recovery_digest=$(cat "$recovery_context"/* | shasum -a 256 | cut -c1-16)
  recovery_image="us-central1-docker.pkg.dev/$recovery_project/openagents/pylon-spot-recovery:$recovery_digest"
  gc builds submit "$recovery_context" --tag="$recovery_image" --timeout=300s --region="$recovery_region" --service-account="projects/$recovery_project/serviceAccounts/oa-mvp-automation@$recovery_project.iam.gserviceaccount.com" --default-buckets-behavior=REGIONAL_USER_OWNED_BUCKET
  gc run jobs deploy "$recovery_job" --image="$recovery_image" --region="$recovery_region" --service-account="$recovery_runtime" --tasks=1 --parallelism=1 --max-retries=0 --task-timeout=90s --cpu=1 --memory=512Mi --set-env-vars=PYLON_RECOVERY_DRY_RUN=1 --labels=openagents-component=pylon-spot-recovery
  gc run jobs add-iam-policy-binding "$recovery_job" --region="$recovery_region" --member="serviceAccount:$recovery_invoker" --role="projects/$recovery_project/roles/pylonSpotRecoveryInvokeOnly"
  recovery_url="https://run.googleapis.com/v2/projects/$recovery_project/locations/$recovery_region/jobs/$recovery_job:run"
  recovery_mode=create
  if gc scheduler jobs describe "$recovery_job" --location="$recovery_region" --format='value(name)' >/dev/null 2>&1;then recovery_mode=update;pause_and_drain;fi
  gc scheduler jobs "$recovery_mode" http "$recovery_job" --location="$recovery_region" --description="$recovery_marker" --schedule='*/5 * * * *' --time-zone=Etc/UTC --uri="$recovery_url" --http-method=POST --oauth-service-account-email="$recovery_invoker" --oauth-token-scope=https://www.googleapis.com/auth/cloud-platform --headers=Content-Type=application/json --message-body='{}' --attempt-deadline=30s --max-retry-attempts=0 --max-retry-duration=0s
  pause_and_drain
  echo 'deployed PAUSED in dry mode; verify, then resume'
  ;;
 verify)
  recovery_state=$(gc scheduler jobs describe "$recovery_job" --location="$recovery_region" --format='value(state)')
  if [[ $recovery_state == PAUSED ]];then
    recovery_observed_dry=$(gc run jobs describe "$recovery_job" --region="$recovery_region" --format=json | jq -er '.spec.template.spec.template.spec.containers[0].env[] | select(.name=="PYLON_RECOVERY_DRY_RUN") | .value')
    [[ $recovery_observed_dry == 1 ]] || { echo 'verify refuses to resume a paused active-mode job; use resume deliberately' >&2;exit 1; }
    gc scheduler jobs resume "$recovery_job" --location="$recovery_region"
  fi
  gc scheduler jobs run "$recovery_job" --location="$recovery_region"
  ;;
 pause) pause_and_drain ;;
 resume)
  gc run jobs update "$recovery_job" --region="$recovery_region" --update-env-vars=PYLON_RECOVERY_DRY_RUN=0
  gc scheduler jobs resume "$recovery_job" --location="$recovery_region"
  ;;
 status)
  gc scheduler jobs describe "$recovery_job" --location="$recovery_region" --format='json(name,state,schedule,lastAttemptTime,status)'
  gc run jobs describe "$recovery_job" --region="$recovery_region" --format='json(spec.template.spec.template.spec.containers,spec.template.spec.template.spec.serviceAccountName)'
  gc run jobs executions list --job="$recovery_job" --region="$recovery_region" --limit=5 --format='table(metadata.name,status.completionTime,status.succeededCount,status.failedCount)'
  ;;
 *) echo 'usage: pylon-spot-recovery.sh deploy|verify|pause|resume|status' >&2;exit 2 ;;
esac
