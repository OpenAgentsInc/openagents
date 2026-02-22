#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"

fail() {
  echo "error: $*" >&2
  exit 1
}

if [[ -f "${ROOT_DIR}/apps/openagents.com/app/Console/Kernel.php" ]]; then
  fail "Laravel scheduler kernel still exists at apps/openagents.com/app/Console/Kernel.php"
fi

if rg -n "Schedule::|->command\\(|->job\\(|schedule\\(" \
  "${ROOT_DIR}/apps/openagents.com/app" \
  "${ROOT_DIR}/apps/openagents.com/routes" \
  --glob '*.php' >/dev/null 2>&1; then
  fail "Unexpected Laravel scheduler definitions detected in active legacy PHP lane."
fi

should_queue_count="$(
  rg -n "implements\\s+ShouldQueue" \
    "${ROOT_DIR}/apps/openagents.com/app" \
    --glob '*.php' 2>/dev/null | wc -l | tr -d ' '
)"
if [[ "${should_queue_count}" -gt 1 ]]; then
  fail "Unexpected queued job count in legacy PHP lane: ${should_queue_count} (expected <= 1)."
fi
if [[ "${should_queue_count}" -eq 1 ]]; then
  first_hit="$(
    rg -n "implements\\s+ShouldQueue" \
      "${ROOT_DIR}/apps/openagents.com/app" \
      --glob '*.php' | head -n 1
  )"
  if [[ "${first_hit}" != *"ForwardResendWebhookToRuntime.php"* ]]; then
    fail "Unexpected queued job implementation found: ${first_hit}"
  fi
fi

if rg -n "queue:work|schedule:run|openagents-queue|openagents-scheduler" \
  "${ROOT_DIR}/apps/openagents.com" \
  "${ROOT_DIR}/docs" \
  "${ROOT_DIR}/scripts" \
  "${ROOT_DIR}/.github" \
  --glob '!apps/openagents.com/docs/archived/**' \
  --glob '!apps/openagents.com/app/**' \
  --glob '!apps/openagents.com/tests/**' \
  --glob '!apps/openagents.com/service/scripts/verify-laravel-async-lanes-disabled.sh' \
  --glob '!apps/openagents.com/service/scripts/disable-legacy-laravel-async-jobs.sh' \
  --glob '!apps/openagents.com/service/docs/LARAVEL_ASYNC_RETIREMENT.md' \
  --glob '!apps/openagents.com/docs/20260222-oa-webparity-066-queue-scheduler-listener-parity.md' >/dev/null 2>&1; then
  fail "Active docs/scripts still reference Laravel queue/scheduler runtime commands."
fi

for pattern in \
  "spawn_resend_webhook_forward_task" \
  "forward_resend_webhook_to_runtime" \
  "mark_webhook_event_forwarding" \
  "mark_webhook_event_retrying" \
  "mark_webhook_event_forwarded"; do
  if ! rg -n "${pattern}" "${ROOT_DIR}/apps/openagents.com/service/src/lib.rs" >/dev/null 2>&1; then
    fail "Rust async webhook path missing required symbol: ${pattern}"
  fi
done

echo "verify-laravel-async-lanes-disabled: pass"
