#!/usr/bin/env bash
# Firm reason: 5. One job on a remote build host (docs/metaverse/remote-builds.md):
# git, flock, coax/build.sh, the command, find and tar. remote.sh sends this
# file with every job, so the host always runs the caller's
# version of it.
#
#   bash ~/inbox/JOB/host-job.sh run JOB   # ~/inbox/JOB: spec (bash, from remote.sh), bundle and this file
#
# Exit status: the command's, or 97 when the host could not run it (remote.sh
# then runs the command on the Mac's own slots), or 96 when every slot is busy
# and OA_REMOTE_NO_WAIT=1 (remote.sh then tries another host).
set -uo pipefail
cmd="${1:-}"
slots="${OA_REMOTE_SLOTS:-3}"
[[ "$cmd" == run ]] || { echo "host-job.sh: run JOB" >&2; exit 97; }
job="$2"; in=~/inbox/$job
infra() { echo "remote: $*" >&2; rm -rf "$in"; exit 97; }
[[ -f "$in/spec" ]] || infra "no job $job on $(hostname)"
# shellcheck disable=SC1091
source ~/.nix-profile/etc/profile.d/nix.sh 2> /dev/null || true
export PATH="$HOME/.bun/bin:$PATH"
# The toolchain's nixpkgs, pinned (#13256): the caller's coax/remote/nixpkgs.rev.
# Every `nix shell nixpkgs#...` on this host (here, in check.sh at any commit,
# in toolchain.sh and tls-env.sh) resolves through the user's flake registry,
# so pinning `nixpkgs` there holds them all to that commit. Unpinned, nixpkgs
# followed nixpkgs-unstable: on 2026-11-11 between 18:08 and 19:17 Chicago
# time every host fetched a new revision whose GCC 16.2.0 wrapper had a new
# store path, every slot's compiler record (which names that path) stopped
# verifying, and the pipeline blamed five innocent commits for it.
pin="$(grep -m1 -oE '^[0-9a-f]{40}$' "$in/nixpkgs.rev" 2> /dev/null)"
if [[ -n "$pin" ]] && command -v nix > /dev/null 2>&1 && ! grep -q "$pin" ~/.config/nix/registry.json 2> /dev/null; then
  ( flock 9
    grep -q "$pin" ~/.config/nix/registry.json 2> /dev/null || nix registry add nixpkgs "github:NixOS/nixpkgs/$pin"
  ) 9> ~/.oa-tools.lock || infra "could not pin nixpkgs to $pin on $(hostname)"
fi
# What the CLI's build needs (the web build builds the CLI) and a host set up
# before #13117 lacks: xxd (the Parakeet build embeds its shader source with
# it) and the system's OpenSSL headers (without them tls-env.sh takes Nix's
# OpenSSL, which doesn't link with Debian's gcc and glibc). Once a host;
# host-startup.sh installs both on a new one.
if ! command -v xxd > /dev/null 2>&1 || [[ ! -f /usr/include/openssl/ssl.h ]]; then
  ( flock 9
    command -v xxd > /dev/null 2>&1 && [[ -f /usr/include/openssl/ssl.h ]] ||
      { sudo -n apt-get update -q; sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -yq xxd libssl-dev; } > /dev/null 2>&1
  ) 9> ~/.oa-tools.lock || true
fi
# What coax/web/api/test.sh needs (#13323) and a host set up before it lacks:
# clang, the API's libraries and PGDG's PostgreSQL 16 with no cluster of its
# own (host-startup.sh installs the same on a new host). Once a host.
api_ready() { [[ -x /usr/lib/postgresql/16/bin/initdb && -f /usr/include/secp256k1_schnorrsig.h && -f /usr/include/postgresql/libpq-fe.h && -f /usr/include/zlib.h ]] && command -v clang > /dev/null 2>&1 && ls /usr/include/*/curl/curl.h > /dev/null 2>&1; }
if ! api_ready; then
  ( flock 9
    api_ready || {
      sudo -n install -d /usr/share/postgresql-common/pgdg /etc/postgresql-common/createcluster.d
      sudo -n curl -fsSL -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc https://www.postgresql.org/media/keys/ACCC4CF8.asc
      echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" | sudo -n tee /etc/apt/sources.list.d/pgdg.list
      echo "create_main_cluster = false" | sudo -n tee /etc/postgresql-common/createcluster.d/oa-no-main.conf
      sudo -n apt-get update -q
      sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -yq clang libsecp256k1-dev libpq-dev libcurl4-openssl-dev zlib1g-dev postgresql-16
    } > /dev/null 2>&1
  ) 9> ~/.oa-tools.lock || true
fi
# spec sets: key commit relcwd argv=(...) env_vars=(...), and shared_token: a
# token for the shared build cache's bucket alone (shared-cache.sh), never part
# of the key
# shellcheck disable=SC1090
source "$in/spec"
# The caller's COAX_* and COAXIAL_* settings hold for the compiler build too
# (COAX_MUTATION=0 skips its mutation gates, as release.sh does).
for v in ${env_vars[@]+"${env_vars[@]}"}; do export "${v?}"; done
[[ -z "${shared_token:-}" ]] || export COAX_SHARED_CACHE_TOKEN="$shared_token"
# The token lasts an hour: remote.sh writes a new one into this file every 15
# minutes while the job runs, and shared-cache.sh reads the file first.
if [[ -n "${shared_token:-}" ]]; then
  mkdir -p ~/tokens && find ~/tokens -type f -mmin +180 -delete 2> /dev/null
  export COAX_SHARED_CACHE_TOKEN_FILE=~/tokens/$job
  [[ -s "$COAX_SHARED_CACHE_TOKEN_FILE" ]] || (umask 077; printf '%s\n' "$shared_token" > "$COAX_SHARED_CACHE_TOKEN_FILE")
  trap 'rm -f "$COAX_SHARED_CACHE_TOKEN_FILE"' EXIT
fi
touch ~/busy
# Results of a passing job are kept under its key (the source tree's hash,
# the command, its directory and the forwarded environment): the same job
# again replays its output and files without running.
res=~/results/$key
if [[ -f "$res/status" && "$(< "$res/status")" == 0 ]]; then
  touch "$res"
  cat "$res/out"; cat "$res/err" >&2
  echo "remote: $(hostname): the same sources and command passed before; replayed its output and files" >&2
  rm -rf "$in"; exit 0
fi
find ~/results -mindepth 1 -maxdepth 1 -mtime +3 -exec rm -rf {} + 2> /dev/null
# A slot: one of OA_REMOTE_SLOTS worktrees, held with flock for the whole job.
slot=""; waited=0
while [[ -z "$slot" ]]; do
  for s in $(seq 1 "$slots"); do
    mkdir -p ~/slots/$s
    exec 9> ~/slots/$s/lock
    if flock -n 9; then slot=$s; break; fi
    exec 9>&-
  done
  if [[ -z "$slot" ]]; then
    # OA_REMOTE_NO_WAIT=1: the caller tries another host first (exit 96).
    if [[ "${OA_REMOTE_NO_WAIT:-0}" == 1 ]]; then rm -rf "$in"; exit 96; fi
    [[ $waited == 1 ]] || echo "remote: $(hostname): all $slots slots busy, waiting" >&2
    waited=1; sleep 5
  fi
done
tree=~/slots/$slot/tree
began=$(date +%s)
# The caller's shared-cache.sh, for wasm.sh and pages.sh in this job (the
# tree's own copy may be older, or missing at an old commit).
if [[ -f "$in/shared-cache.sh" ]]; then
  cp "$in/shared-cache.sh" ~/slots/$slot/shared-cache.sh && chmod +x ~/slots/$slot/shared-cache.sh &&
    export COAX_SHARED_CACHE_TOOL=~/slots/$slot/shared-cache.sh
fi
# The shared clone: fetch main and the job's bundle, and check the job's
# commit out in this slot's worktree, one slot at a time.
(
  flock 8
  cd ~/oa || exit 1
  git fetch -q origin main || exit 1
  git fetch -q "$in/bundle" "+refs/oa-remote/*:refs/oa-remote/*" || exit 1
  if [[ ! -d "$tree/.git" && ! -f "$tree/.git" ]]; then
    git worktree add -q --detach "$tree" "$commit" || exit 1
  fi
  git -C "$tree" checkout -q --detach -f "$commit" || exit 1
  git -C "$tree" clean -qfd || exit 1
  # clean -d keeps ignored files, and a build in this slot leaves the project pages' table
  # (projects/baked.coax, written by `openagents project bake` from that build's commit). A
  # later job's suites would load it as if it were this commit's: project-frames.coax failed
  # on slots whose table predated cdc62f911a's METAVERSE_* names and passed on fresh ones
  # (#13488). Without it they skip those suites, as on any fresh tree; a build makes it again.
  rm -f "$tree/apps/openagents-web/projects/baked.coax"
  git for-each-ref --format='%(refname)' refs/oa-remote/ | grep -v "/$commit\$" | xargs -r -n 50 git update-ref -d 2> /dev/null
  exit 0
) 8> ~/oa/.oa-remote.lock || infra "could not check out $commit on $(hostname)"
rm -rf "$in"
# The compiler: coax/build.sh installs the published Linux build for this
# tree's compiler sources (or bootstraps one), once per source fingerprint.
cd "$tree" || infra "no tree"
fingerprint="$(coax/published.sh name "$tree" 2> /dev/null || echo unknown)"
compiler_line=""
# The Linux release toolchain (publish-host.sh): static GCC for musl from
# nixpkgs. The fingerprint names the toolchain too, so it is taken in it.
toolchain=(nix shell nixpkgs#pkgsStatic.stdenv.cc nixpkgs#python3 nixpkgs#gnumake --command env
           CC="$(uname -m)-unknown-linux-musl-gcc" CXX="$(uname -m)-unknown-linux-musl-g++" COAX_LDFLAGS=-static)
# The slot's compiler is kept only while its record matches this tree in that
# toolchain, as tool.sh suites checks it (check.sh). nixpkgs-unstable moves
# under a slot: its store paths change when Nix fetches the channel again, and
# a compiler kept by its sources alone was then refused by every suite of the
# job ("was not built from this tree"), which the pipeline read as a failing
# commit (2026-11-11: five false culprits, main-green set back 3 hours, #13255).
slot_compiler_ok() {
  [[ -x coax/bin/coax && "$(cat ~/slots/$slot/compiler 2> /dev/null)" == "$fingerprint" ]] || return 1
  "${toolchain[@]}" coax/provenance.sh verify "$tree" coax/bin/coax coax/bin/provenance-check coax/bin/provenance.record > /dev/null 2>&1
}
# OA_REMOTE_FRESH_COMPILER=1: the caller's retry of a check the host could not
# judge (after.sh, #13256): the slot's compiler is installed again first.
[[ "${OA_REMOTE_FRESH_COMPILER:-0}" != 1 ]] || rm -f ~/slots/$slot/compiler
if ! slot_compiler_ok; then
  # A compiler another host built for these sources (#13119): build.sh's
  # cache entry (~/.cache/coax-release/FINGERPRINT), from the shared bucket
  # under compiler/ARCH/FINGERPRINT.tar.gz. build.sh verifies it against this
  # tree's fingerprint and its record before installing it, else bootstraps.
  release_cache="${COAX_RELEASE_CACHE:-$HOME/.cache/coax-release}"
  release_key="$("${toolchain[@]}" coax/provenance.sh fingerprint "$tree" 2> /dev/null || true)"
  compiler_object="compiler/$(uname -m)/$release_key.tar.gz"
  tool="${COAX_SHARED_CACHE_TOOL:-}"
  fetched=0 compiler_began=$(date +%s)
  if [[ -n "$release_key" && -n "$tool" && ! -d "$release_cache/$release_key" ]] && mkdir -p "$release_cache" &&
     got="$(mktemp -d "$release_cache/.shared.XXXXXXXX")"; then
    if "$tool" get "$compiler_object" "$got/c.tgz" && mkdir "$got/x" && tar -xzf "$got/c.tgz" -C "$got/x" 2> /dev/null &&
       [[ -x "$got/x/coax" && -f "$got/x/provenance.record" ]] && mv -T "$got/x" "$release_cache/$release_key" 2> /dev/null; then
      fetched=1
    fi
    rm -rf "$got"
  fi
  if ! "${toolchain[@]}" coax/build.sh > ~/slots/$slot/build.log 2>&1; then
    tail -n 30 ~/slots/$slot/build.log >&2
    infra "coax/build.sh failed on $(hostname) (log above)"
  fi
  echo "$fingerprint" > ~/slots/$slot/compiler
  if grep -q "installed the checked build" ~/slots/$slot/build.log && (( fetched )); then
    compiler_line="compiler from the bucket in $(( $(date +%s) - compiler_began )) s"
  elif (( fetched )); then
    compiler_line="the bucket's compiler didn't verify; built here"
  elif [[ -n "$release_key" && -n "$tool" && -d "$release_cache/$release_key" ]]; then
    # Built here: share it (unless another host already did), off the job's path.
    ( tar -czf ~/slots/$slot/compiler-out.tgz -C "$release_cache/$release_key" . 2> /dev/null &&
        "$tool" put "$compiler_object" ~/slots/$slot/compiler-out.tgz; rm -f ~/slots/$slot/compiler-out.tgz ) > /dev/null 2>&1 &
    compiler_line="compiler built here, shared"
  fi
fi
ready=$(date +%s)
# Paths in the command that named the caller's checkout (@ROOT@) name this one.
args=()
for a in "${argv[@]}"; do args+=("${a//@ROOT@/$tree}"); done
tmp="$res.$$"; rm -rf "$tmp"; mkdir -p "$tmp"
# The shared cache's part: wasm.sh appends each step's result to the stats
# file. The check records this command used on any host last time (keyed by
# the command, not the tree) come first, so a check whose inputs didn't change
# since is reused here too.
stats=~/slots/$slot/cache-stats; : > "$stats"; export COAX_SHARED_CACHE_STATS="$stats"
check_cache="${COAX_CHECK_CACHE:-$HOME/.cache/coax-check}"
check_pack="checks-job/$(printf '%s\n' "$relcwd" "${argv[@]}" | sha256sum | cut -c1-32).tar.gz"
tool="${COAX_SHARED_CACHE_TOOL:-}"
if [[ -n "$tool" && -n "${COAX_SHARED_CACHE_TOKEN:-}" ]] && mkdir -p "$check_cache" &&
   "$tool" get "$check_pack" ~/slots/$slot/checks-in.tgz; then
  tar -xzmf ~/slots/$slot/checks-in.tgz -C "$check_cache" 2> /dev/null
fi
marker=~/slots/$slot/marker; touch "$marker"
export COAX_JOB_MARKER="$marker"
cd "$tree/$relcwd" || infra "no directory $relcwd"
# OA_REMOTE_TIMEOUT (seconds): the command stops there even if its caller is
# gone (a killed caller's job would otherwise hold its slot to the end).
deadline=()
[[ "${OA_REMOTE_TIMEOUT:-0}" =~ ^[1-9][0-9]*$ ]] && deadline=(timeout --kill-after=30 "$OA_REMOTE_TIMEOUT")
${deadline[@]+"${deadline[@]}"} "${args[@]}" > >(tee "$tmp/out") 2> >(tee "$tmp/err" >&2)
status=$?
wait
ended=$(date +%s)
if [[ -n "$tool" && -n "${COAX_SHARED_CACHE_TOKEN:-}" ]]; then
  # Only when the command used check records: a job that compiled nothing keeps the last pack.
  (cd "$check_cache" && find . -maxdepth 1 -type f ! -name '.*' -newer "$marker" > ~/slots/$slot/checks-out.list)
  [[ ! -s ~/slots/$slot/checks-out.list ]] ||
    { (cd "$check_cache" && tar -czf ~/slots/$slot/checks-out.tgz -T ~/slots/$slot/checks-out.list 2> /dev/null) &&
      "$tool" replace "$check_pack" ~/slots/$slot/checks-out.tgz; }
fi
cache_line=""
[[ -z "$tool" ]] || cache_line="; $("$tool" summary "$stats")"
[[ -z "$compiler_line" ]] || cache_line="$cache_line; $compiler_line"
# The files the command wrote or changed come back: everything newer than the
# marker, except Git's, the compiler's and node_modules.
cd "$tree" || infra "no tree"
find . \( -path ./.git -o -path ./coax/bin -o -name node_modules \) -prune -o -type f -newer "$marker" -print0 > "$tmp/files"
tar --null -cf "$tmp/files.tar" -T "$tmp/files" 2> /dev/null
echo "$status" > "$tmp/status"
printf 'slot=%s setup=%s run=%s files=%s%s\n' "$slot" "$((ready - began))" "$((ended - ready))" "$(tr -cd '\0' < "$tmp/files" | wc -c)" "$cache_line" > "$tmp/times"
rm -rf "$res"; mv "$tmp" "$res"
echo "remote: $(hostname) slot $slot: checkout and compiler $((ready - began)) s, command $((ended - ready)) s, status $status" >&2
exit "$status"
