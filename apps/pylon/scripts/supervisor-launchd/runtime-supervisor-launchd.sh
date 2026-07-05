#!/usr/bin/env bash
#
# runtime-supervisor-launchd.sh (#8388 follow-up)
#
# launchd entrypoint for the Khala Sync `runtime.*` control-intent dispatch
# consumer (`apps/pylon/src/orchestration/runtime-intent-supervisor.ts`). Runs
# it IN THE FOREGROUND (exec) so launchd's KeepAlive restarts it if it ever
# dies or exits — mirroring `codex-supervisor-launchd.sh` /
# `claude-supervisor-launchd.sh`. Sources this Pylon's own registered agent
# credential (resolves to the linked owner's real user id — see below) and
# the admin API token from LOCAL SECRETS (never embedded in the plist or
# printed).
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

# launchd's GUI-domain agents get a minimal PATH (no bun, no Homebrew) — add
# the common install locations so `bun` resolves without needing an absolute
# path baked in (verified needed: `caffeinate -i ... .sh` execed with
# `exec: bun: not found` before this was added).
export PATH="$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

# This process pushes `runtime.recordEvent` and Pylon-authored follow-up
# `runtime.startTurn` mutations INTO THE OWNER'S OWN Khala Sync thread scope
# (`scope.thread.<id>`), which is first-writer-wins owned by whatever
# `ctx.userId` the FIRST mutation for that scope resolved to. For a thread the
# owner created from their own mobile/desktop session, that is the owner's
# real personal user id — NOT any separate registered "agent" persona's own
# user id (Artanis, Fable-the-named-bot, etc. each resolve to their OWN
# distinct `user_...` id via `authenticateRequestActor`/`/api/agents/me`,
# confirmed empirically: neither matched a real owned thread and every push
# was rejected with "this runtime thread scope belongs to a different user").
# The one credential that DOES resolve to the linked owner's real user id is
# THIS Pylon's own registered agent credential
# (`$PYLON_HOME/auth/openagents-agent-token`, plain text, no secrets file) —
# it was minted when this Pylon home was originally linked to the owner via
# `pylon auth`/device-link, so its `credential.openauthUserId` matches the
# same identity the owner's own sessions authenticate with. Never echoed.
if [ -f "${PYLON_HOME:-$HOME/.pylon-fable}/auth/openagents-agent-token" ]; then
  OPENAGENTS_AGENT_TOKEN="$(cat "${PYLON_HOME:-$HOME/.pylon-fable}/auth/openagents-agent-token")"
  export OPENAGENTS_AGENT_TOKEN
fi

# Fallback only: an explicit .secrets env override (e.g. a different owner-
# linked token) when the Pylon's own credential file above is unavailable.
SUP_SECRET_ENV="${SUP_SECRET_ENV:-$HOME/work/.secrets/openagents-artanis-agent.env}"
if [ -z "${OPENAGENTS_AGENT_TOKEN:-}" ] && [ -f "$SUP_SECRET_ENV" ]; then
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
# Explicit --pylon-ref (rather than the script's own deterministic
# stableRef(pylonHome) default): the push engine's synthetic clientGroupId
# (`khala-pylon-runtime.<pylonRef>`) is bound to the FIRST user id that
# successfully authenticates against it — sticky/first-write-wins, same
# model as thread scope ownership. An earlier misconfigured run of this
# supervisor (before the owner-linked-token fix above) poisoned the default
# stableRef-derived clientGroupId against the wrong user id, so every
# subsequent runtime.recordEvent push failed 403
# `{"code":"unauthorized_scope","messageSafe":"This client group is bound to
# a different user."}` even with the correct token. Pinning a fresh ref here
# avoids that poisoned default permanently.
exec bun "$REPO_ROOT/apps/pylon/src/orchestration/runtime-intent-supervisor.ts" \
  --pylon-home "$PYLON_HOME" \
  --pylon-ref "${SUP_RUNTIME_PYLON_REF:-pylon.runtime_supervisor.fable.v2}"
