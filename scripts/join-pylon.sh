#!/usr/bin/env bash
# Join with an owned native engine; the learned mode downloads pinned originals.
set -euo pipefail
umask 077
home_dir="${OPENAGENTS_PYLON_HOME:-$HOME/.openagents/pylon}"
source_dir='' relay='wss://relay.openagents.com' slug='' label='' port='' update=true stop=false
model=psionic-toy device=auto model_cache='' self_test=false reported_failure=false
usage() {
  cat <<'HELP'
Usage: join-pylon.sh [--relay wss://HOST] [--slug NAME] [--label TEXT] [--port N]
                     [--model psionic-toy|elephant-1] [--device auto|cuda|metal]
                     [--model-cache DIR] [--self-test] [--source DIR] [--no-update] [--stop]
Default: small toy engine. --model elephant-1 uses the real original 18.8 GB
Elephant checkpoint on CUDA or Metal; no CPU or toy fallback. --self-test sends
an encrypted relay task pinned to YOUR provider, then keeps serving. Keep this
terminal open; Ctrl-C stops both programs. --stop stops the current join.
Default checkout and private state: ~/.openagents/pylon (OPENAGENTS_PYLON_HOME overrides).
Learned state: state/elephant-1; stop it with --model elephant-1 --stop.
Interrupted downloads retain .part files; repeat the same command to resume.
--source uses an existing checkout without updating it, for local development.
HELP
}
failure_hint() {
  if [[ $model != psionic-toy ]]; then printf 'pylon: For an explicit no-model demo, use --model psionic-toy. No fallback was started.\n' >&2; fi
  printf 'pylon: Feedback: https://github.com/OpenAgentsInc/openagents/issues\n' >&2
}
die() { printf 'pylon: %s\n' "$*" >&2; failure_hint; reported_failure=true; exit 1; }
while (($#)); do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --no-update) update=false; shift ;;
    --stop) stop=true; shift ;;
    --self-test) self_test=true; shift ;;
    --relay|--slug|--label|--port|--source|--model|--device|--model-cache)
      (($#>=2)) || die "$1 needs a value"
      case "$1" in --relay) relay=$2 ;; --slug) slug=$2 ;; --label) label=$2 ;; --port) port=$2 ;; --source) source_dir=$2; update=false ;; --model) model=$2 ;; --device) device=$2 ;; --model-cache) model_cache=$2 ;; esac
      shift 2 ;;
    *) die "unknown option $1 (use --help)" ;;
  esac
done
[[ $model == psionic-toy || $model == elephant-1 ]] || die 'supported models: psionic-toy, elephant-1'
[[ $device == auto || $device == cuda || $device == metal ]] || die 'device must be auto, cuda or metal'
state="$home_dir/state"
[[ $model == psionic-toy ]] || state="$state/$model"
if $stop; then
  [[ -d "$state" && ! -L "$state" ]] || die 'no running join state found'
  [[ ! -L "$state/stop" ]] || die 'stop path must not be a symlink'
  : > "$state/stop"
  printf 'pylon: stop requested\n'
  exit 0
fi
for tool in git python3; do command -v "$tool" >/dev/null 2>&1 || die "install $tool first"; done
system="$(uname -s)"
case "$system" in
  Darwin) command -v xcrun >/dev/null 2>&1 || die 'install Xcode Command Line Tools: xcode-select --install'; xcrun --find clang >/dev/null || die 'Xcode Command Line Tools unavailable'; cc="${COAX_CC:-cc}"; cxx="${CXX:-c++}" ;;
  Linux)
    if ! command -v nix >/dev/null 2>&1 && [[ -z ${CXTLS_CFLAGS:-} || -z ${CXTLS_LIBS:-} ]] && [[ -z ${OPENSSL_DEV:-} || -z ${OPENSSL_LIB:-} ]]; then die 'install Nix first (OpenSSL build dependency), or supply an existing Nix OpenSSL closure'; fi
    cc="${COAX_CC:-$(command -v gcc || true)}"; cxx="${CXX:-$(command -v g++ || true)}" ;;
  *) die 'this demo supports macOS and Linux' ;;
esac
backend=''
if [[ $model == elephant-1 ]]; then
  command -v curl >/dev/null 2>&1 || die 'install curl before downloading the learned model'
  if [[ $device == auto ]]; then if [[ $system == Darwin ]]; then device=metal; else device=cuda; fi; fi
  if [[ $device == metal ]]; then
    [[ $system == Darwin && $(uname -m) == arm64 ]] || die 'Metal mode needs an Apple-silicon Mac'
    [[ $(sw_vers -productVersion | cut -d. -f1) -ge 26 && $(xcrun --show-sdk-version | cut -d. -f1) -ge 26 ]] || die 'learned Metal mode needs macOS 26 and the Xcode 26 macOS SDK (Metal 4 tensor kernels)'
    backend=metal
  else
    [[ $system == Linux ]] || die 'CUDA mode is supported on Linux'
    command -v nvidia-smi >/dev/null 2>&1 || die 'CUDA mode needs a working NVIDIA driver and NVRTC 13.2; nvidia-smi is missing'
    nvidia-smi --query-gpu=name,memory.total --format=csv,noheader || die 'the NVIDIA driver is unavailable'
    backend=gpu
  fi
fi
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
if [[ -z $source_dir ]]; then
  # This clone also holds private runtime state. Keep a casual git add . from
  # including that state or downloaded weights; never alter --source repos.
  python3 - "$root" <<'PY'
import os,pathlib,stat,sys
p=pathlib.Path(sys.argv[1])/'.git/info/exclude'
if p.exists() or p.is_symlink():
 s=p.lstat()
 if not stat.S_ISREG(s.st_mode) or s.st_uid!=os.getuid():raise SystemExit('pylon: Git exclude must be an owned regular file')
 text=p.read_text()
else:text=''
for line in ('/state/','/models/','/build/'):
 if line not in text.splitlines():text+='\n'+line+'\n'
p.write_text(text)
PY
fi
[[ -f "$root/psionic/platform/pylon-build.sh" && -f "$root/psionic/fixtures/pylon/$model.selfcheck" ]] || die 'source checkout lacks the selected Pylon model'
[[ ! -L "$home_dir/state" && ! -L "$state" ]] || die 'state path must not be a symlink'
mkdir -p "$state"
chmod 700 "$state"
for name in stop demo-label engine.log pylon.log identity identity.generation identity.generation.lock slug model.json fleet.log self-test.json self-test-request.json; do
  [[ ! -L "$state/$name" ]] || die 'state files must not be symlinks'
done
lock="$state/join.lock"
if ! mkdir "$lock" 2>/dev/null; then die "another join is active (or remove stale $lock after checking its pid file)"; fi
printf '%s\n' "$$" > "$lock/pid"
engine_pid='' pylon_pid='' fleet_pid=''
owned_running() { jobs -pr | grep -qx "$1"; }
stop_owned() {
  local pid="$1" grace="$2" i
  [[ -n "$pid" ]] || return 0
  for ((i=0;i<grace;i++)); do owned_running "$pid" || break; sleep .1; done
  if owned_running "$pid"; then
    kill "$pid" 2>/dev/null || true
    for ((i=0;i<20;i++)); do owned_running "$pid" || break; sleep .1; done
    owned_running "$pid" && kill -KILL "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}
cleanup() {
  local result=$?
  trap - EXIT INT TERM HUP
  [[ -L "$state/stop" ]] || : > "$state/stop"
  stop_owned "$fleet_pid" 0
  stop_owned "$pylon_pid" 450
  stop_owned "$engine_pid" 0
  rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
  if ((result!=0 && result!=130 && result!=143)) && ! $reported_failure; then failure_hint; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
rm -f "$state/stop"
# Build commands have their own process group and a hard 14-minute deadline.
build() {
  local status
  if python3 - "$state/stop" "${PYLON_STAGE:-build}" "$@" <<'PY'
import os,signal,subprocess,sys,time
stop_path,stage=sys.argv[1:3]
if os.path.lexists(stop_path):raise SystemExit(125)
p=subprocess.Popen(sys.argv[3:],start_new_session=True)
class RequestedStop(Exception):pass
def stop(signum,frame): raise KeyboardInterrupt
signal.signal(signal.SIGTERM,stop)
deadline=time.monotonic()+840
try:
    while p.poll() is None:
        if os.path.lexists(stop_path):raise RequestedStop
        remaining=deadline-time.monotonic()
        if remaining<=0:raise TimeoutError
        try:p.wait(timeout=min(.2,remaining))
        except subprocess.TimeoutExpired:pass
    code=p.returncode
except (TimeoutError,KeyboardInterrupt,RequestedStop) as reason:
    try:os.killpg(p.pid,signal.SIGTERM)
    except ProcessLookupError:pass
    try:p.wait(timeout=5)
    except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
    if isinstance(reason,RequestedStop):code=125
    elif isinstance(reason,KeyboardInterrupt):code=130
    else:
        print('pylon: '+stage+' reached its 14-minute bound',file=sys.stderr)
        if stage=='model download/checksum':print('pylon: partial downloads are retained; repeat the same join command to resume',file=sys.stderr)
        code=124
raise SystemExit(code)
PY
  then return 0
  else
    status=$?
    if [[ $status == 125 && -e "$state/stop" ]]; then exit 0; fi
    return "$status"
  fi
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
[[ $model == psionic-toy ]] || out="$out/$model-$backend"
mkdir -p "$out"
# Hash only the loaded Coax graphs and the native build inputs, not unrelated commits.
identity="$(python3 - "$root" "$backend" <<'PY'
import hashlib,json,os,pathlib,subprocess,sys
r=pathlib.Path(sys.argv[1]);h=hashlib.sha256()
inputs=set()
entries=['psionic/lib/pylon/run.coax','psionic/lib/serve-proto.coax','coax/lib/crypto/entropy.coax','psionic/lib/fleet/run.coax']
if sys.argv[2]:
    entries+=['psionic/lib/ops/model-manifest.coax','psionic/lib/device/plan.coax','psionic/lib/device/project.coax','psionic/lib/device/layer.coax','psionic/lib/device/warp.coax']
    if sys.argv[2]=='metal':entries+=['psionic/lib/device/tile.coax']
for entry in entries:
    report=json.loads(subprocess.check_output([str(r/'coax/bin/coax'),'deps',str(r/entry),'--run-steps','250000000']))
    inputs.update(pathlib.Path(p) for p in report['sources'])
for folder in ('psionic/platform','coax/platform'):
    inputs.update(p for p in (r/folder).glob('*') if p.is_file() and p.suffix in ('.c','.h','.sh','.m'))
inputs.update([r/'psionic/tests/elephant_manifest.py',r/'psionic/tests/serve_build_id.py',r/'coax/bin/coax'])
inputs.add(r/'psionic/fixtures/models/mammoth/inventory.h')
for p in sorted(inputs):
    h.update(str(p.relative_to(r)).encode()+b'\0');h.update(p.read_bytes())
h.update(subprocess.check_output([os.environ['COAX_CC'],'--version']))
h.update(os.uname().sysname.encode()+os.uname().machine.encode())
h.update(sys.argv[2].encode())
print(h.hexdigest())
PY
)"
if [[ ! -x "$out/pylon" || ! -x "$out/psionic-serve" || ! -f "$out/source.identity" || "$(cat "$out/source.identity")" != "$identity" ]]; then
  printf 'pylon: building Pylon and %s native engine\n' "$model"
  build "$root/psionic/platform/pylon-build.sh" "$out" plain
  if [[ -n $backend ]]; then build "$root/psionic/platform/serve-build.sh" "$out" release plain "$backend"; else build "$root/psionic/platform/serve-build.sh" "$out" release plain; fi
  printf '%s\n' "$identity" > "$out/source.identity"
fi
if $self_test && [[ ! -x "$out/fleet/psionic-fleet" || ! -f "$out/fleet.identity" || $(cat "$out/fleet.identity") != "$identity" ]]; then
  build "$root/psionic/platform/fleet-build.sh" "$out/fleet" plain
  printf '%s\n' "$identity" > "$out/fleet.identity"
fi
capacity=0
if [[ $model == elephant-1 ]]; then
  printf 'pylon: checking actual GPU kernels and memory plan before any model download\n'
  build "$out/psionic-serve" --check-device --capacity 512 || die 'learned-device preflight failed; inspect the device/compiler error above'
  build "$root/psionic/platform/model-info-build.sh" "$out/model-info"
  "$out/model-info/psionic-model-info" "$model" > "$state/model.json"
  read -r revision capacity < <(python3 - "$state/model.json" <<'PY'
import json,sys
m=json.load(open(sys.argv[1]));print(m['revision'],m['capacity'])
PY
)
  [[ -n $model_cache ]] || model_cache="$home_dir/models/$model-$revision"
  printf 'pylon: pinned originals cache %s (18.8 GB; interrupted downloads resume)\n' "$model_cache"
  PYLON_STAGE='model download/checksum' build python3 "$root/scripts/pylon-fetch-model.py" "$state/model.json" "$model_cache"
fi
if [[ -z "$port" ]]; then
  port="$(python3 - <<'PY'
import socket
s=socket.socket();s.bind(('127.0.0.1',0));print(s.getsockname()[1]);s.close()
PY
)"
fi
[[ "$port" =~ ^[0-9]+$ ]] && ((port>=1024 && port<=65535)) || die 'port must be 1024..65535'
host="$(hostname -s | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9-' '-' | cut -c1-40)"
random_hex() { od -An -N "$1" -tx1 /dev/urandom | tr -d ' \n'; }
if [[ -z $slug ]]; then
  if [[ -f "$state/slug" && ! -L "$state/slug" ]]; then slug=$(cat "$state/slug"); else slug="pylon-${host}-$(random_hex 2)"; fi
fi
[[ -n "$label" ]] || label="${host} (demo)"
[[ "$slug" =~ ^[a-z0-9_-]+$ && ${#slug} -le 64 ]] || die 'slug must be 1..64 lowercase letters, digits, _ or -'
# Fresh on first install, then retained with the provider-bound epoch on restart.
label_file="$state/demo-label"
for file in "$label_file" "$state/stop" "$state/engine.log" "$state/pylon.log" "$state/identity" "$state/identity.generation" "$state/identity.generation.lock" "$state/slug"; do
  [[ ! -L "$file" ]] || die 'state files must not be symlinks'
done
if [[ ! -e $label_file ]]; then random_hex 32 > "$label_file"; fi
python3 - "$label_file" <<'PY'
import os,pathlib,re,stat,sys
p=pathlib.Path(sys.argv[1]);s=p.lstat()
if not stat.S_ISREG(s.st_mode) or s.st_uid!=os.getuid() or not re.fullmatch('[0-9a-f]{64}',p.read_text()):raise SystemExit('pylon: existing demo-label is invalid; do not reset its matching generation state')
PY
chmod 600 "$label_file"
printf '%s\n' "$slug" > "$state/slug"
[[ ! -e "$state/stop" ]] || exit 0
: > "$state/engine.log"; : > "$state/pylon.log"
engine_args=(--toy --port "$port")
if [[ $model == elephant-1 ]]; then
  engine_flag=--elephant-gpu;[[ $backend != metal ]] || engine_flag=--elephant-metal
  engine_args=("$engine_flag" "$model_cache" --table "$root/psionic/fixtures/text/ch05/qwen35.pst" --capacity "$capacity" --port "$port")
fi
"$out/psionic-serve" "${engine_args[@]}" > "$state/engine.log" 2>&1 & engine_pid=$!
printf 'pylon: authenticating/warming %s; no beacon until the native engine and doctor pass\n' "$model"
warmup_status=0
python3 - "$port" "$engine_pid" "$model" "$state/stop" <<'PY' || warmup_status=$?
import os,socket,sys,time
port=int(sys.argv[1]);pid=int(sys.argv[2]);deadline=time.monotonic()+(20 if sys.argv[3]=='psionic-toy' else 840)
while time.monotonic()<deadline:
    if os.path.lexists(sys.argv[4]):raise SystemExit(125)
    try:
        os.kill(pid,0)
        with socket.create_connection(('127.0.0.1',port),timeout=.2):break
    except ProcessLookupError:raise SystemExit('pylon: native engine exited; inspect engine.log')
    except OSError:time.sleep(.1)
else:raise SystemExit('pylon: native engine did not become ready within its bounded warmup; inspect engine.log')
PY
if ((warmup_status)); then [[ ! -e "$state/stop" ]] || exit 0; tail -20 "$state/engine.log" >&2; die 'native engine warmup failed'; fi
[[ ! -e "$state/stop" ]] || exit 0
"$out/pylon" --relay "$relay" --engine "http://127.0.0.1:$port" --model "$model" \
  --slug "$slug" --label "$label" --test-key-label-file "$label_file" \
  --references "$root/psionic/fixtures/pylon/$model.selfcheck" \
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
printf 'Model: %s\nLogs: %s\n' "$model" "$state"
printf 'Stop: Ctrl-C, or touch %q\n' "$state/stop"
printf 'Throwaway identity: keep its demo-label private. This demo does no paid work.\n'
if $self_test; then
  provider=$(sed -nE 's/^pylon: TEST-ONLY key, signed in Coax; pubkey ([0-9a-f]{64})$/\1/p' "$state/pylon.log" | head -1)
  [[ ${#provider} == 64 ]] || die 'could not observe this Pylon public key for its native self-test'
  service_identity=$(cat "$state/identity")
  test_port=$(python3 - <<'PY'
import socket
s=socket.socket();s.bind(('127.0.0.1',0));print(s.getsockname()[1]);s.close()
PY
)
  "$out/fleet/psionic-fleet" --relay "$relay" --identity "$service_identity" --provider "$provider" --port "$test_port" --timeout-ms 15000 --attempt-ms 10000 --discover-ms 1500 > "$state/fleet.log" 2>&1 & fleet_pid=$!
  python3 - "$test_port" "$fleet_pid" <<'PY'
import os,socket,sys,time
for _ in range(200):
 try:
  os.kill(int(sys.argv[2]),0)
  with socket.create_connection(('127.0.0.1',int(sys.argv[1])),timeout=.2):break
 except ProcessLookupError:raise SystemExit('pylon: native self-test buyer exited; inspect fleet.log')
 except OSError:time.sleep(.1)
else:raise SystemExit('pylon: native self-test buyer did not listen')
PY
  printf '{"model":"%s","state":"Water is wet.","questions":{"wet":{"type":"choice","instructions":"Is the water wet?","criteria":{"yes":null,"no":null}}}}' "$model" > "$state/self-test-request.json"
  curl --fail --silent --show-error --max-time 20 --header 'Content-Type: application/json' --data-binary "@$state/self-test-request.json" "http://127.0.0.1:$test_port/v1/systemone" > "$state/self-test.json" || die 'encrypted relay self-test failed; inspect fleet.log and pylon.log'
  python3 - "$state/self-test.json" "$provider" "$service_identity" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]));s=r.get('service',{});a=r.get('answers',{}).get('wet',{})
if s.get('provider')!=sys.argv[2] or s.get('identity')!=sys.argv[3]:raise SystemExit('pylon: self-test did not name this exact provider and checkpoint')
if a.get('choice') not in ('yes','no') or set(a.get('probabilities',{}))!={'yes','no'}:raise SystemExit('pylon: relay response did not contain the requested wet decision')
if r.get('model')=='elephant-1' and a['choice']!='yes':raise SystemExit('pylon: learned relay response disagrees with the wet reference')
PY
  printf 'Self-test passed: %s via %s\n' "$model" "$provider"
  cat "$state/self-test.json"; printf '\n'
  stop_owned "$fleet_pid" 0;fleet_pid=''
fi
while kill -0 "$pylon_pid" 2>/dev/null; do
  if ! kill -0 "$engine_pid" 2>/dev/null; then die 'native engine stopped; inspect engine.log'; fi
  [[ ! -f "$state/stop" ]] || break
  sleep 1
done

if [[ ! -f "$state/stop" ]]; then
  status=0
  wait "$pylon_pid" || status=$?
  pylon_pid=''
  ((status==0)) || die "Pylon stopped with status $status; inspect pylon.log"
fi
