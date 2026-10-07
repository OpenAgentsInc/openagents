# The retail task owner on a Boat sandbox, version 1 (#10748).
#
# retail_cloud::boat writes this file to /tmp/oa-retail/owner-v1.sh through
# the Boat files API and runs it as `sh owner-v1.sh VERB ARG...` through the
# Boat commands API. It writes no JSON: the adapter reads the plain files it
# leaves under the task directory through the files API.
#
# Task directory layout, under /tmp/oa-retail/task/<task>:
#   spec.json, prompt, key, workspace, max_seconds, checks/<n>  (adapter)
#   started/      created once; the runner starts only when it is created
#   pid           the runner's process (group) id
#   status        queued | running | cancelled | ended END PATCH, then
#                 `check N EXIT` lines
#   events        one progress line per event; the cursor is the line number
#   artifacts/    patch, checks, log, scrubbed of the customer's key
#   manifest      `NAME KIND SHA256 SIZE` lines, written last
#   stop/<req>    the stop receipt for request <req>
set -u
umask 077

digest() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

size() {
    wc -c < "$1" | tr -d ' '
}

mode() {
    stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1"
}

event() {
    printf '%s\n' "$2" | tr -d '\r' | head -n 1 >> "$1/events"
}

put() {
    # Replace a file atomically.
    printf '%s\n' "$2" > "$1.tmp" && mv "$1.tmp" "$1"
}

scrub() {
    # Replace the key's text in a file without putting it on a command line.
    [ -s "$2" ] || return 0
    awk 'NR == FNR { k = $0; next }
        { while (k != "" && (i = index($0, k)) > 0) {
              $0 = substr($0, 1, i - 1) "[redacted]" substr($0, i + length(k))
          }
          print }' "$1" "$2" > "$2.scrubbed" && mv "$2.scrubbed" "$2"
}

run() {
    dir=$1
    echo $$ > "$dir/pid"
    ws=$(cat "$dir/workspace")
    key=$(cat "$dir/key")
    max=$(cat "$dir/max_seconds")
    engine=${OA_RETAIL_ENGINE:-codex}
    art="$dir/artifacts"
    mkdir -p "$art"
    : > "$art/log"
    : > "$art/checks"
    # A stop before the runner started leaves nothing to run.
    [ "$(head -n 1 "$dir/status")" = queued ] || exit 0
    put "$dir/status" running
    event "$dir" "the executor started"
    CODEX_HOME="$dir/engine-home"
    export CODEX_HOME
    mkdir -p "$CODEX_HOME"
    end=completed
    if ! "$engine" login --with-api-key < "$key" >> "$art/log" 2>&1; then
        end=failed
    fi
    if [ "$end" = completed ]; then
        limit=""
        command -v timeout >/dev/null 2>&1 && limit="timeout $max"
        (cd "$ws" && $limit "$engine" exec --full-auto --skip-git-repo-check \
            "$(cat "$dir/prompt")") >> "$art/log" 2>&1
        code=$?
        case $code in
            0) ;;
            124) end=timed_out ;;
            *) end=failed ;;
        esac
    fi
    event "$dir" "the executor ended: $end"
    git -C "$ws" add -A >/dev/null 2>&1
    git -C "$ws" diff --cached --binary > "$art/patch" 2>/dev/null
    scrub "$key" "$art/patch"
    patch=-
    [ -s "$art/patch" ] && patch=$(digest "$art/patch")
    lines=""
    n=0
    while [ -f "$dir/checks/$n" ]; do
        printf '$ %s\n' "$(cat "$dir/checks/$n")" >> "$art/checks"
        (cd "$ws" && sh -c "$(cat "$dir/checks/$n")") >> "$art/checks" 2>&1
        lines="${lines}check $n $?
"
        n=$((n + 1))
    done
    event "$dir" "$n checks ran"
    scrub "$key" "$art/log"
    scrub "$key" "$art/checks"
    : > "$dir/manifest.tmp"
    for name in patch checks log; do
        case $name in patch) kind=patch ;; checks) kind=checks ;; *) kind=log ;; esac
        if [ -f "$art/$name" ]; then
            printf '%s %s %s %s\n' "$name" "$kind" "$(digest "$art/$name")" \
                "$(size "$art/$name")" >> "$dir/manifest.tmp"
        fi
    done
    mv "$dir/manifest.tmp" "$dir/manifest"
    # A stop that came first keeps its status.
    if [ "$(head -n 1 "$dir/status")" = running ]; then
        printf 'ended %s %s\n%s' "$end" "$patch" "$lines" > "$dir/status.tmp"
        mv "$dir/status.tmp" "$dir/status"
    fi
}

verb=${1:-}
[ $# -gt 0 ] && shift
case $verb in
    clone)
        repo=$1 commit=$2 ws=$3
        if [ "$(git -C "$ws" rev-parse HEAD 2>/dev/null)" != "$commit" ]; then
            rm -rf "$ws"
            mkdir -p "$ws"
            git -C "$ws" init -q &&
                git -C "$ws" remote add origin "$repo" &&
                git -C "$ws" fetch -q --depth 1 origin "$commit" &&
                git -C "$ws" checkout -q --detach FETCH_HEAD || exit 1
        fi
        printf '%s %s\n' "$repo" "$commit" > "$(dirname "$ws")/source"
        if [ -z "$(git -C "$ws" status --porcelain)" ]; then state=clean; else state=dirty; fi
        printf '%s %s\n' "$(git -C "$ws" rev-parse HEAD)" "$state"
        ;;
    prepare)
        mkdir -p "$(dirname "$1")" && chmod 700 "$(dirname "$1")" &&
            : > "$1" && chmod 600 "$1" && echo prepared
        ;;
    private)
        chmod 600 "$1" && mode "$1"
        ;;
    remove)
        rm -f -- "$1" && echo removed
        ;;
    exists)
        if [ -e "$1" ]; then echo yes; else echo no; fi
        ;;
    submit)
        dir=$1
        if mkdir "$dir/started" 2>/dev/null; then
            put "$dir/status" queued
            : > "$dir/events"
            if command -v setsid >/dev/null 2>&1; then
                setsid sh "$0" run "$dir" < /dev/null > /dev/null 2>&1 &
            else
                sh "$0" run "$dir" < /dev/null > /dev/null 2>&1 &
            fi
            echo new
        else
            echo existing
        fi
        ;;
    run)
        run "$1"
        ;;
    stop)
        dir=$1 request=$2
        receipt="$dir/stop/$request"
        if [ -f "$receipt" ]; then
            echo existing
            exit 0
        fi
        mkdir -p "$dir/stop"
        started=0
        [ -d "$dir/started" ] && started=1
        if [ -f "$dir/pid" ]; then
            pid=$(cat "$dir/pid")
            kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null
        fi
        case $(head -n 1 "$dir/status" 2>/dev/null) in
            ended*) ;;
            *) put "$dir/status" cancelled ;;
        esac
        {
            printf 'at %s\nstarted %s\n' "$(date +%s)" "$started"
            [ -s "$dir/artifacts/patch" ] && printf 'effect %s\n' "$(digest "$dir/artifacts/patch")"
            echo status
            cat "$dir/status" 2>/dev/null
        } > "$receipt.tmp"
        mv "$receipt.tmp" "$receipt"
        echo stopped
        ;;
    *)
        echo "usage: owner-v1.sh clone|prepare|private|remove|exists|submit|stop ARG..." >&2
        exit 64
        ;;
esac
