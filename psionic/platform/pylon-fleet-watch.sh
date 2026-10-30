#!/usr/bin/env bash
# Deployment effects only. Runtime admission, window and recovery decisions
# belong to psionic/lib/ops/fleet-watch.coax. No VM creation or Google keys.
set -euo pipefail
watch_here=$(cd "$(dirname "$0")" && pwd)
watch_project=openagentsgemini watch_region=us-central1 watch_job=pylon-fleet-watch
watch_runtime=pylon-watch-runtime@openagentsgemini.iam.gserviceaccount.com
watch_scheduler=pylon-watch-scheduler@openagentsgemini.iam.gserviceaccount.com
watch_marker='OpenAgents bounded Pylon fleet watch'
gc(){ CLOUDSDK_CORE_HTTP_TIMEOUT=30 gcloud --project="$watch_project" --quiet "$@"; }
refuse(){ echo "$*" >&2;exit 1; }
window(){
 [[ $1 =~ ^[0-9]{1,10}$ && $2 =~ ^[0-9]{1,10}$ ]] || refuse 'START and END must be Unix seconds'
 watch_start=$((10#$1));watch_end=$((10#$2))
 ((watch_start>0 && watch_end-watch_start==21600 && watch_end>$(date +%s))) || refuse 'window must be exactly six hours and not expired'
}
provider(){ [[ $1 =~ ^[0-9a-f]{64}$ ]] || refuse 'GPT-OSS provider must be a lowercase 64-character hex public key'; }
owned_job(){
 local watch_existing
 if watch_existing=$(gc run jobs describe "$watch_job" --region="$watch_region" --format=json 2>/dev/null);then
  jq -e --arg account "$watch_runtime" '.metadata.labels["openagents-component"]=="pylon-fleet-watch" and .spec.template.spec.template.spec.serviceAccountName==$account' <<<"$watch_existing" >/dev/null || refuse 'refusing a job not owned by this fleet watcher'
 fi
}
owned_scheduler(){
 local watch_description
 if watch_description=$(gc scheduler jobs describe "$watch_job" --location="$watch_region" --format='value(description)' 2>/dev/null);then
  [[ $watch_description == "$watch_marker" ]] || refuse 'refusing a Scheduler not owned by this fleet watcher'
  return 0
 fi
 return 1
}
ensure_sa(){
 local watch_email=$1 watch_id=$2 watch_description
 if watch_description=$(gc iam service-accounts describe "$watch_email" --format='value(description)' 2>/dev/null);then
  [[ $watch_description == "$watch_marker" ]] || refuse 'refusing an unrelated service account'
 else gc iam service-accounts create "$watch_id" --display-name="$watch_marker" --description="$watch_marker";fi
}
ensure_role(){
 local watch_role=$1 watch_file=$2 watch_expected=$3 watch_permissions
 if watch_permissions=$(gc iam roles describe "$watch_role" --format='value(includedPermissions)' 2>/dev/null);then
  [[ $watch_permissions == "$watch_expected" ]] || refuse 'refusing a custom role with different permissions'
 else gc iam roles create "$watch_role" --file="$watch_file";fi
}
active_executions(){
 gc run jobs executions list --job="$watch_job" --region="$watch_region" --format=json |
  jq -r '.[] | select(.status.completionTime==null) | .metadata.name'
}
pause_and_drain(){
 owned_job
 owned_scheduler || refuse 'fleet watcher Scheduler does not exist'
 gc scheduler jobs pause "$watch_job" --location="$watch_region"
 gc run jobs update "$watch_job" --region="$watch_region" --update-env-vars=PYLON_WATCH_DRY_RUN=1
 local watch_active watch_execution watch_quiet=0 watch_poll
 for ((watch_poll=0;watch_poll<45;watch_poll++));do
  watch_active=$(active_executions)
  if [[ -z $watch_active ]];then watch_quiet=$((watch_quiet+1));else
   watch_quiet=0
   for watch_execution in $watch_active;do gc run jobs executions cancel "$watch_execution" --region="$watch_region";done
  fi
  if ((watch_poll>=30 && watch_quiet>=5));then echo 'watcher paused and dry; executions drained (previously accepted recovery requests may still settle)';return;fi
  sleep 1
 done
 refuse 'watcher pause requested but execution drain is incomplete; do not begin maintenance'
}

case ${1:-status} in
 deploy)
  [[ $# -ge 4 && $# -le 5 ]] || refuse 'deploy requires IMAGE START END [GPTOSS_PROVIDER]'
  watch_image=$2
  [[ $watch_image =~ ^us-central1-docker\.pkg\.dev/openagentsgemini/openagents/pylon-fleet-watch@sha256:[0-9a-f]{64}$ ]] || refuse 'deploy requires an immutable watcher image in the fixed project/repository'
  window "$3" "$4"
  watch_provider=${5:-};[[ -z $watch_provider ]] || provider "$watch_provider"
  owned_job
  if owned_scheduler;then pause_and_drain;watch_scheduler_mode=update;else watch_scheduler_mode=create;fi
  ensure_sa "$watch_runtime" pylon-watch-runtime
  ensure_sa "$watch_scheduler" pylon-watch-scheduler
  ensure_role pylonFleetWatchReadOnly "$watch_here/fleet-watch-read-role.yaml" compute.instances.get
  ensure_role pylonFleetWatchStartOnly "$watch_here/fleet-watch-start-role.yaml" compute.instances.start
  ensure_role pylonFleetWatchInvokeOnly "$watch_here/fleet-watch-invoke-role.yaml" run.jobs.run
  for watch_target in psionic-rtx6000-pylon:us-central1-b oct30-att-tdx-clef:us-central1-a;do
   gc compute instances add-iam-policy-binding "${watch_target%:*}" --zone="${watch_target#*:}" --member="serviceAccount:$watch_runtime" --role="projects/$watch_project/roles/pylonFleetWatchReadOnly"
  done
  gc compute instances add-iam-policy-binding oct30-att-tdx-clef --zone=us-central1-a --member="serviceAccount:$watch_runtime" --role="projects/$watch_project/roles/pylonFleetWatchStartOnly"
  gc run jobs add-iam-policy-binding pylon-spot-recovery --region="$watch_region" --member="serviceAccount:$watch_runtime" --role="projects/$watch_project/roles/pylonFleetWatchInvokeOnly"
  for watch_service in oa-mesh-elephant oa-mesh-mammoth;do
   gc run services add-iam-policy-binding "$watch_service" --region="$watch_region" --member="serviceAccount:$watch_runtime" --role=roles/run.invoker
  done
  gc run jobs deploy "$watch_job" --image="$watch_image" --region="$watch_region" --service-account="$watch_runtime" --tasks=1 --parallelism=1 --max-retries=0 --task-timeout=240s --cpu=1 --memory=512Mi --set-env-vars="PYLON_WATCH_START=$watch_start,PYLON_WATCH_END=$watch_end,PYLON_WATCH_DRY_RUN=1,PYLON_WATCH_GPTOSS_PROVIDER=$watch_provider" --labels=openagents-component=pylon-fleet-watch
  gc run jobs add-iam-policy-binding "$watch_job" --region="$watch_region" --member="serviceAccount:$watch_scheduler" --role="projects/$watch_project/roles/pylonFleetWatchInvokeOnly"
  gc scheduler jobs "$watch_scheduler_mode" http "$watch_job" --location="$watch_region" --description="$watch_marker" --schedule='*/5 * * * *' --time-zone=Etc/UTC --uri="https://run.googleapis.com/v2/projects/$watch_project/locations/$watch_region/jobs/$watch_job:run" --http-method=POST --oauth-service-account-email="$watch_scheduler" --oauth-token-scope=https://www.googleapis.com/auth/cloud-platform --headers=Content-Type=application/json --message-body='{}' --attempt-deadline=30s --max-retry-attempts=0 --max-retry-duration=0s
  gc scheduler jobs pause "$watch_job" --location="$watch_region"
  echo 'deployed PAUSED and dry; verify the actual Scheduler path before activation'
  ;;
 verify)
  owned_job
  owned_scheduler || refuse 'fleet watcher Scheduler does not exist'
  watch_dry=$(gc run jobs describe "$watch_job" --region="$watch_region" --format=json | jq -er '.spec.template.spec.template.spec.containers[0].env[] | select(.name=="PYLON_WATCH_DRY_RUN") | .value')
  [[ $watch_dry == 1 ]] || refuse 'verify refuses active mode; pause first'
  watch_state=$(gc scheduler jobs describe "$watch_job" --location="$watch_region" --format='value(state)')
  watch_receipt=${PYLON_WATCH_RECEIPT:-/tmp/pylon-fleet-watch-scheduler.json}
  watch_work=$(mktemp -d)
  cleanup_verify(){
   watch_exit=$?
   trap - EXIT
   if [[ $watch_state == PAUSED ]];then gc scheduler jobs pause "$watch_job" --location="$watch_region" || watch_exit=1;fi
   rm -rf "$watch_work"
   exit "$watch_exit"
  }
  trap cleanup_verify EXIT
  gc run jobs executions list --job="$watch_job" --region="$watch_region" --format=json | jq '[.[].metadata.name]' > "$watch_work/before.json"
  if [[ $watch_state == PAUSED ]];then gc scheduler jobs resume "$watch_job" --location="$watch_region";fi
  gc scheduler jobs run "$watch_job" --location="$watch_region"
  watch_deadline=$(($(date +%s)+300));watch_execution=''
  while (($(date +%s)<watch_deadline));do
   if [[ -z $watch_execution ]];then
    watch_execution=$(gc run jobs executions list --job="$watch_job" --region="$watch_region" --format=json | jq -r --slurpfile old "$watch_work/before.json" '[.[].metadata.name | select(. as $name | $old[0] | index($name) | not)] | first // empty')
   fi
   if [[ -n $watch_execution ]];then
    gc run jobs executions describe "$watch_execution" --region="$watch_region" --format=json > "$watch_work/execution.json"
    if jq -e '.status.completionTime!=null' "$watch_work/execution.json" >/dev/null;then
     jq -e --arg creator "$watch_scheduler" '.metadata.annotations["run.googleapis.com/creator"]==$creator and (.status.succeededCount|tonumber)==1' "$watch_work/execution.json" >/dev/null || refuse 'Scheduler execution failed or has the wrong creator'
     cp "$watch_work/execution.json" "$watch_receipt"
     echo "verified Scheduler-created execution $watch_execution; receipt $watch_receipt"
     exit 0
    fi
   fi
   sleep 2
  done
  refuse 'Scheduler verification timed out; no successful execution claimed'
  ;;
 activate)
  [[ $# == 4 ]] || refuse 'activate requires START END GPTOSS_PROVIDER'
  window "$2" "$3";provider "$4"
  owned_job
  owned_scheduler || refuse 'fleet watcher Scheduler does not exist'
  gc run jobs update "$watch_job" --region="$watch_region" --update-env-vars="PYLON_WATCH_START=$watch_start,PYLON_WATCH_END=$watch_end,PYLON_WATCH_GPTOSS_PROVIDER=$4,PYLON_WATCH_DRY_RUN=0"
  gc scheduler jobs resume "$watch_job" --location="$watch_region"
  echo 'watcher enabled for the explicit six-hour window; no existing controller settings changed'
  ;;
 pause) pause_and_drain ;;
 status)
  gc scheduler jobs describe "$watch_job" --location="$watch_region" --format='json(name,state,schedule,lastAttemptTime,status)'
  gc run jobs describe "$watch_job" --region="$watch_region" --format='json(metadata.labels,spec.template.spec.template.spec.containers,spec.template.spec.template.spec.serviceAccountName)'
  gc run jobs executions list --job="$watch_job" --region="$watch_region" --limit=5 --format='table(metadata.name,status.completionTime,status.succeededCount,status.failedCount)'
  ;;
 *) refuse 'usage: pylon-fleet-watch.sh deploy IMAGE START END [GPTOSS_PROVIDER] | verify | activate START END GPTOSS_PROVIDER | pause | status' ;;
esac
