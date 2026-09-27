# Coder SSH host script, openagents.coder.ssh-script.v1.
#
# The client sends this fixed script to `sh -s` on standard input and names
# the operation in the arguments. It never interprets text from the client
# as shell code. Every result line starts with "oa-ssh " and holds
# key=value fields; any other output is diagnostic.
#
#   up LOCK_WAIT UPLOAD RUNNER ENTRY... -- SERVE_ARG...
#   invite LOCK_WAIT SHA -- INVITE_ARG...
#   remove LOCK_WAIT
#
# ENTRY is OS/ARCH/SHA256 for one pinned release archive. UPLOAD is "-" or
# the name of an archive the client uploaded to the uploads directory.
# RUNNER is the client's digest of the serve arguments.

set -u
umask 077

oa_root="$HOME/.openagents/ssh-host"
oa_versions="$oa_root/versions"
oa_uploads="$oa_root/uploads"
oa_lock="$oa_root/lock"
oa_managed="$oa_root/managed"
oa_log="$oa_root/host.log"
oa_hostdir="$HOME/.openagents/host"
oa_runtime="$oa_hostdir/runtime"
oa_held=no
oa_scratch=
oa_sha_tool=

say() { printf 'oa-ssh %s\n' "$*"; }

finish() {
  if [ -n "$oa_scratch" ]; then rm -rf "$oa_scratch"; oa_scratch=; fi
  if [ "$oa_held" = yes ]; then
    oa_held=no
    if [ "$(cat "$oa_lock/owner" 2>/dev/null)" = "$$ $oa_node" ]; then
      rm -rf "$oa_lock"
    fi
  fi
}

fail() { say "error=$1"; finish; exit "$2"; }

trap finish EXIT
trap 'exit 130' HUP INT TERM

pause() { sleep 0.1 2>/dev/null || sleep 1; }

oa_node=$(uname -n 2>/dev/null | tr -cd 'A-Za-z0-9._-' | cut -c1-64)

private_dir() {
  if [ -L "$1" ]; then fail unsafe-layout 17; fi
  if [ ! -d "$1" ]; then mkdir -p "$1" || fail unsafe-layout 17; fi
  chmod 700 "$1" || fail unsafe-layout 17
}

tools() {
  for oa_tool in sha256sum shasum openssl; do
    if command -v "$oa_tool" >/dev/null 2>&1; then oa_sha_tool=$oa_tool; break; fi
  done
  if [ -z "$oa_sha_tool" ]; then fail missing-sha256-tool 16; fi
  for oa_tool in tar gzip; do
    if ! command -v "$oa_tool" >/dev/null 2>&1; then fail "missing-$oa_tool" 16; fi
  done
}

sha256_of() {
  case "$oa_sha_tool" in
    sha256sum) sha256sum "$1" | cut -d ' ' -f 1 ;;
    shasum) shasum -a 256 "$1" | cut -d ' ' -f 1 ;;
    openssl) openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1 ;;
  esac
}

detect() {
  oa_os_raw=$(uname -s 2>/dev/null | tr -cd 'A-Za-z0-9._-' | cut -c1-32)
  oa_arch_raw=$(uname -m 2>/dev/null | tr -cd 'A-Za-z0-9._-' | cut -c1-32)
  case "$oa_os_raw" in
    Linux) oa_os=linux ;;
    Darwin) oa_os=macos ;;
    *) oa_os= ;;
  esac
  case "$oa_arch_raw" in
    x86_64|amd64) oa_arch=x86_64 ;;
    aarch64|arm64) oa_arch=aarch64 ;;
    *) oa_arch= ;;
  esac
  if [ -z "$oa_os" ] || [ -z "$oa_arch" ]; then
    say "unsupported_os=${oa_os_raw:-none} unsupported_arch=${oa_arch_raw:-none}"
    fail unsupported-platform 11
  fi
  say "os=$oa_os arch=$oa_arch"
}

# Takes the installation lock. A lock whose recorded owner process no longer
# exists on this machine is moved aside and reclaimed; a live owner is waited
# for up to LOCK_WAIT seconds.
acquire() {
  private_dir "$oa_root"
  oa_tries=0
  oa_blank=0
  while ! mkdir "$oa_lock" 2>/dev/null; do
    oa_owner=$(cat "$oa_lock/owner" 2>/dev/null || true)
    oa_dead=no
    case "$oa_owner" in
      '')
        oa_blank=$((oa_blank + 1))
        if [ "$oa_blank" -ge 20 ]; then oa_dead=yes; fi
        ;;
      *)
        oa_blank=0
        oa_owner_pid=${oa_owner%% *}
        oa_owner_node=${oa_owner#* }
        case "$oa_owner_pid" in
          ''|*[!0-9]*) oa_dead=yes ;;
          *)
            if [ "$oa_owner_node" = "$oa_node" ] && ! kill -0 "$oa_owner_pid" 2>/dev/null; then
              oa_dead=yes
            fi
            ;;
        esac
        ;;
    esac
    if [ "$oa_dead" = yes ]; then
      oa_stale="$oa_root/lock.stale.$$"
      rm -rf "$oa_stale"
      if mv "$oa_lock" "$oa_stale" 2>/dev/null; then
        if [ "$(cat "$oa_stale/owner" 2>/dev/null || true)" = "$oa_owner" ]; then
          rm -rf "$oa_stale"
          oa_blank=0
          say "lock=reclaimed"
          continue
        fi
        # A live owner replaced the lock between the check and the move.
        mv "$oa_stale" "$oa_lock" 2>/dev/null || rm -rf "$oa_stale"
      fi
    fi
    oa_tries=$((oa_tries + 1))
    if [ "$oa_tries" -ge $((oa_wait * 10)) ]; then fail busy 13; fi
    pause
  done
  oa_held=yes
  printf '%s %s\n' "$$" "$oa_node" > "$oa_lock/owner" || fail unsafe-layout 17
}

# Runs a command for at most SECONDS with its output in FILE and no input.
bounded() {
  oa_limit=$1
  oa_out=$2
  shift 2
  "$@" </dev/null >"$oa_out" 2>&1 &
  oa_child=$!
  ( sleep "$oa_limit"; kill -9 "$oa_child" ) </dev/null >/dev/null 2>&1 &
  oa_watch=$!
  wait "$oa_child"
  oa_status=$?
  kill "$oa_watch" 2>/dev/null
  wait "$oa_watch" 2>/dev/null
  return "$oa_status"
}

installed() {
  oa_v="$oa_versions/$1"
  if [ -L "$oa_v" ] || [ ! -d "$oa_v" ] || [ -L "$oa_v/coder" ] || [ ! -f "$oa_v/coder" ] || [ ! -f "$oa_v/manifest" ]; then
    return 1
  fi
  oa_want=$(sed -n 's/^binary_sha256=\([0-9a-f]\{64\}\)$/\1/p' "$oa_v/manifest" | head -n 1)
  oa_from=$(sed -n 's/^archive_sha256=\([0-9a-f]\{64\}\)$/\1/p' "$oa_v/manifest" | head -n 1)
  [ -n "$oa_want" ] && [ "$oa_from" = "$1" ] && [ "$(sha256_of "$oa_v/coder")" = "$oa_want" ]
}

# Verifies an uploaded archive, extracts it into staging, proves that its
# binary runs, and then moves it into place.
install() {
  oa_sha=$1
  oa_upload=$2
  private_dir "$oa_versions"
  private_dir "$oa_uploads"
  find "$oa_uploads" -type f -mtime +0 -exec rm -f {} \; 2>/dev/null
  if [ "$oa_upload" = - ] || [ -L "$oa_uploads/$oa_upload" ] || [ ! -f "$oa_uploads/$oa_upload" ]; then
    fail missing-upload 17
  fi
  oa_scratch="$oa_versions/.staging.$$"
  rm -rf "$oa_scratch"
  mkdir "$oa_scratch" || fail unsafe-layout 17
  mv -f "$oa_uploads/$oa_upload" "$oa_scratch/archive" || fail missing-upload 17
  if [ "$(sha256_of "$oa_scratch/archive")" != "$oa_sha" ]; then
    fail checksum-mismatch 12
  fi
  mkdir "$oa_scratch/tree" || fail unsafe-layout 17
  if ! (cd "$oa_scratch/tree" && gzip -dc ../archive | tar -xf -) >/dev/null 2>&1; then
    fail bad-archive 14
  fi
  if [ -L "$oa_scratch/tree/coder" ] || [ ! -f "$oa_scratch/tree/coder" ]; then
    fail bad-archive 14
  fi
  chmod 700 "$oa_scratch/tree/coder" || fail bad-archive 14
  if ! bounded 10 "$oa_scratch/probe" "$oa_scratch/tree/coder" --version; then
    fail binary-rejected 14
  fi
  oa_bin_sha=$(sha256_of "$oa_scratch/tree/coder")
  printf 'schema=openagents.coder.ssh-bundle.v1\narchive_sha256=%s\nbinary_sha256=%s\n' \
    "$oa_sha" "$oa_bin_sha" > "$oa_scratch/tree/manifest" || fail unsafe-layout 17
  if [ -e "$oa_versions/$oa_sha" ] || [ -L "$oa_versions/$oa_sha" ]; then
    # A retained version that failed verification is kept for inspection.
    mv "$oa_versions/$oa_sha" "$oa_versions/.rejected.$oa_sha.$$" || fail unsafe-layout 17
  fi
  mv "$oa_scratch/tree" "$oa_versions/$oa_sha" || fail unsafe-layout 17
  rm -rf "$oa_scratch"
  oa_scratch=
}

read_runtime() {
  oa_rt_pid=
  oa_rt_port=
  if [ -L "$oa_runtime" ] || [ ! -f "$oa_runtime" ]; then return 1; fi
  if [ "$(sed -n 's/^schema=//p' "$oa_runtime" | head -n 1)" != openagents.coder.host-runtime.v1 ]; then
    return 1
  fi
  oa_rt_pid=$(sed -n 's/^pid=\([1-9][0-9]\{0,9\}\)$/\1/p' "$oa_runtime" | head -n 1)
  oa_rt_port=$(sed -n 's/^port=\([1-9][0-9]\{0,4\}\)$/\1/p' "$oa_runtime" | head -n 1)
  [ -n "$oa_rt_pid" ] && [ -n "$oa_rt_port" ] && [ "$oa_rt_port" -le 65535 ]
}

read_managed() {
  oa_m_pid=
  oa_m_runner=
  oa_m_version=
  if [ -L "$oa_managed" ] || [ ! -f "$oa_managed" ]; then return 1; fi
  if [ "$(sed -n 's/^schema=//p' "$oa_managed" | head -n 1)" != openagents.coder.ssh-managed.v1 ]; then
    return 1
  fi
  oa_m_pid=$(sed -n 's/^pid=\([1-9][0-9]\{0,9\}\)$/\1/p' "$oa_managed" | head -n 1)
  oa_m_runner=$(sed -n 's/^runner=\([0-9a-f]\{64\}\)$/\1/p' "$oa_managed" | head -n 1)
  oa_m_version=$(sed -n 's/^version=\([0-9a-f]\{64\}\)$/\1/p' "$oa_managed" | head -n 1)
  [ -n "$oa_m_pid" ] && [ -n "$oa_m_runner" ] && [ -n "$oa_m_version" ]
}

stop_pid() {
  kill -TERM "$1" 2>/dev/null || return 0
  oa_n=0
  while kill -0 "$1" 2>/dev/null; do
    if [ "$oa_n" -ge 100 ]; then kill -KILL "$1" 2>/dev/null; break; fi
    pause
    oa_n=$((oa_n + 1))
  done
}

# Starts a managed host bound to loopback. The ownership record is written
# before the host reports ready, so a client that dies during the wait still
# leaves the host marked as launcher-owned.
launch() {
  oa_bin=$1
  oa_runner=$2
  oa_sha=$3
  shift 3
  private_dir "$oa_hostdir"
  rm -f "$oa_runtime"
  if command -v setsid >/dev/null 2>&1; then
    setsid nohup "$oa_bin" "$@" </dev/null >>"$oa_log" 2>&1 &
    oa_pid=$!
  else
    set -m 2>/dev/null
    nohup "$oa_bin" "$@" </dev/null >>"$oa_log" 2>&1 &
    oa_pid=$!
    set +m 2>/dev/null
  fi
  printf 'schema=openagents.coder.ssh-managed.v1\npid=%s\nrunner=%s\nversion=%s\n' \
    "$oa_pid" "$oa_runner" "$oa_sha" > "$oa_managed.pending" || fail unsafe-layout 17
  mv -f "$oa_managed.pending" "$oa_managed" || fail unsafe-layout 17
  oa_n=0
  while :; do
    if read_runtime && [ "$oa_rt_pid" = "$oa_pid" ]; then return 0; fi
    if ! kill -0 "$oa_pid" 2>/dev/null || [ "$oa_n" -ge 150 ]; then
      stop_pid "$oa_pid"
      rm -f "$oa_managed"
      fail host-did-not-start 15
    fi
    pause
    oa_n=$((oa_n + 1))
  done
}

# Whether the managed record names a live process that still runs the
# recorded version's binary. A process identifier alone can be reused by an
# unrelated process after the host exits, and that process is never stopped.
managed_live() {
  read_managed || return 1
  kill -0 "$oa_m_pid" 2>/dev/null || return 1
  if command -v ps >/dev/null 2>&1; then
    oa_args=$(ps -o args= -p "$oa_m_pid" 2>/dev/null) || return 1
    case "$oa_args" in
      *"$oa_versions/$oa_m_version/coder"*) return 0 ;;
      *) return 1 ;;
    esac
  fi
  return 0
}

# Reuses or relaunches a managed host, adopts a running host as external,
# or starts a new managed host.
start() {
  oa_bin=$1
  oa_runner=$2
  oa_sha=$3
  shift 3
  oa_started=started
  if managed_live; then
    if read_runtime && [ "$oa_rt_pid" = "$oa_m_pid" ] && [ "$oa_m_runner" = "$oa_runner" ] && [ "$oa_m_version" = "$oa_sha" ]; then
      say "host=reused ownership=managed pid=$oa_rt_pid port=$oa_rt_port"
      return 0
    fi
    stop_pid "$oa_m_pid"
    oa_started=relaunched
  fi
  rm -f "$oa_managed"
  if read_runtime && kill -0 "$oa_rt_pid" 2>/dev/null && [ "$oa_rt_pid" != "${oa_m_pid:-}" ]; then
    say "host=adopted ownership=external pid=$oa_rt_pid port=$oa_rt_port"
    return 0
  fi
  launch "$oa_bin" "$oa_runner" "$oa_sha" "$@"
  say "host=$oa_started ownership=managed pid=$oa_rt_pid port=$oa_rt_port"
}

op_up() {
  oa_upload=$1
  oa_runner=$2
  shift 2
  detect
  tools
  oa_pick=
  while [ "$#" -gt 0 ] && [ "$1" != -- ]; do
    case "$1" in
      "$oa_os/$oa_arch/"*) oa_pick=${1##*/} ;;
    esac
    shift
  done
  if [ "$#" -gt 0 ]; then shift; fi
  case "$oa_pick" in
    ''|*[!0-9a-f]*) fail no-artifact 18 ;;
  esac
  acquire
  if installed "$oa_pick"; then
    if [ "$oa_upload" != - ]; then rm -f "$oa_uploads/$oa_upload"; fi
    say "install=reused sha=$oa_pick"
  elif [ "$oa_upload" = - ]; then
    say "need=$oa_pick"
    finish
    exit 10
  else
    install "$oa_pick" "$oa_upload"
    say "install=fresh sha=$oa_pick"
  fi
  start "$oa_versions/$oa_pick/coder" "$oa_runner" "$oa_pick" "$@"
}

op_invite() {
  oa_sha=$1
  shift
  if [ "${1:-}" = -- ]; then shift; fi
  tools
  acquire
  if ! installed "$oa_sha"; then fail not-installed 19; fi
  oa_scratch="$oa_root/.invite.$$"
  rm -rf "$oa_scratch"
  mkdir "$oa_scratch" || fail unsafe-layout 17
  if ! bounded 30 "$oa_scratch/out" "$oa_versions/$oa_sha/coder" "$@"; then
    fail invite-failed 20
  fi
  if [ "$(wc -l < "$oa_scratch/out" | tr -d ' ')" != 1 ] || [ "$(wc -c < "$oa_scratch/out" | tr -d ' ')" -gt 4096 ]; then
    fail invite-malformed 20
  fi
  oa_invitation=$(cat "$oa_scratch/out")
  if [ -z "$oa_invitation" ] || [ -n "$(printf '%s' "$oa_invitation" | tr -d 'A-Za-z0-9._:/+=?&%@~-')" ]; then
    fail invite-malformed 20
  fi
  rm -rf "$oa_scratch"
  oa_scratch=
  say "invitation=$oa_invitation"
}

op_remove() {
  acquire
  if managed_live; then
    stop_pid "$oa_m_pid"
    if read_runtime && [ "$oa_rt_pid" = "$oa_m_pid" ]; then rm -f "$oa_runtime"; fi
    rm -f "$oa_managed"
    say "host=stopped ownership=managed pid=$oa_m_pid"
    return 0
  fi
  rm -f "$oa_managed"
  if read_runtime && kill -0 "$oa_rt_pid" 2>/dev/null; then
    if [ "$oa_rt_pid" != "${oa_m_pid:-}" ]; then
      say "host=detached ownership=external pid=$oa_rt_pid"
      return 0
    fi
    # Both records name a process that no longer runs the host.
    rm -f "$oa_runtime"
  fi
  say "host=absent"
}

oa_op=${1:-}
oa_wait=${2:-}
case "$oa_wait" in
  ''|*[!0-9]*) fail usage 17 ;;
esac
if [ "$oa_wait" -lt 1 ]; then oa_wait=1; fi
shift 2
case "$oa_op" in
  up) if [ "$#" -lt 2 ]; then fail usage 17; fi; op_up "$@" ;;
  invite) if [ "$#" -lt 1 ]; then fail usage 17; fi; op_invite "$@" ;;
  remove) op_remove ;;
  *) fail usage 17 ;;
esac
