#!/usr/bin/env bash
# Firm reason: 5. One job on a remote build host (docs/verse/remote-builds.md):
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
# spec sets: key commit relcwd argv=(...) env_vars=(...)
# shellcheck disable=SC1090
source "$in/spec"
# The caller's COAX_* and COAXIAL_* settings hold for the compiler build too
# (COAX_MUTATION=0 skips its mutation gates, as release.sh does).
for v in ${env_vars[@]+"${env_vars[@]}"}; do export "${v?}"; done
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
  git for-each-ref --format='%(refname)' refs/oa-remote/ | grep -v "/$commit\$" | xargs -r -n 50 git update-ref -d 2> /dev/null
  exit 0
) 8> ~/oa/.oa-remote.lock || infra "could not check out $commit on $(hostname)"
rm -rf "$in"
# The compiler: coax/build.sh installs the published Linux build for this
# tree's compiler sources (or bootstraps one), once per source fingerprint.
cd "$tree" || infra "no tree"
fingerprint="$(coax/published.sh name "$tree" 2> /dev/null || echo unknown)"
if [[ ! -x coax/bin/coax || "$(cat ~/slots/$slot/compiler 2> /dev/null)" != "$fingerprint" ]]; then
  # The Linux release toolchain (publish-host.sh): static GCC for musl from nixpkgs.
  if ! nix shell nixpkgs#pkgsStatic.stdenv.cc nixpkgs#python3 nixpkgs#gnumake --command env \
      CC="$(uname -m)-unknown-linux-musl-gcc" CXX="$(uname -m)-unknown-linux-musl-g++" COAX_LDFLAGS=-static \
      coax/build.sh > ~/slots/$slot/build.log 2>&1; then
    tail -n 30 ~/slots/$slot/build.log >&2
    infra "coax/build.sh failed on $(hostname) (log above)"
  fi
  echo "$fingerprint" > ~/slots/$slot/compiler
fi
ready=$(date +%s)
# Paths in the command that named the caller's checkout (@ROOT@) name this one.
args=()
for a in "${argv[@]}"; do args+=("${a//@ROOT@/$tree}"); done
tmp="$res.$$"; rm -rf "$tmp"; mkdir -p "$tmp"
marker=~/slots/$slot/marker; touch "$marker"
cd "$tree/$relcwd" || infra "no directory $relcwd"
# OA_REMOTE_TIMEOUT (seconds): the command stops there even if its caller is
# gone (a killed caller's job would otherwise hold its slot to the end).
deadline=()
[[ "${OA_REMOTE_TIMEOUT:-0}" =~ ^[1-9][0-9]*$ ]] && deadline=(timeout --kill-after=30 "$OA_REMOTE_TIMEOUT")
${deadline[@]+"${deadline[@]}"} "${args[@]}" > >(tee "$tmp/out") 2> >(tee "$tmp/err" >&2)
status=$?
wait
ended=$(date +%s)
# The files the command wrote or changed come back: everything newer than the
# marker, except Git's, the compiler's and node_modules.
cd "$tree" || infra "no tree"
find . \( -path ./.git -o -path ./coax/bin -o -name node_modules \) -prune -o -type f -newer "$marker" -print0 > "$tmp/files"
tar --null -cf "$tmp/files.tar" -T "$tmp/files" 2> /dev/null
echo "$status" > "$tmp/status"
printf 'slot=%s setup=%s run=%s files=%s\n' "$slot" "$((ready - began))" "$((ended - ready))" "$(tr -cd '\0' < "$tmp/files" | wc -c)" > "$tmp/times"
rm -rf "$res"; mv "$tmp" "$res"
echo "remote: $(hostname) slot $slot: checkout and compiler $((ready - began)) s, command $((ended - ready)) s, status $status" >&2
exit "$status"
