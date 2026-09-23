#!/usr/bin/env bash
# Install this checkout's Coder Terminal as `coder`, or roll back to the
# build it replaced.
#
#   scripts/install-coder.sh             build, install, and switch to it
#   scripts/install-coder.sh --rollback  switch back to the previous build
#
# Install builds crates/coder in release mode with the toolchain
# rust-toolchain.toml pins, in a target directory of its own, and copies the
# binary to ~/.openagents/versions/coder-openagents-<short-sha>, with
# `-dirty` appended when the tree had uncommitted changes. It then points
# ~/.openagents/bin/coder at that copy by renaming a new symbolic link over
# the old one, so `coder` is always one build or the other and never
# missing. The build it replaced is printed and recorded in
# ~/.openagents/versions/coder.previous; a rollback is that one link again.
#
# Rollback points the link back at the recorded build and records the one
# it replaced, so a second rollback undoes the first.
#
# Environment:
#   OPENAGENTS_HOME            Default ~/.openagents.
#   CODER_INSTALL_TARGET_DIR   Cargo's target directory for the release
#                              build; default ~/.cache/openagents/target-install-coder.
#   CODER_INSTALL_CARGO        Cargo command; default cargo. Tests point it at
#                              a stub.
set -euo pipefail

source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
home="${OPENAGENTS_HOME:-$HOME/.openagents}"
bin_dir="$home/bin"
versions="$home/versions"
link="$bin_dir/coder"
record="$versions/coder.previous"
cargo="${CODER_INSTALL_CARGO:-cargo}"
target_dir="${CODER_INSTALL_TARGET_DIR:-$HOME/.cache/openagents/target-install-coder}"

say() { echo "install-coder: $*" >&2; }
die() {
  say "$*"
  exit 1
}

usage() {
  sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

# Where the link points now, or `none`.
current() {
  if test -L "$link"; then
    readlink "$link"
  elif test -e "$link"; then
    echo "$link (a file, not a link)"
  else
    echo none
  fi
}

# Points the link at $1 by renaming a new link over the old one.
switch_to() {
  local target="$1"
  local staged="$bin_dir/.coder.$$"
  mkdir -p "$bin_dir"
  ln -sfn "$target" "$staged"
  mv -f "$staged" "$link"
}

rollback() {
  test -r "$record" || die "no previous build is recorded in $record"
  local previous
  previous="$(cat "$record")"
  test -n "$previous" && test "$previous" != none || die "$record records no build"
  test -x "$previous" || die "the recorded build $previous is not an executable"
  local was
  was="$(current)"
  switch_to "$previous"
  echo "$was" >"$record"
  say "rolled back: $link -> $previous"
  say "replaced:    $was (recorded in $record)"
  "$link" --version || die "$link --version failed after the rollback"
}

install() {
  local commit short dirty name
  commit="$(git -C "$source_dir" rev-parse HEAD)" || die "$source_dir is not a Git checkout"
  short="$(git -C "$source_dir" rev-parse --short=10 HEAD)"
  dirty=0
  if test -n "$(git -C "$source_dir" status --porcelain --untracked-files=no)"; then
    dirty=1
  fi
  name="coder-openagents-$short"
  if test "$dirty" = 1; then
    name="$name-dirty"
  fi

  say "building coder $short$(test "$dirty" = 1 && echo ' (dirty)') in $target_dir"
  (
    cd "$source_dir"
    CARGO_TARGET_DIR="$target_dir" \
      CODER_BUILD_COMMIT="$commit" \
      CODER_BUILD_DIRTY="$dirty" \
      "$cargo" build --release --locked -p coder --bin coder
  ) || die "the build failed; $link is unchanged"
  local built="$target_dir/release/coder"
  test -x "$built" || die "the build produced no executable at $built"

  mkdir -p "$versions"
  local installed="$versions/$name"
  local staged="$versions/.$name.$$"
  cp "$built" "$staged"
  chmod 755 "$staged"
  mv -f "$staged" "$installed"

  local was
  was="$(current)"
  switch_to "$installed"
  if test "$was" != "$installed"; then
    echo "$was" >"$record"
  fi
  say "installed: $link -> $installed"
  say "previous:  $was"
  if test "$was" != none && test "$was" != "$installed"; then
    say "roll back with: scripts/install-coder.sh --rollback (or ln -sfn '$was' '$link')"
  fi
  "$link" --version || die "$link --version failed; roll back with --rollback"

  local found
  found="$(command -v coder || true)"
  if test -z "$found"; then
    say "coder is not on PATH: add $bin_dir to PATH, or link ~/.local/bin/coder to $link"
  elif test "$(readlink -f "$found")" != "$(readlink -f "$link")"; then
    say "note: coder on PATH is $found, which does not resolve to $link"
  fi
}

case "${1:-}" in
  "") install ;;
  --rollback) rollback ;;
  -h | --help) usage ;;
  *)
    usage >&2
    die "unknown argument: $1"
    ;;
esac
