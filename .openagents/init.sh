#!/bin/bash
# .openagents/init.sh - Preflight checklist for Golden Loop v2
#
# Exit codes:
#   0 = All checks passed
#   1 = Fatal error (abort session)
#   2 = Warnings only (continue with caution)
#
# Logged to: docs/logs/YYYYMMDD/HHMM-preflight.log
#
# See docs/mechacoder/GOLDEN-LOOP-v2.md Section 2.2.1 for full documentation.

set -o pipefail

DAY=$(TZ=America/Chicago date +%Y%m%d)
TS=$(TZ=America/Chicago date +%H%M)
LOG_DIR="docs/logs/$DAY"
LOG_FILE="$LOG_DIR/${TS}-preflight.log"

mkdir -p "$LOG_DIR"

log() {
    echo "[$(date -Iseconds)] $1" | tee -a "$LOG_FILE"
}

warn() {
    log "WARNING: $1"
    WARNINGS=$((WARNINGS + 1))
}

fatal() {
    log "FATAL: $1"
    exit 1
}

WARNINGS=0

log "=== Golden Loop v2 Preflight Checklist ==="
log "Working directory: $(pwd)"
log "Project: $(jq -r '.projectId // "unknown"' .openagents/project.json 2>/dev/null || echo 'unknown')"

# 1. Check git status (uncommitted changes)
log "Checking git status..."
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    warn "Uncommitted changes detected:"
    git status --short >> "$LOG_FILE"
    # Policy: continue with warning (change to 'fatal' for strict mode)
fi

# 2. Smoke test: typecheck
log "Running smoke test (typecheck)..."
if ! bun run typecheck >> "$LOG_FILE" 2>&1; then
    fatal "Typecheck failed at preflight. Fix errors before running agent."
fi
log "Typecheck passed."

# 3. Smoke test: quick test run (optional, based on project config)
SMOKE_TEST=$(jq -r '.smokeTestCommand // empty' .openagents/project.json 2>/dev/null)
if [ -n "$SMOKE_TEST" ]; then
    log "Running smoke test: $SMOKE_TEST"
    if ! eval "$SMOKE_TEST" >> "$LOG_FILE" 2>&1; then
        fatal "Smoke test failed: $SMOKE_TEST"
    fi
    log "Smoke test passed."
else
    log "No smokeTestCommand configured, skipping test smoke."
fi

# 4. Check Claude Code CLI is available (if Claude Code enabled)
CLAUDE_ENABLED=$(jq -r '.claudeCode.enabled // false' .openagents/project.json 2>/dev/null)
if [ "$CLAUDE_ENABLED" = "true" ]; then
    log "Checking Claude Code CLI..."
    if ! command -v claude >/dev/null 2>&1; then
        fatal "Claude Code enabled but 'claude' CLI not found. Install it: npm install -g @anthropic-ai/claude-code"
    fi
    # Claude CLI handles its own OAuth auth - no need to check credentials here
    log "Claude Code CLI available."
fi

# 5. Check network connectivity (if not offline mode)
OFFLINE_MODE=$(jq -r '.offlineMode // "block"' .openagents/project.json 2>/dev/null)
log "Checking network connectivity..."
if ! curl -s --connect-timeout 5 https://api.github.com >/dev/null 2>&1; then
    if [ "$OFFLINE_MODE" = "allow" ]; then
        warn "Network unreachable. Continuing in offline mode."
    else
        fatal "Network unreachable and offlineMode is not 'allow'. Aborting."
    fi
else
    log "Network connectivity confirmed."
fi

# 6. Check disk space (warn if < 1GB free)
log "Checking disk space..."
FREE_KB=$(df -k . | tail -1 | awk '{print $4}')
if [ "$FREE_KB" -lt 1048576 ]; then
    warn "Low disk space: $(($FREE_KB / 1024))MB free"
fi

# 7. Sync with remote (optional, fetch only)
ALLOW_PUSH=$(jq -r '.allowPush // false' .openagents/project.json 2>/dev/null)
if [ "$ALLOW_PUSH" = "true" ]; then
    log "Fetching from remote..."
    if git fetch origin >> "$LOG_FILE" 2>&1; then
        BEHIND=$(git rev-list --count HEAD..origin/$(git rev-parse --abbrev-ref HEAD) 2>/dev/null || echo 0)
        if [ "$BEHIND" -gt 0 ]; then
            warn "Local branch is $BEHIND commits behind remote."
        fi
    else
        warn "git fetch failed (continuing with local state)"
    fi
fi

# Summary
log "=== Preflight Complete ==="
if [ "$WARNINGS" -gt 0 ]; then
    log "Completed with $WARNINGS warning(s). Review $LOG_FILE for details."
    exit 2
else
    log "All checks passed. Ready for Golden Loop."
    exit 0
fi
