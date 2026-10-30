#!/usr/bin/env bash
# A rollout is successful only when its authenticated Cloud Run snapshots and
# the uncached routed health response all identify the same immutable revision.
# Caller supplies a private temporary directory and an immutable build digest.
api_verified_rollout() { # service host image commit release tempdir [gcloud deploy flags...]
  local service="$1" host="$2" image="$3" commit="$4" release="$5" work="$6"
  shift 6
  case "$service:$host" in
    oa-api-staging:staging.openagents.com|oa-api:openagents.com) ;;
    *) echo 'api: unexpected service/host pairing' >&2; return 1 ;;
  esac
  local guard="$here/deploy_verify.py" revision status attempt
  gcloud run services describe "$service" --project "$project" --region "$region" --format=json > "$work/pre.json" || return 1
  api_run_deploy gcloud run deploy "$service" --image "$image" --project "$project" --region "$region" "$@" --quiet || return 1
  for attempt in $(seq 1 20); do
    gcloud run services describe "$service" --project "$project" --region "$region" --format=json > "$work/before.json" || return 1
    if revision="$(python3 "$guard" ready-revision "$work/before.json" "$service" "$image" 2>/dev/null)"; then break; fi
    [[ "$attempt" != 20 ]] || { echo 'api: new digest revision never became ready with exclusive traffic' >&2; return 1; }
    sleep 3
  done
  gcloud run revisions describe "$revision" --project "$project" --region "$region" --format=json > "$work/revision.json" || return 1
  status="$(curl -sS --proto '=https' --connect-timeout 5 --max-time 20 --max-filesize 1048576 \
    -H 'Cache-Control: no-cache, no-store' -H 'Pragma: no-cache' \
    -o "$work/health.json" -w '%{http_code}' "https://$host/api/playtest/health")" || return 1
  gcloud run services describe "$service" --project "$project" --region "$region" --format=json > "$work/after.json" || return 1
  python3 "$guard" --pre "$work/pre.json" --before "$work/before.json" --revision "$work/revision.json" \
    --after "$work/after.json" --health "$work/health.json" --service "$service" --image "$image" \
    --commit "$commit" --release "$release" --deploy-exit 0 --http-status "$status" || return 1
  printf '  ok    %s %s %s\n' "$host" "$revision" "$image"
}
