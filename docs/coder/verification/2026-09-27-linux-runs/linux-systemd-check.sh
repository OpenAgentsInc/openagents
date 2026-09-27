#!/bin/bash
# Throwaway Linux systemd runtime check for coder-service running the real
# `coder host serve`. Every file lives under BASE except the one registration
# link install adds to ~/.config/systemd/user and the link `systemctl --user
# enable` adds to default.target.wants; uninstall removes both, and the trap
# uninstalls on any exit. It never enables linger or reboots.
#
#   WT=<source checkout> BIN=<coder-service> CODER=<coder> BASE=<new temp dir> \
#     LABEL=openagents-test-<unique> PORT=<free loopback port> \
#     bash linux-systemd-check.sh
set -u
WT=${WT:?set WT to the source checkout}
BIN=${BIN:?set BIN to the coder-service binary}
CODER=${CODER:?set CODER to the coder binary}
BASE=${BASE:?set BASE to a fresh temporary directory}
LABEL=${LABEL:?set LABEL to a unique throwaway unit name}
PORT=${PORT:?set PORT to a free loopback port}
SVC=$BASE/svc
ROOT=$SVC/host
BUNDLES=$SVC/host-bundle
TASKS=$SVC/tasks
ACCESS=$SVC/coder-access
HROOT=$SVC/coder-host
REG=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
OWNER=79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
RELAY=ws://127.0.0.1:9/
mkdir -m 700 "$SVC" "$TASKS" || exit 1
if [ -e "$REG/$LABEL.service" ] || [ -L "$REG/$LABEL.service" ]; then
  echo "$REG/$LABEL.service already exists"; exit 1
fi
touch "$BASE/start-marker"
echo "base=$BASE label=$LABEL port=$PORT"
echo "--- machine"
uname -srm
ldd --version | head -n 1
systemctl --version | head -n 1
loginctl show-user "$(id -u)" --property=Linger

stage() { # $1 binary; prints its bundle identity
  local sha
  sha=$(sha256sum "$1" | cut -d' ' -f1)
  python3 "$WT/scripts/coder-host.py" --root "$BUNDLES" --tasks "$TASKS" install \
    --binary "$1" --sha256 "$sha" --source-revision "$(printf 'a%.0s' {1..40})" \
    --uncommitted-source >/dev/null || exit 1
  echo "$sha"
}
fixture() { # $1 name, $2 exit|silent: a stand-in host that never reports ready
  cat > "$SVC/$1" <<EOF
#!/bin/sh
case "\$1" in
  --version) echo "fixture host $1"; exit 0;;
  task) echo "usage: coder task execute"; exit 0;;
esac
printf broken > '$TASKS/marker'
if [ "$2" = exit ]; then exit 3; fi
exec sleep 3600
EOF
  chmod 700 "$SVC/$1"
}
# Two real builds with distinct identities: the second has one trailing
# comment line, which the ELF loader ignores.
cp "$CODER" "$SVC/coder-v1"
cp "$CODER" "$SVC/coder-v2"
printf '\n# v2\n' >> "$SVC/coder-v2"
fixture bad exit
fixture silent silent
V1=$(stage "$SVC/coder-v1"); V2=$(stage "$SVC/coder-v2")
BAD=$(stage "$SVC/bad"); SILENT=$(stage "$SVC/silent")
echo "v1=$V1 v2=$V2 bad=$BAD silent=$SILENT"

"$CODER" host init --owner "$OWNER" --relay "$RELAY" --loopback-test \
  --state "$ACCESS" --root "$HROOT" >/dev/null || exit 1
KEY=$("$CODER" host public-key --state "$ACCESS" --root "$HROOT") || exit 1
echo "host key=$KEY"
printf '{"evidence":"before"}' > "$TASKS/evidence.json"

descriptor_field() { # $1 python expression over d
  "$BIN" --root "$ROOT" descriptor 2>/dev/null | python3 -c "import json,sys;d=json.load(sys.stdin);print($1)" 2>/dev/null
}
wait_ready() { # waits for a ready descriptor at a generation above $1
  for _ in $(seq 1 600); do
    [ "$(descriptor_field "d['state']=='ready' and d['host_generation']>$1")" = True ] && return 0
    sleep 0.1
  done
  echo "TIMED OUT waiting for ready"; return 1
}
status_field() { "$BIN" --root "$ROOT" service status | python3 -c "import json,sys;d=json.load(sys.stdin);print(d$1)"; }

cleanup() {
  echo "--- cleanup"
  "$BIN" --root "$ROOT" service uninstall
  systemctl --user show "$LABEL.service" --property=LoadState,ActiveState
  ls -la "$REG/$LABEL.service" "$REG/default.target.wants/$LABEL.service" 2>&1 | sed 's/^/  /'
  if pgrep -f "$BASE/svc" >/dev/null; then echo "LEFTOVER PROCESSES"; pgrep -af "$BASE/svc"; else echo "no process references $BASE/svc"; fi
  echo "files under ~/.openagents newer than the start of this run:"
  find "$HOME/.openagents" -newer "$BASE/start-marker" \( -path '*/host/*' -o -path '*/tasks/*' -o -path '*/coder-access/*' -o -path '*/host-bundle/*' \) 2>/dev/null | sed 's/^/  /'
  echo "(end)"
}
trap cleanup EXIT

echo "--- install"
"$BIN" --root "$ROOT" service install --label "$LABEL" --bundle-root "$BUNDLES" \
  --state "$TASKS" --host-key "$KEY" --version "$V1" --listen "127.0.0.1:$PORT" \
  --ready-timeout 40 --stop-grace 3 -- \
  host serve --state "$ACCESS" --root "$HROOT" --tasks "$TASKS" --no-runtime \
  --no-telemetry --loopback-test || exit 1
echo "--- rendered unit"
cat "$REG/$LABEL.service"
echo "registration: $(readlink "$REG/$LABEL.service")"
wait_ready 0 || exit 1
echo "--- status after install"
"$BIN" --root "$ROOT" service status
echo "--- the cgroup"
systemctl --user show "$LABEL.service" --property=MainPID,ControlGroup,NRestarts
CG=$(systemctl --user show "$LABEL.service" --property=ControlGroup --value)
ps -o pid=,pgid=,args= -p "$(tr '\n' ',' < "/sys/fs/cgroup$CG/cgroup.procs" | sed 's/,$//')"
echo "listener: $(ss -ltn "sport = :$PORT" | tail -n +2)"

echo "--- update to v2 (a second real build)"
"$BIN" --root "$ROOT" update --to "$V2" --wait 90; echo "exit=$?"
echo "--- update to a build that exits"
"$BIN" --root "$ROOT" update --to "$BAD" --wait 90; echo "exit=$?"
echo "marker after rollback: $(cat "$TASKS/marker" 2>/dev/null || echo absent)"
echo "evidence after rollback: $(cat "$TASKS/evidence.json")"
echo "running version: $(descriptor_field "d['version']==\"$V2\"")"

echo "--- restart"
GEN=$(descriptor_field "d['host_generation']")
"$BIN" --root "$ROOT" service restart
wait_ready "$GEN" && echo "ready again: $(descriptor_field "d['host_generation'], d['version']==\"$V2\"")"

echo "--- SIGKILL of the launcher during a trial"
REQ=$("$BIN" --root "$ROOT" update --to "$SILENT" | python3 -c 'import json,sys;print(json.load(sys.stdin)["request"])')
for _ in $(seq 1 300); do [ "$(descriptor_field "d['state']")" = updating ] && break; sleep 0.1; done
for _ in $(seq 1 300); do [ -f "$TASKS/marker" ] && break; sleep 0.1; done
LPID=$(status_field "['pid']")
TRIAL_GROUP=$(python3 -c "import json;print(json.load(open('$ROOT/launcher.json'))['host_group'])")
echo "trial in progress; SIGKILL to launcher $LPID (trial group $TRIAL_GROUP)"
kill -9 "$LPID"
for _ in $(seq 1 900); do
  [ "$(descriptor_field "d['update']['request']=='$REQ' and d['update']['state']=='rolled-back' and d['state']=='ready'")" = True ] && break
  sleep 0.1
done
"$BIN" --root "$ROOT" descriptor
echo "processes left in trial group $TRIAL_GROUP: $(ps -o pid= -g "$TRIAL_GROUP" | tr -d ' ' | tr '\n' ' ')"
echo "new launcher pid: $(status_field "['pid']"); restarts: $(systemctl --user show "$LABEL.service" --property=NRestarts --value)"
echo "marker after rollback: $(cat "$TASKS/marker" 2>/dev/null || echo absent)"
echo "--- status before uninstall"
"$BIN" --root "$ROOT" service status
