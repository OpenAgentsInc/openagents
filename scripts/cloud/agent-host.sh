#!/bin/sh
# The GCE agent host: a third OpenAgents device for agent chats, beside the
# Mac and coderos-4080 (#12116, docs/cloud/agent-host.md).
#
#   scripts/cloud/agent-host.sh status        # VM state, engine unit, idle check
#   scripts/cloud/agent-host.sh start         # start a stopped host (idle stop powers it off)
#   scripts/cloud/agent-host.sh stop          # power it off now (only its disk bills)
#   scripts/cloud/agent-host.sh ssh [CMD]     # ssh as coder over IAP
#   scripts/cloud/agent-host.sh login-codex   # Codex device login on the host
#   scripts/cloud/agent-host.sh upgrade       # rebuild the engine from origin/main, restart it
#
# Every gcloud call runs as the automation account (CLOUDSDK_CONFIG, default
# ~/work/.secrets/gcloud-sa-config), never a person's login. ssh uses the
# pool key of this computer (~/.openagents/cloud/pool_ed25519).
set -eu

NAME=${OA_AGENT_HOST:-oa-agent-host-1}
ZONE=${OA_AGENT_HOST_ZONE:-us-central1-a}
PROJECT=${OA_PROJECT:-openagentsgemini}
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
KEY=$HOME/.openagents/cloud/pool_ed25519
KNOWN=$HOME/.openagents/cloud/agent-host-known_hosts
UNIT=openagents-agent-host.service

g() { gcloud "$@" --project "$PROJECT" --quiet; }

remote() {
    tty=
    if [ "${1:-}" = -t ]; then tty=-t; shift; fi
    ssh $tty -i "$KEY" -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$KNOWN" \
        -o IdentitiesOnly=yes -o ConnectTimeout=40 -o ServerAliveInterval=20 -o LogLevel=ERROR \
        -o "ProxyCommand=gcloud compute start-iap-tunnel $NAME 22 --listen-on-stdin --project $PROJECT --zone $ZONE --verbosity=error" \
        "coder@$NAME" "$@"
}

state() { g compute instances describe "$NAME" --zone "$ZONE" --format='value(status)'; }

case ${1:-} in
    status)
        s=$(state)
        echo "$NAME ($ZONE): $s"
        [ "$s" = RUNNING ] || exit 0
        remote 'export XDG_RUNTIME_DIR=/run/user/$(id -u)
            printf "engine: %s (%s)\n" "$(systemctl --user is-active '"$UNIT"')" "$(cat ~/.openagents/native-host/current/COMMIT 2>/dev/null || echo unknown)"
            printf "idle stop: %s, %s min\n" "$(systemctl is-active oa-agent-host-idle.timer)" \
                "$(curl -sf -m 2 -H Metadata-Flavor:Google http://metadata.google.internal/computeMetadata/v1/instance/attributes/oa-agent-host-idle-minutes || echo 60)"
            printf "codex: %s\n" "$(codex login status 2>&1 | head -1)"
            printf "disk: %s free\n" "$(df -h / | awk "NR==2{print \$4}")"' ;;
    start)
        g compute instances start "$NAME" --zone "$ZONE" >/dev/null
        echo "$NAME is starting; the engine comes up with it and the device shows in about a minute." ;;
    stop)
        g compute instances stop "$NAME" --zone "$ZONE" --async >/dev/null
        echo "$NAME is stopping." ;;
    ssh)
        shift
        if [ $# -eq 0 ]; then remote -t; else remote "$@"; fi ;;
    login-codex)
        echo "Open the link below and enter the code; Codex on $NAME is then signed in."
        remote -t 'codex login --device-auth && codex login status' ;;
    upgrade)
        remote "FORCE=${FORCE:-} bash -se" <<'EOS'
set -eu
# A restart ends the chats running on the engine: wait for them (FORCE=1 skips).
for p in codex claude grok cargo; do
    if [ -z "${FORCE:-}" ] && pgrep -u coder -x "$p" >/dev/null; then
        echo "an agent is working on this host ($p); upgrade later (or FORCE=1)" >&2; exit 1
    fi
done
export PATH="$HOME/.cargo/bin:$PATH" CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cd ~/openagents && git fetch -q origin main && git checkout -q --detach origin/main
rev=$(git rev-parse HEAD)
cd apps/openagents
CARGO_TARGET_DIR=$HOME/work/openagents-target-app cargo build --locked -q -p openagents --no-default-features
dir=$HOME/.openagents/native-host/versions/$rev
mkdir -p "$dir"
cp "$HOME/work/openagents-target-app/debug/openagents" "$dir/openagents.tmp"
strip "$dir/openagents.tmp" 2>/dev/null || true
mv -f "$dir/openagents.tmp" "$dir/openagents"; chmod 555 "$dir/openagents"; echo "$rev" > "$dir/COMMIT"
ln -sfn "$dir" "$HOME/.openagents/native-host/current"
export XDG_RUNTIME_DIR=/run/user/$(id -u)
systemctl --user restart openagents-agent-host.service
echo "engine at ${rev%"${rev#??????????}"}: $(systemctl --user is-active openagents-agent-host.service)"
EOS
        ;;
    *) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
