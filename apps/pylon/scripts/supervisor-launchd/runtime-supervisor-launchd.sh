#!/usr/bin/env bash
#
# runtime-supervisor-launchd.sh (#8388 follow-up)
#
# launchd entrypoint for the Khala Sync `runtime.*` control-intent dispatch
# consumer (`apps/pylon/src/orchestration/runtime-intent-supervisor.ts`). Runs
# it IN THE FOREGROUND (exec) so launchd's KeepAlive restarts it if it ever
# dies or exits — mirroring `codex-supervisor-launchd.sh` /
# `claude-supervisor-launchd.sh`. Sources the owner-linked agent token and the
# admin API token from LOCAL SECRETS (never embedded in the plist or printed).
#
# Unlike the codex/claude supervisors this process does NOT fire `khala
# request` — it only polls durable `runtime.*` intents and dispatches real
# local Codex turns for the owner Pylon's OWN linked user, so there is no
# `SUP_PYLON_REF`/live-ref resolution step here (the intent poller is scoped
# by owner user id, not by pylon ref).
#
# Installed via install.sh; driven by com.openagents.runtime-supervisor.plist.
set -uo pipefail

REPO_ROOT="${SUP_REPO_ROOT:-__REPO_ROOT__}"

# Owner-linked (Artanis) agent token — this process pushes `runtime.recordEvent`
# and Pylon-authored follow-up `runtime.startTurn` mutations into the owner's
# own scope, so it must be owner-linked (never the fable token). Read from
# .secrets; the file exports OPENAGENTS_AGENT_TOKEN. Tokens are never echoed.
SUP_SECRET_ENV="${SUP_SECRET_ENV:-$HOME/work/.secrets/openagents-artanis-agent.env}"
if [ -f "$SUP_SECRET_ENV" ]; then
  set -a; . "$SUP_SECRET_ENV"; set +a
fi

# Admin bearer for the admin-guarded internal poll routes
# (`/api/internal/khala-sync/runtime-intents`, `.../chat-message`). Production
# admin token lives in `.secrets/vortex-admin.env` (exports
# OPENAGENTS_ADMIN_API_TOKEN) — the same file other Pylon/ops tooling in this
# workspace already reads for the prod admin bearer. Never echoed.
SUP_ADMIN_SECRET_ENV="${SUP_ADMIN_SECRET_ENV:-$HOME/work/.secrets/vortex-admin.env}"
if [ -f "$SUP_ADMIN_SECRET_ENV" ]; then
  set -a; . "$SUP_ADMIN_SECRET_ENV"; set +a
fi

# This dispatch consumer uses the standing-pylon home by default (registered
# owner home with the linked Codex accounts) — the SAME home the standing
# pylon and codex supervisor already use. Never a fresh/new pylon home.
export PYLON_HOME="${PYLON_HOME:-$HOME/.pylon-fable}"
export PYLON_DISABLE_DAEMON_ROUTING="${PYLON_DISABLE_DAEMON_ROUTING:-1}"
export OPENAGENTS_BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents.com}"

# Scope the poll to this Pylon's linked owner user — REQUIRED for safety: this
# process only has ONE owner's local Codex account registry, so polling every
# owner's runtime intents would try to dispatch other users' turns against
# this owner's own Codex credentials. Override via env/secret if the linked
# owner ever changes; do not leave this unset in a multi-owner deployment.
export OPENAGENTS_RUNTIME_OWNER_USER_ID="${OPENAGENTS_RUNTIME_OWNER_USER_ID:-user_ccf97bf1-ad33-4c55-b9c7-41eeeb9e0c93}"

cd "$REPO_ROOT" || exit 1
exec bun "$REPO_ROOT/apps/pylon/src/orchestration/runtime-intent-supervisor.ts" \
  --pylon-home "$PYLON_HOME"
