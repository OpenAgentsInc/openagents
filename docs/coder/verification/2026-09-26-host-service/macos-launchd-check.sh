#!/bin/bash
# Throwaway macOS launchd runtime check for coder-service. It creates only
# temporary files and one uniquely labeled gui-domain agent whose definition
# is registered from a temporary directory, never ~/Library/LaunchAgents, and
# it removes the agent on exit.
#
#   WT=<checkout> BIN=<checkout>/target/debug/coder-service \
#     bash docs/coder/verification/2026-09-26-host-service/macos-launchd-check.sh
set -u
WT=${WT:?set WT to the repository checkout}
BIN=${BIN:-$WT/target/debug/coder-service}
BASE=$(mktemp -d "${TMPDIR:-/tmp}/coder-service-check.XXXXXX")
chmod 700 "$BASE"
LABEL=org.openagents.test.coder-service-$(date +%s)
KEY=79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
ROOT=$BASE/host
BUNDLES=$BASE/host-bundle
TASKS=$BASE/tasks
REG=$BASE/LaunchAgents
mkdir -m 700 "$TASKS"
printf '{"schema":"openagents.coder.task-store.v2","evidence":"before"}' > "$TASKS/tasks.json"
echo "base=$BASE label=$LABEL"

fixture() { # $1 tag, $2 mode (good|exit)
  cat > "$BASE/$1" <<EOF
#!/bin/sh
case "\$1" in
  --version) echo "fixture host $1"; exit 0;;
  task) echo "usage: coder task execute"; exit 0;;
esac
if [ "$2" = exit ]; then printf broken > '$TASKS/marker'; exit 3; fi
if [ "$2" = silent ]; then printf broken > '$TASKS/marker'; exec sleep 3600; fi
printf '%s' "\$OPENAGENTS_HOST_VERSION" > '$TASKS/marker'
printf '{"schema":"openagents.coder.host-ready.v1","generation":%s,"version":"%s","protocol_version":1,"capabilities":["fixture-$1"]}' "\$OPENAGENTS_HOST_GENERATION" "\$OPENAGENTS_HOST_VERSION" > "\$OPENAGENTS_HOST_READY_FILE.tmp"
mv "\$OPENAGENTS_HOST_READY_FILE.tmp" "\$OPENAGENTS_HOST_READY_FILE"
exec sleep 3600
EOF
  chmod 700 "$BASE/$1"
  local sha
  sha=$(shasum -a 256 "$BASE/$1" | cut -d' ' -f1)
  python3 "$WT/scripts/coder-host.py" --root "$BUNDLES" --tasks "$TASKS" install \
    --binary "$BASE/$1" --sha256 "$sha" --source-revision "$(printf 'a%.0s' {1..40})" --uncommitted-source >/dev/null || exit 1
  echo "$sha"
}
V1=$(fixture one good); V2=$(fixture two good); BAD=$(fixture three exit); SILENT=$(fixture four silent)
echo "v1=$V1 v2=$V2 bad=$BAD silent=$SILENT"

cleanup() {
  echo "--- cleanup"
  "$BIN" --root "$ROOT" service uninstall
  launchctl print "gui/$(id -u)/$LABEL" >/dev/null 2>&1 && echo "STILL LOADED" || echo "unloaded: launchctl print fails as expected"
  pgrep -f "$BASE" && echo "LEFTOVER PROCESSES" || echo "no process references $BASE"
}
trap cleanup EXIT

echo "--- install"
"$BIN" --root "$ROOT" service install --platform macos --label "$LABEL" --registration-dir "$REG" \
  --bundle-root "$BUNDLES" --state "$TASKS" --host-key "$KEY" --version "$V1" --ready-timeout 20 --stop-grace 2 -- host serve || exit 1
sleep 2
echo "--- status after install"
"$BIN" --root "$ROOT" service status
echo "--- update to v2"
"$BIN" --root "$ROOT" update --to "$V2" --wait 30; echo "exit=$?"
echo "marker=$(cat "$TASKS/marker")"
echo "--- update to failing build"
"$BIN" --root "$ROOT" update --to "$BAD" --wait 30; echo "exit=$?"
echo "marker=$(cat "$TASKS/marker") tasks=$(cat "$TASKS/tasks.json")"
echo "--- restart"
"$BIN" --root "$ROOT" service restart
sleep 3
"$BIN" --root "$ROOT" descriptor
echo "--- launcher SIGKILL during a trial"
REQ=$("$BIN" --root "$ROOT" update --to "$SILENT" | python3 -c 'import json,sys;print(json.load(sys.stdin)["request"])')
for i in $(seq 1 100); do grep -q '"state":"trial"' "$ROOT/descriptor.json" && break; sleep 0.1; done
TRIAL_GROUP=$(python3 -c "import json;print(json.load(open('$ROOT/launcher.json'))['host_group'])")
LPID=$("$BIN" --root "$ROOT" service status | python3 -c 'import json,sys;print(json.load(sys.stdin)["pid"])')
echo "trial in progress; sending SIGKILL to launcher pid $LPID (trial host group $TRIAL_GROUP)"
kill -9 "$LPID"
for i in $(seq 1 600); do
  OUT=$("$BIN" --root "$ROOT" descriptor 2>/dev/null)
  echo "$OUT" | python3 -c "import json,sys;d=json.load(sys.stdin);sys.exit(0 if d['update']['request']=='$REQ' and d['update']['state']=='rolled-back' and d['state']=='ready' else 1)" 2>/dev/null && break
  sleep 0.1
done
echo "$OUT"
echo "processes left in trial group $TRIAL_GROUP: $(ps -o pid= -g "$TRIAL_GROUP" | tr -d ' ' | tr '\n' ' ')"
echo "new launcher pid: $("$BIN" --root "$ROOT" service status | python3 -c 'import json,sys;print(json.load(sys.stdin)["pid"])')"
echo "marker=$(cat "$TASKS/marker") tasks=$(cat "$TASKS/tasks.json")"
echo "--- status before uninstall"
"$BIN" --root "$ROOT" service status | grep -E '"(running|enabled|starts_at|survives_logout|pending_restart|loaded|registered)"'
