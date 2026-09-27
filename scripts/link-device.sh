#!/usr/bin/env bash
# Build this checkout's Coder host and link a computer as a serving host.
#
#   scripts/link-device.sh [SETUP OPTIONS]            this computer
#   scripts/link-device.sh --ssh DEST [SETUP OPTIONS] another computer
#   scripts/link-device.sh --build-only               build and stage only
#
# Local: builds and installs `coder` with scripts/install-coder.sh, builds
# `coder-service` and `microcoder` into ~/.openagents/bin, stages the `coder`
# build as the host service's bundle with scripts/coder-host.py, then runs
# `coder link setup SETUP OPTIONS`.
#
# --ssh DEST: fast-forwards DEST's clean checkout (--remote-checkout DIR,
# default ~/openagents) to origin/main, runs this script there with
# --build-only, then runs `coder link setup --ssh DEST SETUP OPTIONS` here,
# which sets DEST up with the owner's public key and lists it in the owner
# directory with the owner key held on this computer. The owner key never
# leaves this computer.
#
# Every step is idempotent: an unchanged build stages nothing new, and
# `coder link setup` changes the service only when something changed.
# See docs/coder/guides/link-devices.md.
set -euo pipefail

source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
home="${OPENAGENTS_HOME:-$HOME/.openagents}"
target_dir="${CODER_INSTALL_TARGET_DIR:-$HOME/.cache/openagents/target-install-coder}"
cargo="${CODER_INSTALL_CARGO:-cargo}"

say() { echo "link-device: $*" >&2; }
die() {
  say "$*"
  exit 1
}

ssh_dest=""
remote_checkout="openagents"
build_only=0
setup_args=()
while test $# -gt 0; do
  case "$1" in
    --ssh)
      test $# -ge 2 || die "--ssh needs a destination"
      ssh_dest="$2"
      shift 2
      ;;
    --remote-checkout)
      test $# -ge 2 || die "--remote-checkout needs a directory"
      remote_checkout="$2"
      shift 2
      ;;
    --build-only)
      build_only=1
      shift
      ;;
    -h | --help)
      sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      setup_args+=("$1")
      shift
      ;;
  esac
done

build() {
  CODER_INSTALL_TARGET_DIR="$target_dir" "$source_dir/scripts/install-coder.sh"
  say "building coder-service and microcoder"
  (
    cd "$source_dir"
    CARGO_TARGET_DIR="$target_dir" "$cargo" build --release --locked \
      -p coder-service --bin coder-service -p microcoder --bin microcoder
  ) || die "the build failed"
  mkdir -p "$home/bin"
  local name
  for name in coder-service microcoder; do
    cp "$target_dir/release/$name" "$home/bin/.$name.$$"
    chmod 755 "$home/bin/.$name.$$"
    mv -f "$home/bin/.$name.$$" "$home/bin/$name"
  done

  # The host service runs a digest-named bundle of this exact build.
  local built="$target_dir/release/coder" sha revision flags=()
  if command -v sha256sum >/dev/null; then
    sha="$(sha256sum "$built" | cut -d' ' -f1)"
  else
    sha="$(shasum -a 256 "$built" | cut -d' ' -f1)"
  fi
  revision="$(git -C "$source_dir" rev-parse HEAD)"
  if test -n "$(git -C "$source_dir" status --porcelain --untracked-files=no)"; then
    flags+=(--uncommitted-source)
  fi
  python3 "$source_dir/scripts/coder-host.py" --root "$home/host-bundle" --tasks "$home/tasks" \
    install --binary "$built" --sha256 "$sha" --source-revision "$revision" "${flags[@]+"${flags[@]}"}" >&2 ||
    die "staging the host bundle failed"
  say "staged host bundle $sha"
}

if test -n "$ssh_dest"; then
  test "$build_only" = 0 || die "--build-only runs on the computer being built"
  say "updating $ssh_dest:$remote_checkout to origin/main"
  # shellcheck disable=SC2029 # the checkout path expands on the remote side
  ssh -o BatchMode=yes "$ssh_dest" "cd $(printf %q "$remote_checkout") &&
    test -z \"\$(git status --porcelain --untracked-files=no)\" &&
    git fetch --quiet origin main &&
    git merge --quiet --ff-only origin/main &&
    scripts/link-device.sh --build-only" ||
    die "building on $ssh_dest failed (a dirty checkout is left alone)"
  coder="$home/bin/coder"
  test -x "$coder" || die "no local coder at $coder; run scripts/link-device.sh here first"
  exec "$coder" link setup --ssh "$ssh_dest" "${setup_args[@]+"${setup_args[@]}"}"
fi

build
if test "$build_only" = 1; then
  exit 0
fi
exec "$home/bin/coder" link setup "${setup_args[@]+"${setup_args[@]}"}"
