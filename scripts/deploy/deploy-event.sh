# Best-effort deploy announcements for the Grid's build billboard (#12442,
# nips/openagents/NIP-DEPLOY.md). Source this file, then call
#
#   deploy_event ENV PHASE [TIER] [COMMIT] [BUILD]
#
# ENV is staging or production; PHASE, TIER (default image), COMMIT (a SHA,
# whose subject comes from git), and BUILD (a build ID) are as in
# `openagents deploy event --help`. Every phase of one deploy shares
# DEPLOY_EVENT_RUN (set by deploy_event_begin) and DEPLOY_EVENT_STARTED.
#
# A missing `openagents`, a missing announcer key, or a relay that doesn't
# answer never fails or slows the deploy past a few seconds: each call waits
# at most 3 seconds for the relay and its result is ignored.
# DEPLOY_EVENTS=0 turns the announcements off; DEPLOY_EVENT_TEST=1 marks
# them as test events, which the billboard hides unless asked.
# POSIX sh: web.sh runs under /bin/sh and livehost.sh under bash.

# Starts a new run unless one is already set (a caller such as web.sh shares
# its run with the scripts it calls).
deploy_event_begin() {
    if [ -z "${DEPLOY_EVENT_RUN:-}" ]; then
        DEPLOY_EVENT_RUN=$(od -An -N6 -tx1 /dev/urandom 2> /dev/null | tr -d ' \n')
        DEPLOY_EVENT_STARTED=$(date +%s)
        export DEPLOY_EVENT_RUN DEPLOY_EVENT_STARTED
    fi
}

deploy_event() {
    [ "${DEPLOY_EVENTS:-1}" != 0 ] || return 0
    [ -n "${DEPLOY_EVENT_RUN:-}" ] || return 0
    command -v openagents > /dev/null 2>&1 || return 0
    [ -r "${OPENAGENTS_DEPLOY_ANNOUNCER_KEY:-$HOME/work/.secrets/deploy-announcer.key}" ] || return 0
    _de_commit=${4:-}
    _de_subject=
    if [ -n "$_de_commit" ]; then
        _de_subject=$(git -C "${DEPLOY_EVENT_ROOT:-.}" log -1 --format=%s "$_de_commit" -- 2> /dev/null || true)
        _de_commit=$(printf '%s' "$_de_commit" | cut -c1-10)
    fi
    set -- --env "$1" --phase "$2" --tier "${3:-image}" --run "$DEPLOY_EVENT_RUN" \
        --started "${DEPLOY_EVENT_STARTED:-$(date +%s)}" --wait 3 \
        ${_de_commit:+--commit "$_de_commit"} ${_de_subject:+--subject "$_de_subject"} \
        ${5:+--build "$5"} ${DEPLOY_EVENT_BY:+--by "$DEPLOY_EVENT_BY"}
    if [ "${DEPLOY_EVENT_TEST:-}" = 1 ]; then set -- "$@" --test; fi
    openagents deploy event "$@" > /dev/null 2>&1 || true
}

# Watches one deploy for the rest of the script: ENV, TIER, and COMMIT name
# it, and an exit with a failure status while a step is open announces that
# step's failure phase. web.sh only; it sets no other EXIT trap.
deploy_event_watch() { # ENV TIER COMMIT
    _de_env=$1
    _de_tier=$2
    _de_sha=$3
    _de_fail=
    _de_build=
    deploy_event_begin
    trap '_de_status=$?; if [ "$_de_status" -ne 0 ] && [ -n "$_de_fail" ]; then deploy_event "$_de_env" "$_de_fail" "$_de_tier" "$_de_sha" "$_de_build"; fi; exit "$_de_status"' EXIT
}

# Announces PHASE for the watched deploy; FAIL_PHASE, when given, is what an
# exit with a failure status announces until the next step.
deploy_event_step() { # PHASE [FAIL_PHASE]
    _de_fail=${2:-}
    deploy_event "$_de_env" "$1" "$_de_tier" "$_de_sha" "$_de_build"
}
