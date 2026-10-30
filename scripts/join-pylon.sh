#!/usr/bin/env bash
# Join the Oct 30 Pylon demo with a local toy engine and a throwaway identity.
set -euo pipefail
umask 077
home_dir="${OPENAGENTS_PYLON_HOME:-$HOME/.openagents/pylon}"
source_dir='' relay='wss://relay.openagents.com' slug='' label='' port='' update=true stop=false
usage() {
  cat <<'HELP'
Usage: join-pylon.sh [--relay wss://HOST] [--slug NAME] [--label TEXT] [--port N]
                     [--source DIR] [--no-update] [--stop]
Joins with a toy decision engine and a fresh throwaway demo identity. Keep this
terminal open; Ctrl-C stops both programs. --stop stops the current join.
Default checkout and private state: ~/.openagents/pylon (OPENAGENTS_PYLON_HOME overrides).
--source uses an existing checkout without updating it, for local development.
HELP
}
die() { printf 'pylon: %s\n' "$*" >&2; exit 1; }
while (($#)); do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --no-update) update=false; shift ;;
    --stop) stop=true; shift ;;
    --relay|--slug|--label|--port|--source)
      (($#>=2)) || die "$1 needs a value"
      case "$1" in --relay) relay=$2 ;; --slug) slug=$2 ;; --label) label=$2 ;; --port) port=$2 ;; --source) source_dir=$2; update=false ;; esac
      shift 2 ;;
    *) die "unknown option $1 (use --help)" ;;
  esac
done
state="$home_dir/state"
if $stop; then
  [[ -d "$state" && ! -L "$state" ]] || die 'no running join state found'
  : > "$state/stop"
  printf 'pylon: stop requested\n'
  exit 0
fi
for tool in git python3; do command -v "$tool" >/dev/null 2>&1 || die "install $tool first"; done
system="$(uname -s)"
case "$system" in
  Darwin) command -v xcrun >/dev/null 2>&1 || die 'install Xcode Command Line Tools: xcode-select --install'; xcrun --find clang >/dev/null || die 'Xcode Command Line Tools unavailable'; cc="${COAX_CC:-$(xcrun --find clang)}"; cxx="${CXX:-$(xcrun --find clang++)}" ;;
  Linux) command -v nix >/dev/null 2>&1 || die 'install Nix first (OpenSSL build dependency)'; cc="${COAX_CC:-$(command -v gcc || true)}"; cxx="${CXX:-$(command -v g++ || true)}" ;;
  *) die 'this demo supports macOS and Linux' ;;
esac
[[ -n "$cc" && -n "$cxx" ]] || die 'install a C and C++ compiler (gcc/g++ or Xcode CLT)'
command -v "$cc" >/dev/null && command -v "$cxx" >/dev/null || die 'C/C++ compiler unavailable'
export COAX_CC="$cc" CC="$cc" CXX="$cxx"
if [[ "$system" == Linux && -z "${SSL_CERT_FILE:-}" ]]; then
  for ca in /etc/ssl/certs/ca-certificates.crt /etc/ssl/cert.pem /etc/pki/tls/certs/ca-bundle.crt; do
    if [[ -f "$ca" ]]; then export SSL_CERT_FILE="$ca"; break; fi
  done
fi
mkdir -p "$(dirname "$home_dir")"
[[ ! -L "$home_dir" ]] || die 'checkout path must not be a symlink'
if [[ -n "$source_dir" ]]; then
  root="$(cd "$source_dir" && pwd -P)"
else
  if [[ ! -e "$home_dir" ]]; then
    printf 'pylon: downloading source\n'
    git clone --depth 1 --filter=blob:none --sparse --branch main --single-branch https://github.com/OpenAgentsInc/openagents.git "$home_dir"
    git -C "$home_dir" sparse-checkout set coax psionic scripts proto
  fi
  [[ -d "$home_dir/.git" ]] || die 'existing directory is not a standalone checkout; choose OPENAGENTS_PYLON_HOME'
  root="$(cd "$home_dir" && pwd -P)"
  if $update; then
    [[ -z "$(git -C "$root" status --porcelain --untracked-files=no)" ]] || die 'checkout has local edits; keep them and use --no-update'
    [[ "$(git -C "$root" symbolic-ref --short HEAD)" == main ]] || die 'checkout is not on main; use --no-update deliberately'
    git -C "$root" fetch origin main
    git -C "$root" merge --ff-only origin/main
    [[ "$(git -C "$root" rev-parse HEAD)" == "$(git -C "$root" rev-parse origin/main)" ]] || die 'local main differs from remote main'
  fi
fi
[[ -f "$root/psionic/platform/pylon-build.sh" && -f "$root/psionic/fixtures/pylon/psionic-toy.selfcheck" ]] || die 'source checkout lacks the Pylon demo'
mkdir -p "$state"
[[ ! -L "$state" ]] || die 'state path must not be a symlink'
chmod 700 "$state"
lock="$state/join.lock"
if ! mkdir "$lock" 2>/dev/null; then die "another join is active (or remove stale $lock after checking its pid file)"; fi
printf '%s\n' "$$" > "$lock/pid"
engine_pid='' pylon_pid=''
cleanup() {
  trap - EXIT INT TERM HUP
  : > "$state/stop"
  [[ -z "$pylon_pid" ]] || kill "$pylon_pid" 2>/dev/null || true
  [[ -z "$engine_pid" ]] || kill "$engine_pid" 2>/dev/null || true
  [[ -z "$pylon_pid" ]] || wait "$pylon_pid" 2>/dev/null || true
  [[ -z "$engine_pid" ]] || wait "$engine_pid" 2>/dev/null || true
  rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
# Build commands have their own process group and a hard 14-minute deadline.
build() {
  python3 - "$@" <<'PY'
import os,signal,subprocess,sys
p=subprocess.Popen(sys.argv[1:],start_new_session=True)
def stop(signum,frame): raise KeyboardInterrupt
signal.signal(signal.SIGTERM,stop)
try: code=p.wait(timeout=840)
except (subprocess.TimeoutExpired,KeyboardInterrupt):
    os.killpg(p.pid,signal.SIGTERM)
    try:p.wait(timeout=5)
    except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
    print('pylon: build stopped (interrupted or exceeded 14 minutes)',file=sys.stderr);code=124
raise SystemExit(code)
PY
}
cd "$root"
if ! "$root/coax/provenance.sh" verify "$root" "$root/coax/bin/coax" "$root/coax/bin/provenance-check" "$root/coax/bin/provenance.record" >/dev/null 2>&1; then
  fingerprint="$("$root/coax/provenance.sh" fingerprint "$root")"
  cache="$HOME/.cache/coax-release/$fingerprint"
  if [[ -f "$cache/provenance.record" ]]; then
    "$root/coax/provenance.sh" install "$root" "$cache"
  else
    printf 'pylon: building the checked Coax compiler (several minutes)\n'
    export COAX_MUTATE_JOBS="${COAX_MUTATE_JOBS:-4}"
    build "$root/coax/build.sh"
  fi
fi
out="$home_dir/build"
mkdir -p "$out"
# Hash only the loaded Coax graphs and the native build inputs, not unrelated commits.
identity="$(python3 - "$root" <<'PY'
import hashlib,json,os,pathlib,subprocess,sys
r=pathlib.Path(sys.argv[1]);h=hashlib.sha256()
inputs=set()
for entry in ('psionic/lib/pylon/run.coax','psionic/lib/serve-proto.coax','coax/lib/crypto/entropy.coax'):
    report=json.loads(subprocess.check_output([str(r/'coax/bin/coax'),'deps',str(r/entry)]))
    inputs.update(pathlib.Path(p) for p in report['sources'])
for folder in ('psionic/platform','coax/platform'):
    inputs.update(p for p in (r/folder).glob('*') if p.is_file() and p.suffix in ('.c','.h','.sh'))
inputs.update([r/'psionic/tests/elephant_manifest.py',r/'psionic/tests/serve_build_id.py',r/'coax/bin/coax'])
for p in sorted(inputs):
    h.update(str(p.relative_to(r)).encode()+b'\0');h.update(p.read_bytes())
h.update(subprocess.check_output([os.environ['COAX_CC'],'--version']))
h.update(os.uname().sysname.encode()+os.uname().machine.encode())
print(h.hexdigest())
PY
)"
if [[ ! -x "$out/pylon" || ! -x "$out/psionic-serve" || ! -f "$out/source.identity" || "$(cat "$out/source.identity")" != "$identity" ]]; then
  printf 'pylon: building Pylon and toy engine\n'
  build "$root/psionic/platform/pylon-build.sh" "$out" plain
  build "$root/psionic/platform/serve-build.sh" "$out" release plain
  printf '%s\n' "$identity" > "$out/source.identity"
fi
if [[ -z "$port" ]]; then
  port="$(python3 - <<'PY'
import socket
s=socket.socket();s.bind(('127.0.0.1',0));print(s.getsockname()[1]);s.close()
PY
)"
fi
[[ "$port" =~ ^[0-9]+$ ]] && ((port>=1024 && port<=65535)) || die 'port must be 1024..65535'
host="$(hostname -s | tr -c 'A-Za-z0-9-' '-' | cut -c1-40)"
random_hex() { od -An -N "$1" -tx1 /dev/urandom | tr -d ' \n'; }
[[ -n "$slug" ]] || slug="pylon-${host}-$(random_hex 2)"
[[ -n "$label" ]] || label="${host} (demo)"
[[ "$slug" =~ ^[a-zA-Z0-9_-]+$ && ${#slug} -le 64 ]] || die 'slug must be 1..64 letters, digits, _ or -'
# This is a fresh demo label every join, never derived from an existing identity.
label_file="$state/demo-label"
[[ ! -L "$label_file" && ! -L "$state/stop" ]] || die 'state files must not be symlinks'
random_hex 32 > "$label_file"
chmod 600 "$label_file"
rm -f "$state/stop"
: > "$state/engine.log"; : > "$state/pylon.log"
"$out/psionic-serve" --toy --port "$port" > "$state/engine.log" 2>&1 & engine_pid=$!
python3 - "$port" "$engine_pid" <<'PY'
import os,socket,sys,time
port=int(sys.argv[1]);pid=int(sys.argv[2])
for _ in range(200):
    try:
        os.kill(pid,0)
        with socket.create_connection(('127.0.0.1',port),timeout=.2):break
    except ProcessLookupError:raise SystemExit('pylon: toy engine exited; inspect engine.log')
    except OSError:time.sleep(.1)
else:raise SystemExit('pylon: toy engine did not start within 20 seconds')
PY
"$out/pylon" --relay "$relay" --engine "http://127.0.0.1:$port" --model psionic-toy \
  --slug "$slug" --label "$label" --test-key-label-file "$label_file" \
  --references "$root/psionic/fixtures/pylon/psionic-toy.selfcheck" \
  --identity-state "$state/identity" --stop-file "$state/stop" --decide-ms 30000 \
  > "$state/pylon.log" 2>&1 & pylon_pid=$!
printf 'pylon: connecting %s to %s\n' "$slug" "$relay"
ready=false
for ((i=0;i<900;i++)); do
  if grep -q 'pylon: online, waiting for jobs' "$state/pylon.log"; then ready=true; break; fi
  kill -0 "$pylon_pid" 2>/dev/null || { tail -20 "$state/pylon.log" >&2; die 'Pylon exited before it became ready'; }
  [[ ! -f "$state/stop" ]] || exit 0
  sleep .1
done
$ready || { tail -20 "$state/pylon.log" >&2; die 'Pylon did not become online within 90 seconds'; }
printf '\nPylon online: %s\n' "$slug"
grep -E '^pylon: (TEST-ONLY key|npub )' "$state/pylon.log" || true
printf 'Model: psionic-toy (demo)\nLogs: %s\n' "$state"
printf 'Stop: Ctrl-C, or touch %q\n' "$state/stop"
printf 'Throwaway identity: keep its demo-label private. This demo does no paid work.\n'
while kill -0 "$pylon_pid" 2>/dev/null; do
  if ! kill -0 "$engine_pid" 2>/dev/null; then die 'toy engine stopped; inspect engine.log'; fi
  [[ ! -f "$state/stop" ]] || break
  sleep 1
done

if [[ ! -f "$state/stop" ]]; then
  status=0
  wait "$pylon_pid" || status=$?
  pylon_pid=''
  ((status==0)) || die "Pylon stopped with status $status; inspect pylon.log"
fi
