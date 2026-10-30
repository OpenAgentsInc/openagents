#!/usr/bin/env bash
# Bounded infrastructure effects. Coax admits every window, target and answer.
set -euo pipefail
umask 077
watch_project=openagentsgemini
watch_start=${PYLON_WATCH_START:?missing watch start}
watch_end=${PYLON_WATCH_END:?missing watch end}
watch_dry=${PYLON_WATCH_DRY_RUN:-1}
watch_policy=${PYLON_WATCH_POLICY:-/app/fleet-watch}
watch_probe=${PYLON_WATCH_PROBE:-/app/fleet-watch-probe}
watch_work=$(mktemp -d);trap 'rm -rf "$watch_work"' EXIT
watch_now=$(date +%s)
watch_window=$($watch_policy window "$watch_now" "$watch_start" "$watch_end")
if [[ $watch_window == 1 || $watch_window == 2 ]];then
 jq -nc --argjson now "$watch_now" --argjson window "$watch_window" '{severity:"INFO",action:"outside_window",now:$now,window:$window}';exit 0
fi
[[ $watch_window == 0 ]] || { echo '{"severity":"ERROR","action":"invalid_window"}';exit 1; }
meta(){ curl --noproxy '*' --fail --silent --show-error --connect-timeout 3 --max-time 8 --header 'Metadata-Flavor: Google' "$1"; }
meta http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token > "$watch_work/token.json"
watch_token=$(jq -er .access_token "$watch_work/token.json")
printf 'Authorization: Bearer %s\n' "$watch_token" > "$watch_work/access.header";unset watch_token
api(){ curl --noproxy '*' --proto '=https' --fail --silent --show-error --connect-timeout 5 --max-time 20 --header "@$watch_work/access.header" "$@"; }
observe_vm(){
 local zone=$1 vm=$2 file="$watch_work/$2.json" status action url
 url="https://compute.googleapis.com/compute/v1/projects/$watch_project/zones/$zone/instances/$vm"
 api "$url" > "$file" || return 1
 status=$(jq -er .status "$file")
 action=$($watch_policy decision "$watch_project" "$zone" "$vm" "$status" "$(date +%s)" "$watch_start" "$watch_end" "$watch_dry")
 case "$action" in
  0|1|2) jq -nc --arg vm "$vm" --arg status "$status" --argjson action "$action" '{severity:"INFO",kind:"vm",vm:$vm,status:$status,action:$action}' ;;
  3) api --request POST "$url/start" > "$watch_work/start.json" || return 1
     jq -e 'has("error")|not' "$watch_work/start.json" >/dev/null || return 1
     jq -nc --arg vm "$vm" '{severity:"NOTICE",kind:"vm",vm:$vm,action:"start_requested",model_ready:false}' ;;
  4) api --request POST --header 'Content-Type: application/json' --data '{}' 'https://run.googleapis.com/v2/projects/openagentsgemini/locations/us-central1/jobs/pylon-spot-recovery:run' > "$watch_work/recovery.json" || return 1
     jq -nc --arg vm "$vm" '{severity:"NOTICE",kind:"vm",vm:$vm,action:"existing_recovery_dispatched",model_ready:false}' ;;
  *) jq -nc --arg vm "$vm" --argjson action "$action" '{severity:"ERROR",kind:"vm",vm:$vm,action:$action}';return 1 ;;
 esac
}
decision_probe(){
 local model=$1 origin=$2 identity=$3 provider=$4 slug=$5 nonce file="$watch_work/$1"
 nonce="$(date +%s)-${CLOUD_RUN_EXECUTION:-probe}"
 jq -nc --arg model "$model" --arg nonce "$nonce" '{model:$model,state:("Water is wet. Public monitoring probe "+$nonce+"."),questions:{wet:{type:"choice",instructions:"Is the water wet?",criteria:{yes:null,no:null}}}}' > "$file.request"
 meta "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity?audience=$origin&format=full" > "$file.token"
 printf 'Authorization: Bearer %s\n' "$(cat "$file.token")" > "$file.header"
 curl --noproxy '*' --proto '=https' --fail --silent --show-error --connect-timeout 5 --max-time 40 --request POST --header "@$file.header" --header 'Content-Type: application/json' --data-binary "@$file.request" "$origin/v1/systemone" > "$file.response" || return 1
 "$watch_probe" "$file.request" "$file.response" "$identity" "$provider" "$slug" || { jq -nc --arg model "$model" '{severity:"ERROR",kind:"inference",ok:false,model:$model,reason:"answer_or_binding_refused"}';return 1; }
 jq -c --arg model "$model" --argjson checked_at "$(date +%s)" '{severity:"INFO",kind:"inference",ok:true,model:$model,checked_at:$checked_at,service:.service,answers:.answers,latency_ms:.latency_ms}' "$file.response"
}
confidential_probe(){
 "${PYLON_WATCH_CONFIDENTIAL_PROBE:-/opt/oa-confidential/probe-confidential-pylon.sh}" --evidence-dir "$watch_work/confidential" --timeout-seconds 120
}
text_probe(){
 local client=${PYLON_WATCH_TEXT_PROBE:-/app/probe-gptoss.sh}
 [[ -x $client && -n ${PYLON_WATCH_GPTOSS_PROVIDER:-} ]] || { echo '{"severity":"ERROR","kind":"gptoss","ok":false,"reason":"probe_or_provider_pending"}';return 1; }
 # The native pinned client interface is finalized with the text-lane owner.
 "$client" "$PYLON_WATCH_GPTOSS_PROVIDER" "$watch_work/gptoss"
}
watch_failed=0
observe_vm us-central1-b psionic-rtx6000-pylon || watch_failed=1
observe_vm us-central1-a oct30-att-tdx-clef || watch_failed=1
decision_probe elephant-1 https://oa-mesh-elephant-ezxz4mgdsq-uc.a.run.app elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba 59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7 oct30-gcloud-elephant-rtx > "$watch_work/elephant.log" 2>&1 & watch_elephant=$!
decision_probe mammoth-1 https://oa-mesh-mammoth-ezxz4mgdsq-uc.a.run.app mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019 d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829 oct30-gcloud-mammoth > "$watch_work/mammoth.log" 2>&1 & watch_mammoth=$!
confidential_probe > "$watch_work/confidential.log" 2>&1 & watch_confidential=$!
text_probe > "$watch_work/gptoss.log" 2>&1 & watch_gptoss=$!
for watch_pid in "$watch_elephant" "$watch_mammoth" "$watch_confidential" "$watch_gptoss";do wait "$watch_pid" || watch_failed=1;done
cat "$watch_work/elephant.log" "$watch_work/mammoth.log" "$watch_work/confidential.log" "$watch_work/gptoss.log"
jq -nc --argjson checked_at "$(date +%s)" --argjson failed "$watch_failed" '{severity:(if $failed==0 then "INFO" else "ERROR" end),kind:"cycle",ok:($failed==0),checked_at:$checked_at,required_roles:4}'
exit "$watch_failed"
