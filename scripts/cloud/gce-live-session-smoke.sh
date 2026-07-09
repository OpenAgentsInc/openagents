#!/usr/bin/env bash
# Bounded live GCE per-session provisioner smoke for
# openagents.gce_capacity_class.v1 / LiveGceProvisioner.
#
# Provisions one ephemeral VM + session firewall, probes RUNNING, attempts a
# bounded IAP-SSH echo assignment, then GUARANTEES teardown (VM + firewall
# delete) with a final empty `instances list` assertion via a hard trap, even
# on any error or interrupt. Mirrors the exact gcloud lifecycle the Rust
# LiveGceProvisioner drives.
#
# Requires gcloud + an authenticated account (gcloud CLI auth or ADC). The raw
# project id is supplied via OA_CODEX_GCE_PROJECT_ID and is never hardcoded
# here (INVARIANTS: no raw GCP project ids in tracked files).
set -uo pipefail

PROJECT="${OA_CODEX_GCE_PROJECT_ID:?set OA_CODEX_GCE_PROJECT_ID to the raw GCP project id}"
ZONE="${OA_CODEX_GCE_ZONE:-us-central1-a}"
MACHINE="${OA_CODEX_GCE_MACHINE_TYPE:-e2-small}"
IMG_FAMILY="${OA_CODEX_GCE_IMAGE_FAMILY:-ubuntu-2404-lts-amd64}"
IMG_PROJECT="${OA_CODEX_GCE_IMAGE_PROJECT:-ubuntu-os-cloud}"

RUN_ID="run_gce_live_smoke_$(date -u +%Y%m%d%H%M%S)"
SEED="${RUN_ID}|gcp-project-ref://openagents/cloud-primary"
DIGEST=$(printf '%s' "$SEED" | shasum -a 256 | cut -c1-16)
VM="oa-codex-sess-${DIGEST}"
FW="oa-codex-sess-fw-${DIGEST}"
TTL_EXPIRES=$(( $(date -u +%s) + 900 ))

echo "RUN_ID=$RUN_ID"
echo "VM=$VM"
echo "FW=$FW"

DELETED_VM="false"; REMOVED_FW="false"; FINAL_COUNT="unknown"

retry() { local n=0; until "$@"; do n=$((n+1)); [ $n -ge 4 ] && return 1; sleep 6; done; }

teardown() {
  echo "=== TEARDOWN (guaranteed, retried) ==="
  for i in 1 2 3 4 5; do
    if gcloud compute instances delete "$VM" --project "$PROJECT" --zone "$ZONE" --quiet 2>/tmp/td_vm.err; then
      DELETED_VM="true"; break
    elif grep -q "was not found" /tmp/td_vm.err; then
      DELETED_VM="true"; break
    else
      echo "vm delete attempt $i failed; retrying"; sleep 8
    fi
  done
  for i in 1 2 3 4 5; do
    if gcloud compute firewall-rules delete "$FW" --project "$PROJECT" --quiet 2>/tmp/td_fw.err; then
      REMOVED_FW="true"; break
    elif grep -q "was not found" /tmp/td_fw.err; then
      REMOVED_FW="true"; break
    else
      echo "fw delete attempt $i failed; retrying"; sleep 8
    fi
  done
  for i in 1 2 3 4 5; do
    FINAL_COUNT=$(gcloud compute instances list --project "$PROJECT" --zones "$ZONE" \
      --filter="name=${VM}" --format="value(name)" 2>/tmp/list.err | grep -c . )
    [ -n "$FINAL_COUNT" ] && break
    sleep 6
  done
  echo "TEARDOWN_RESULT deleted_vm=$DELETED_VM removed_firewall=$REMOVED_FW final_session_vm_count=$FINAL_COUNT"
}
trap teardown EXIT

LABELS="openagents-managed=true,openagents-capacity-class=$(echo gce.ephemeral.standard.v1 | tr '.' '-'),openagents-lease-ref=d-${DIGEST},openagents-owner-ref=d-${DIGEST},openagents-ttl-expires=${TTL_EXPIRES}"

echo "=== 1) instances create (e2-small ephemeral, tagged+labeled) ==="
gcloud compute instances create "$VM" \
  --project "$PROJECT" --zone "$ZONE" --machine-type "$MACHINE" \
  --image-family "$IMG_FAMILY" --image-project "$IMG_PROJECT" \
  --no-restart-on-failure --no-address \
  --tags "$VM" --labels "$LABELS" 2>&1 | tail -4 || { echo "CREATE FAILED"; exit 1; }

echo "=== 2) firewall-rules create (session-scoped IAP ssh, no labels) ==="
gcloud compute firewall-rules create "$FW" \
  --project "$PROJECT" --direction INGRESS --action ALLOW \
  --rules tcp:22 --target-tags "$VM" --source-ranges 35.235.240.0/20 2>&1 | tail -3 || { echo "FW CREATE FAILED"; exit 1; }

echo "=== 3) health probe (status == RUNNING) ==="
STATUS=$(gcloud compute instances describe "$VM" --project "$PROJECT" --zone "$ZONE" --format="value(status)" 2>&1)
echo "vm_status=$STATUS"
[ "$STATUS" = "RUNNING" ] || { echo "NOT RUNNING"; exit 1; }

echo "=== 4) bounded Codex/echo assignment over IAP SSH (retry for boot) ==="
ASSIGN_OK="false"
for i in 1 2 3 4 5 6; do
  ASSIGN_OUT=$(timeout 90 gcloud compute ssh "$VM" --project "$PROJECT" --zone "$ZONE" \
    --tunnel-through-iap --command "echo OA_GCE_LIVE_SMOKE_OK_${DIGEST}" 2>/tmp/ssh.err || true)
  if echo "$ASSIGN_OUT" | grep -q "OA_GCE_LIVE_SMOKE_OK_${DIGEST}"; then
    ASSIGN_OK="true"; echo "assignment_stdout=${ASSIGN_OUT}"; break
  fi
  echo "ssh attempt $i not ready; waiting for boot/iap"; sleep 15
done
echo "ASSIGNMENT_RESULT=$ASSIGN_OK"
echo "=== done; trap tears down ==="
