#!/usr/bin/env bash
# HTTP/filesystem effects only. The compiled Coax procedure chooses the action.
set -euo pipefail
umask 077
recovery_project=openagentsgemini recovery_zone=us-central1-b recovery_vm=psionic-rtx6000-pylon
recovery_dry=${PYLON_RECOVERY_DRY_RUN:-1}
recovery_decider=${PYLON_RECOVERY_DECIDER:-/app/spot-recovery}
recovery_work=$(mktemp -d);trap 'rm -rf "$recovery_work"' EXIT
recovery_url=https://compute.googleapis.com/compute/v1/projects/openagentsgemini/zones/us-central1-b/instances/psionic-rtx6000-pylon
if ! curl --noproxy '*' --fail --silent --show-error --connect-timeout 3 --max-time 8 --header 'Metadata-Flavor: Google' http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token >"$recovery_work/token";then
  printf '{"severity":"ERROR","action":"token_failed"}\n';exit 1
fi
recovery_token=$(jq -er .access_token "$recovery_work/token")
printf 'Authorization: Bearer %s\n' "$recovery_token" >"$recovery_work/auth";unset recovery_token
if ! curl --noproxy '*' --proto '=https' --fail --silent --show-error --connect-timeout 5 --max-time 15 --header "@$recovery_work/auth" "$recovery_url" >"$recovery_work/observation";then
  printf '{"severity":"ERROR","action":"inspect_failed","vm":"psionic-rtx6000-pylon"}\n';exit 1
fi
recovery_status=$(jq -er .status "$recovery_work/observation")
recovery_action=$("$recovery_decider" "$recovery_project" "$recovery_zone" "$recovery_vm" "$recovery_status" "$recovery_dry")
case "$recovery_action" in
  0) printf '{"severity":"INFO","action":"noop","status":"RUNNING","vm":"psionic-rtx6000-pylon"}\n' ;;
  1) printf '{"severity":"INFO","action":"wait","vm":"psionic-rtx6000-pylon"}\n' ;;
  2) printf '{"severity":"INFO","action":"would_start","status":"TERMINATED","vm":"psionic-rtx6000-pylon"}\n' ;;
  3)
    if ! curl --noproxy '*' --proto '=https' --fail --silent --show-error --connect-timeout 5 --max-time 15 --request POST --header "@$recovery_work/auth" "$recovery_url/start" >"$recovery_work/operation";then
      printf '{"severity":"ERROR","action":"start_failed","vm":"psionic-rtx6000-pylon"}\n';exit 1
    fi
    if jq -e .error "$recovery_work/operation" >/dev/null;then printf '{"severity":"ERROR","action":"start_failed","vm":"psionic-rtx6000-pylon"}\n';cat "$recovery_work/operation" >&2;exit 1;fi
    printf '{"severity":"NOTICE","action":"start_requested","vm":"psionic-rtx6000-pylon","application_health":"not_checked"}\n';cat "$recovery_work/operation"
    ;;
  *) printf '{"severity":"ERROR","action":"refused","vm":"psionic-rtx6000-pylon"}\n';exit 1 ;;
esac
