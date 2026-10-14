#!/bin/sh
# The whole 1.0 web stack on this Mac, wired the way production runs it
# (docs/dev/full-local.md):
#
#   scripts/dev/full-local.sh start [--no-build] [--lab] [--game] [--cad]
#                                                  build, start everything, return
#   scripts/dev/full-local.sh stop                 stop what start started
#   scripts/dev/full-local.sh status               what runs, where
#
# What starts:
#   gateway        127.0.0.1:$GATEWAY_PORT (8791). The account service
#                  (GitHub sign-in with ~/work/.secrets/github-oauth-local.json,
#                  sessions, device sign-in, connected repositories and
#                  projects) and the inference routes (/v1/responses,
#                  /v1/chat/completions) in one process, as in production.
#   chat worker    built from this checkout; every model call goes to the
#                  gateway above (CODER_INFERENCE_URL/KEY). It joins
#                  relay.openagents.com (or $FULL_LOCAL_RELAY, your own
#                  relay) on its own fresh key and answers only the site
#                  below.
#   website        127.0.0.1:$WEB_PORT (4301, the port the local GitHub OAuth
#                  App calls back to): chats on disk, sign-in through the
#                  gateway, Settings -> Claude (encrypted with
#                  ~/work/.secrets/openagents-web-cloud-byo-keys.json),
#                  projects, /device, the Coder sync API, Settings -> Plan,
#                  the chat and component Wasm builds, and /environments when
#                  ~/work/.secrets/boat.env and a Codex login exist
#                  (FULL_LOCAL_ENVIRONMENTS=0 turns it off; real Boat
#                  machines cost money). A chat's /export saves into
#                  ~/Downloads (--local-downloads) and copies the path.
#   coder          $FULL_LOCAL/bin/coder: this checkout's coder-new, pointed at
#                  the website (OPENAGENTS_ORIGIN) with its own state folder,
#                  so `coder login` and /sync on never touch your real Coder.
#   video lab      with --lab: scripts/film/lab.py serve on 127.0.0.1:8792
#                  (VIDEO_LAB_PORT), and the website's chat gets the Lab's tools
#                  through Gemini (docs/video-lab/mvp.md). The gateway then
#                  defaults to 8790, since the film review page holds 8791.
#                  Gemini's credential is $GOOGLE_APPLICATION_CREDENTIALS, else
#                  ~/work/.secrets/gcp-mvp-automation.json.
#   game lab       with --lab or --game: game-lab serve on 127.0.0.1:8793
#                  (GAME_LAB_PORT), and the website's chat gets its game_*
#                  tools the same way (docs/game-lab/mvp.md). --game alone
#                  leaves Video Lab off.
#   cad lab        with --lab or --cad: cad-lab serve on 127.0.0.1:8794
#                  (CAD_LAB_PORT), built in release (its kernel is slow
#                  unoptimised), and the website's chat gets its cad_* tools
#                  the same way (docs/cad-lab/mvp.md). --cad alone leaves
#                  Video Lab off.
#
# Everything persists under $FULL_LOCAL (default ~/.openagents/full-local):
# accounts, sessions, chats, saved keys, the gateway registry, logs.
# Binaries come from $CARGO_TARGET_DIR (default
# ~/work/openagents-target-fulllocal). No key is ever printed. A port held by
# something this script didn't start is left alone: start stops and says so.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
secrets=${OPENAGENTS_SECRETS:-$HOME/work/.secrets}
state=${FULL_LOCAL:-$HOME/.openagents/full-local}
target=${CARGO_TARGET_DIR:-$HOME/work/openagents-target-fulllocal}
gateway_port=${GATEWAY_PORT:-8791}
web_port=${WEB_PORT:-4301}
# The relay the website and the chat worker meet on. Set it to run your
# own (docs/self-host.md), e.g. ws://127.0.0.1:8080.
relay=${FULL_LOCAL_RELAY:-wss://relay.openagents.com}
run="$state/run"
logs="$state/logs"
export CARGO_TARGET_DIR="$target"
# A running stack's ports, for status and stop.
if [ -f "$run/ports" ] && [ "${1:-}" != start ]; then
    # shellcheck disable=SC1091
    . "$run/ports"
fi

usage() {
    sed -n '5,8p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
}

# pid NAME: the recorded pid of a component this script started, if alive.
pid() {
    [ -f "$run/$1.pid" ] || return 1
    p=$(cat "$run/$1.pid")
    [ -n "$p" ] && kill -0 "$p" 2>/dev/null && echo "$p"
}

# holder PORT: pids listening on 127.0.0.1:PORT (or any address).
holder() {
    lsof -nP -iTCP:"$1" -sTCP:LISTEN -t 2>/dev/null | sort -u
}

# free_or_ours NAME PORT: fails, saying why, when PORT is held by anything
# other than this script's NAME process.
free_or_ours() {
    held=$(holder "$2")
    [ -z "$held" ] && return 0
    ours=$(pid "$1" || true)
    for p in $held; do
        if [ "$p" != "$ours" ]; then
            what=$(ps -o command= -p "$p" 2>/dev/null | cut -c1-120)
            echo "Port $2 is already in use by process $p, which this script didn't start:" >&2
            echo "  $what" >&2
            echo "Stop it yourself, or pick another port (e.g. ${3:-PORT}=$(($2 + 100)))." >&2
            if [ "$2" = 4301 ]; then
                echo "GitHub sign-in only comes back to 127.0.0.1:4301 (the local OAuth App's callback)." >&2
            fi
            return 1
        fi
    done
}

wait_http() {
    # wait_http URL SECONDS
    i=0
    while [ "$i" -lt "$2" ]; do
        curl -fsS -o /dev/null "$1" 2>/dev/null && return 0
        sleep 1
        i=$((i + 1))
    done
    return 1
}

load() {
    # load VAR FILE: source FILE for VAR when VAR is unset and FILE exists.
    eval "set_=\${$1:-}"
    if [ -z "$set_" ] && [ -f "$secrets/$2" ]; then
        set -a
        # shellcheck disable=SC1090
        . "$secrets/$2"
        set +a
    fi
}

cargo_build() {
    if openagents lease list > /dev/null 2>&1; then
        openagents lease build --keep-target-dir -- cargo build -q --manifest-path "$root/Cargo.toml" "$@"
    else
        cargo build -q --manifest-path "$root/Cargo.toml" "$@"
    fi
}

build() {
    echo "Building into $target ..."
    if ! cargo_build -p gateway --bin gateway -p coder --bin coder-worker --bin chat-goldens \
        -p openagents-web --bin openagents-web -p coder-new --bin coder-new \
        -p tenancy --example bootstrap_registry > "$logs/build.log" 2>&1; then
        tail -n 30 "$logs/build.log" >&2
        echo "The build failed; the whole output is in $logs/build.log" >&2
        exit 1
    fi
    # The browser builds production serves beside the site. Without them
    # the pages still work: the composer and the component catalog fall
    # back to plain HTML.
    "$root/scripts/build-coder-chat-web.sh" "$state/build/chat" > /dev/null 2>&1 \
        || echo "The chat Wasm build failed; the site starts without it." >&2
    "$root/scripts/build-coder-components-web.sh" "$state/build/components" > /dev/null 2>&1 \
        || echo "The components Wasm build failed; the site starts without it." >&2
    "$root/scripts/build-bunny-web.sh" "$state/build/bunny" > /dev/null 2>&1 \
        || echo "The Grow Little Bunny build failed; /games/grow-little-bunny can't be played." >&2
}

status() {
    any=0
    for name in gateway worker web lab game cad; do
        [ "$name" = lab ] && [ "${lab:-0}" != 1 ] && continue
        [ "$name" = game ] && [ "${game:-0}" != 1 ] && continue
        [ "$name" = cad ] && [ "${cad:-0}" != 1 ] && continue
        if p=$(pid "$name"); then
            any=1
            printf '%-8s running (pid %s)\n' "$name" "$p"
        else
            printf '%-8s stopped\n' "$name"
        fi
    done
    if [ "$any" = 1 ]; then
        echo
        echo "Website:   http://127.0.0.1:$web_port"
        echo "Gateway:   http://127.0.0.1:$gateway_port (admin token in $state/admin.token)"
        echo "Relay:     $relay"
        echo "Coder:     $state/bin/coder login   (then: $state/bin/coder, /sync on)"
        if [ "${lab:-0}" = 1 ]; then
            echo "Video Lab: http://127.0.0.1:${lab_port:-8792} (the chat's films; log in $logs/lab.log)"
        fi
        if [ "${game:-0}" = 1 ]; then
            echo "Game Lab:  http://127.0.0.1:${game_port:-8793} (the chat's games; log in $logs/game.log)"
        fi
        if [ "${cad:-0}" = 1 ]; then
            echo "CAD Lab:   http://127.0.0.1:${cad_port:-8794} (the chat's CAD models; log in $logs/cad.log)"
        fi
        echo "Logs:      $logs/{gateway,worker,web}.log"
    fi
}

stop() {
    for name in web worker gateway lab game cad; do
        if p=$(pid "$name"); then
            kill "$p" 2>/dev/null || true
            i=0
            while kill -0 "$p" 2>/dev/null && [ "$i" -lt 20 ]; do
                sleep 0.5
                i=$((i + 1))
            done
            kill -9 "$p" 2>/dev/null || true
            echo "Stopped $name (pid $p)"
        fi
        rm -f "$run/$name.pid"
    done
}

start() {
    build_first=1
    lab=0
    game=0
    cad=0
    for arg in "$@"; do
        case "$arg" in
        --no-build) build_first=0 ;;
        --lab) lab=1 game=1 cad=1 ;;
        --game) game=1 ;;
        --cad) cad=1 ;;
        *) usage ;;
        esac
    done
    if [ "$lab" = 1 ] && [ -z "${GATEWAY_PORT:-}" ]; then
        gateway_port=8790
    fi
    lab_port=${VIDEO_LAB_PORT:-8792}
    game_port=${GAME_LAB_PORT:-8793}
    cad_port=${CAD_LAB_PORT:-8794}
    if pid gateway > /dev/null || pid worker > /dev/null || pid web > /dev/null; then
        echo "Already running; 'stop' first to restart."
        status
        return 0
    fi
    free_or_ours web "$web_port" WEB_PORT
    free_or_ours gateway "$gateway_port" GATEWAY_PORT

    oauth="$secrets/github-oauth-local.json"
    keyring="$secrets/openagents-web-cloud-byo-keys.json"
    for need in "$oauth" "$keyring"; do
        [ -f "$need" ] || { echo "Missing $need" >&2; exit 1; }
    done
    if [ "$web_port" != 4301 ]; then
        echo "Note: the website is on $web_port; GitHub sign-in only returns to 127.0.0.1:4301." >&2
    fi

    mkdir -p "$state" "$run" "$logs"
    chmod 700 "$state"
    printf 'web_port=%s\ngateway_port=%s\nrelay=%s\nlab=%s\nlab_port=%s\ngame=%s\ngame_port=%s\ncad=%s\ncad_port=%s\n' \
        "$web_port" "$gateway_port" "$relay" "$lab" "$lab_port" "$game" "$game_port" "$cad" "$cad_port" > "$run/ports"
    if [ "$build_first" = 1 ]; then
        build
    elif [ ! -s "$state/build/chat/coder_chat_web_bg.wasm" ] \
        || [ -n "$(find "$root/crates/coder-chat-web" -name '*.rs' -newer "$state/build/chat/coder_chat_web_bg.wasm" 2>/dev/null | head -n 1)" ]; then
        # The chat page's browser code must match this checkout: an older
        # build keeps a sent message on "Working" after the page moves
        # without a full load.
        echo "Rebuilding the chat's browser code (older than this checkout) ..."
        "$root/scripts/build-coder-chat-web.sh" "$state/build/chat" > "$logs/chat-wasm.log" 2>&1 \
            || echo "The chat Wasm build failed; see $logs/chat-wasm.log" >&2
    fi

    # Google first (Vertex AI): Gemini for the chat's own tools and the
    # product knowledge embeddings. $GOOGLE_APPLICATION_CREDENTIALS, else
    # this Mac's automation service account.
    google_credentials=${GOOGLE_APPLICATION_CREDENTIALS:-}
    if [ -z "$google_credentials" ] && [ -f "$secrets/gcp-mvp-automation.json" ]; then
        google_credentials="$secrets/gcp-mvp-automation.json"
    fi

    load OPENROUTER_API_KEY openrouter.env
    load AI_GATEWAY_API_KEY ai-gateway.env
    load TYPESAFE_API_KEY typesafe.env
    # Google first (#11602): the gateway, the chat worker's product-note
    # embeddings, and the website reach Vertex on the automation account's
    # key when no Google credential is set; OpenRouter and the Vercel AI
    # Gateway stay behind it as fallbacks.
    if [ -z "${GOOGLE_APPLICATION_CREDENTIALS:-}${VERTEX_ACCESS_TOKEN:-}" ] \
        && [ -f "$secrets/gcp-mvp-automation.json" ]; then
        export GOOGLE_APPLICATION_CREDENTIALS="$secrets/gcp-mvp-automation.json"
    fi

    mkdir -p "$state/gateway/usage" "$state/gateway/attempts" "$state/worker-usage" \
        "$state/tasks" "$state/chats" "$state/byo" "$state/coder" "$state/bin"
    chmod 700 "$state/byo" "$state/coder"

    # One registry: `house` is the service tenant the chat worker's key
    # calls inference as; `signup` is where GitHub sign-up puts new
    # accounts' personal workspaces (docs/auth/github.md).
    if [ ! -s "$state/service.key" ]; then
        (umask 077 && "$target/debug/examples/bootstrap_registry" --registry "$state/gateway/registry" \
            --tenant house --also-tenant signup --door local-kev --model kev-latest \
            --signature "sha256:$(printf '0%.0s' $(seq 1 64))" > "$state/service.key")
    fi
    [ -s "$state/admin.token" ] || (umask 077 && openssl rand -hex 24 > "$state/admin.token")
    [ -s "$state/worker.secret" ] || (umask 077 && openssl rand -hex 32 > "$state/worker.secret")
    [ -s "$state/csrf.key" ] || (umask 077 && openssl rand -hex 32 > "$state/csrf.key")

    cat > "$state/gateway.json" <<EOF
{
  "v": "openagents.gateway.v1",
  "listen": "127.0.0.1:$gateway_port",
  "registry": "$state/gateway/registry",
  "accounts": {
    "signup_tenant": "signup",
    "open_signup": true,
    "github": {
      "credentials": "$oauth",
      "redirect_url": "http://127.0.0.1:$web_port/auth/github/callback"
    }
  },
  "inference": {
    "admin_token_env": "INFERENCE_ADMIN_TOKEN",
    "service_tenants": ["house"],
    "journal": "$state/gateway/attempts",
    "accounts": [
      {"id": "google-credit", "upstream": "vertex", "granted": 30000000000, "balance": 30000000000, "basis": "prepaid"},
      {"id": "zai-credit", "upstream": "zai", "granted": 100000000, "balance": 100000000, "basis": "prepaid"},
      {"id": "pro-free-capacity", "upstream": "pro", "granted": 0, "balance": 0, "basis": "free_capacity"},
      {"id": "openrouter", "upstream": "openrouter", "granted": 0, "balance": 0, "basis": "pay_as_you_go"},
      {"id": "vercel", "upstream": "vercel", "granted": 0, "balance": 0, "basis": "pay_as_you_go"}
    ]
  }
}
EOF
    rm -f "$state/cloud.json"
    (umask 077 && cat > "$state/cloud.json" <<EOF
{"schema": "openagents.cloud.web-config.v1", "public_origin": "http://127.0.0.1:$web_port", "account_service": "http://127.0.0.1:$gateway_port", "csrf_secret": "$state/csrf.key"}
EOF
    )

    # A gateway stopped by a signal leaves its ledger lock behind; clear it
    # when the process named inside is gone.
    lock="$state/gateway/registry/quota-ledger.lock"
    if [ -f "$lock" ]; then
        holder_pid=$(sed -n 's/^pid \([0-9]*\)$/\1/p' "$lock")
        if [ -z "$holder_pid" ] || ! kill -0 "$holder_pid" 2>/dev/null; then
            rm -f "$lock"
        fi
    fi

    # The gateway: accounts and inference.
    (
        INFERENCE_ADMIN_TOKEN=$(cat "$state/admin.token")
        export INFERENCE_ADMIN_TOKEN
        cd "$state/gateway"
        exec nohup "$target/debug/gateway" --config "$state/gateway.json" > "$logs/gateway.log" 2>&1
    ) &
    echo $! > "$run/gateway.pid"
    if ! wait_http "http://127.0.0.1:$gateway_port/healthz" 60; then
        echo "The gateway didn't start; see $logs/gateway.log" >&2
        stop > /dev/null
        exit 1
    fi

    # The chat worker, its model calls through the gateway.
    (
        CODER_WORKER_SECRET=$(cat "$state/worker.secret")
        CODER_INFERENCE_KEY=$(cat "$state/service.key")
        export CODER_WORKER_SECRET CODER_INFERENCE_KEY
        export CODER_INFERENCE_URL="http://127.0.0.1:$gateway_port"
        export CODER_RELAY="$relay" CODER_WORKER_OPEN=1 CODER_WORKER_JOBS=16
        export CODER_WORKER_USAGE_DIR="$state/worker-usage"
        export OPENAGENTS_PRODUCT_KNOWLEDGE="$root/knowledge/openagents"
        # Product notes' embeddings take the house order: Vertex first,
        # then OpenAI or OpenRouter, the Vercel AI Gateway last (#11602).
        # Vertex needs the Google credential and its project, as
        # production's worker sets them (deploy/coder-worker-chat.env.example).
        export CODER_AI_GATEWAY_KEY="${AI_GATEWAY_API_KEY:-}"
        if [ -n "$google_credentials" ]; then
            export GOOGLE_APPLICATION_CREDENTIALS="$google_credentials"
            KB_VERTEX_PROJECT=${KB_VERTEX_PROJECT:-$(sed -n 's/.*"project_id": *"\([^"]*\)".*/\1/p' "$google_credentials" | head -n 1)}
            KB_VERTEX_PROJECT=${KB_VERTEX_PROJECT:-openagentsgemini}
            export KB_VERTEX_PROJECT KB_VERTEX_MODEL=${KB_VERTEX_MODEL:-gemini-embedding-001}
        fi
        cd "$state"
        exec nohup "$target/debug/coder-worker" > "$logs/worker.log" 2>&1
    ) &
    echo $! > "$run/worker.pid"
    key=""
    i=0
    while [ "$i" -lt 90 ]; do
        key=$(sed -n 's/^worker  *\([0-9a-f]\{64\}\)$/\1/p' "$logs/worker.log" 2>/dev/null | head -n 1)
        grep -q "subscribed" "$logs/worker.log" 2>/dev/null && [ -n "$key" ] && break
        pid worker > /dev/null || break
        sleep 1
        i=$((i + 1))
    done
    if [ -z "$key" ]; then
        echo "The chat worker didn't start; see $logs/worker.log" >&2
        stop > /dev/null
        exit 1
    fi
    echo "$key" > "$state/worker.pub"

    # Environments, when Boat and a Codex login are set up.
    environments=""
    codex_home=${CODEX_HOME:-$HOME/.codex}
    if [ "${FULL_LOCAL_ENVIRONMENTS:-1}" != 0 ] && [ -f "$secrets/boat.env" ] \
        && grep -q '^ *\(export \)\{0,1\}BOAT_API_KEY=.' "$secrets/boat.env" \
        && [ -f "$codex_home/auth.json" ]; then
        mkdir -p "$state/environments"
        chmod 700 "$state/environments"
        claude=null
        [ -n "${ANTHROPIC_API_KEY:-}" ] && claude='"ANTHROPIC_API_KEY"'
        github=null
        credentials='[]'
        if [ -n "${GH_TOKEN:-}" ] || gh auth token > /dev/null 2>&1; then
            github='"GH_TOKEN"'
            credentials='["GH_TOKEN"]'
        fi
        (umask 077 && cat > "$state/environments/environments.json" <<EOF
{
  "schema": "openagents.environment.studio.v1",
  "state": "$state/environments",
  "machines": {
    "schema": "openagents.environment.owners.v1",
    "provider": "boat",
    "workdir": "/home/user/repo",
    "template": null,
    "credential_names": $credentials,
    "tick_seconds": 15
  },
  "owner": { "workspace": "local", "principal": "owner" },
  "codex_home": "$codex_home",
  "size": "small",
  "deadline_seconds": 7200,
  "github_token": $github,
  "claude_key": $claude
}
EOF
        )
        environments="$state/environments/environments.json"
    fi

    # Video Lab (docs/video-lab/mvp.md): the Lab service, and the website's
    # chat pointed at it with Gemini as the model that calls its tools.
    lab_url=""
    if [ "$lab" = 1 ]; then
        lab_url="http://127.0.0.1:$lab_port"
        if curl -fsS -o /dev/null "$lab_url/healthz" 2>/dev/null; then
            echo "Video Lab is already running on $lab_url."
        else
            VIDEO_LAB_PORT=$lab_port nohup python3 "$root/scripts/film/lab.py" serve --port "$lab_port" \
                > "$logs/lab.log" 2>&1 &
            echo $! > "$run/lab.pid"
            if ! wait_http "$lab_url/healthz" 30; then
                echo "Video Lab didn't start; see $logs/lab.log" >&2
                stop > /dev/null
                exit 1
            fi
        fi
        [ -n "$google_credentials" ] || echo "No Google credential, so the chat can't use the Lab's tools (Gemini)." >&2
    fi

    # Game Lab (docs/game-lab/mvp.md): the service that keeps the games'
    # versions, checks edits with bot runs and builds the game once; the
    # website's chat gets its tools the same way as Video Lab's.
    game_url=""
    if [ "$game" = 1 ]; then
        game_url="http://127.0.0.1:$game_port"
        if curl -fsS -o /dev/null "$game_url/healthz" 2>/dev/null; then
            echo "Game Lab is already running on $game_url."
        else
            if [ ! -x "$target/debug/game-lab" ] || [ "$build_first" = 1 ]; then
                cargo_build -p game-lab --bin game-lab >> "$logs/build.log" 2>&1 \
                    || { echo "The Game Lab build failed; see $logs/build.log" >&2; stop > /dev/null; exit 1; }
            fi
            nohup "$target/debug/game-lab" serve --port "$game_port" --repo "$root" \
                > "$logs/game.log" 2>&1 &
            echo $! > "$run/game.pid"
            if ! wait_http "$game_url/healthz" 30; then
                echo "Game Lab didn't start; see $logs/game.log" >&2
                stop > /dev/null
                exit 1
            fi
        fi
        [ -n "$google_credentials" ] || echo "No Google credential, so the chat can't use Game Lab's tools (Gemini)." >&2
    fi

    # CAD Lab (docs/cad-lab/mvp.md): the service that keeps the CAD
    # documents' versions and builds and checks every edit; the website's
    # chat gets its tools the same way.
    cad_url=""
    if [ "$cad" = 1 ]; then
        cad_url="http://127.0.0.1:$cad_port"
        if curl -fsS -o /dev/null "$cad_url/healthz" 2>/dev/null; then
            echo "CAD Lab is already running on $cad_url."
        else
            if [ ! -x "$target/release/cad-lab" ] || [ "$build_first" = 1 ]; then
                cargo_build --release -p cad-lab --bin cad-lab >> "$logs/build.log" 2>&1 \
                    || { echo "The CAD Lab build failed; see $logs/build.log" >&2; stop > /dev/null; exit 1; }
            fi
            nohup "$target/release/cad-lab" serve --port "$cad_port" \
                > "$logs/cad.log" 2>&1 &
            echo $! > "$run/cad.pid"
            if ! wait_http "$cad_url/healthz" 30; then
                echo "CAD Lab didn't start; see $logs/cad.log" >&2
                stop > /dev/null
                exit 1
            fi
        fi
        [ -n "$google_credentials" ] || echo "No Google credential, so the chat can't use CAD Lab's tools (Gemini)." >&2
    fi

    # The website.
    (
        if [ -n "$cad_url" ]; then
            export OPENAGENTS_CAD_LAB="$cad_url"
            [ -n "$google_credentials" ] && export GOOGLE_APPLICATION_CREDENTIALS="$google_credentials"
        fi
        if [ -n "$game_url" ]; then
            export OPENAGENTS_GAME_LAB="$game_url"
            [ -n "$google_credentials" ] && export GOOGLE_APPLICATION_CREDENTIALS="$google_credentials"
        fi
        if [ -n "$lab_url" ]; then
            export OPENAGENTS_VIDEO_LAB="$lab_url"
            [ -n "$google_credentials" ] && export GOOGLE_APPLICATION_CREDENTIALS="$google_credentials"
        fi
        if [ -n "$environments" ]; then
            set -a
            # shellcheck disable=SC1091
            . "$secrets/boat.env"
            set +a
            if [ -z "${GH_TOKEN:-}" ]; then
                GH_TOKEN=$(gh auth token 2>/dev/null || true)
                [ -n "$GH_TOKEN" ] && export GH_TOKEN
            fi
            set -- --environments "$environments"
        else
            set --
        fi
        [ -s "$state/build/chat/coder_chat_web_bg.wasm" ] && set -- "$@" --chat-build "$state/build/chat"
        [ -s "$state/build/components/coder_components_web_bg.wasm" ] \
            && set -- "$@" --components-build "$state/build/components"
        [ -s "$state/build/bunny/bunny_web_bg.wasm" ] && set -- "$@" --bunny "$state/build/bunny"
        export OPENAGENTS_WEB_CHAT_WORKER="$key" OPENAGENTS_WEB_CHAT_RELAY="$relay"
        cd "$state"
        exec nohup "$target/debug/openagents-web" --listen "127.0.0.1:$web_port" \
            --store "$state/tasks" --chat-store "$state/chats" \
            --cloud-config "$state/cloud.json" --github-oauth "$oauth" \
            --cloud-byo "$state/byo" --cloud-byo-keys "$keyring" \
            --plan-meter "$state/plan-meter.sqlite" --local-downloads "$HOME/Downloads" \
            "$@" > "$logs/web.log" 2>&1
    ) &
    echo $! > "$run/web.pid"
    if ! wait_http "http://127.0.0.1:$web_port/health" 120; then
        echo "The website didn't start; see $logs/web.log" >&2
        stop > /dev/null
        exit 1
    fi

    # Coder pointed at this website, with its own sign-in and chats.
    cat > "$state/bin/coder" <<EOF
#!/bin/sh
# coder-new from $root, signed in to http://127.0.0.1:$web_port only.
export OPENAGENTS_ORIGIN=http://127.0.0.1:$web_port
case "\${1:-}" in
login | logout) command=\$1; shift; exec "$target/debug/coder-new" "\$command" --state "$state/coder" "\$@" ;;
esac
exec "$target/debug/coder-new" --state "$state/coder" "\$@"
EOF
    chmod 755 "$state/bin/coder"

    [ -n "$environments" ] && echo "Environments are on (real Boat machines; they cost money)."
    echo
    status
}

case "${1:-}" in
start)
    shift
    start "$@"
    ;;
stop) stop ;;
status) status ;;
*) usage ;;
esac
