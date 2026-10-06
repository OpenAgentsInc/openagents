#!/usr/bin/env bash
# Install this checkout's Coder Terminal and bundled OpenAgents CLI, or
# roll back to the builds they replaced.
#
#   scripts/install-coder.sh             build and install both commands
#   scripts/install-coder.sh --rollback  restore the previous builds
#
# Install builds crates/coder and crates/openagents-cli in release mode with the toolchain
# rust-toolchain.toml pins, in a target directory of its own, and copies the
# binaries to ~/.openagents/versions/{coder,openagents}-openagents-<short-sha>, with
# `-dirty` appended when the tree had uncommitted changes. It then points
# ~/.openagents/bin/{coder,openagents} at those copies by renaming each new
# symbolic link over the old one. Both builds pass --version before either
# link changes. The builds they replaced are printed and recorded in
# ~/.openagents/versions/{coder,openagents}.previous.
# Every switch is also appended to the corresponding .history file, so
# a build two installs back stays recorded.
#
# Rollback restores both recorded builds and records the ones it replaces,
# so a second rollback undoes the first. A legacy Coder-only rollback leaves
# the CLI unchanged.
#
# Environment:
#   OPENAGENTS_HOME            Default ~/.openagents.
#   CODER_INSTALL_TARGET_DIR   Cargo's target directory for the release
#                              build; default ~/.cache/openagents/target-install-coder.
#   CODER_INSTALL_CARGO        Cargo command; default cargo. Tests point it at
#                              a stub.
set -euo pipefail

source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
install_root="${OPENAGENTS_HOME:-$HOME/.openagents}"
bin_dir="$install_root/bin"
versions="$install_root/versions"
cargo="${CODER_INSTALL_CARGO:-cargo}"
target_dir="${CODER_INSTALL_TARGET_DIR:-$HOME/.cache/openagents/target-install-coder}"

say() { echo "install-coder: $*" >&2; }
die() {
  say "$*"
  exit 1
}

usage() {
  sed -n '2,6p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

# Where a command's link points now, or `none`.
current() {
  local link="$bin_dir/$1"
  if test -L "$link"; then
    readlink "$link"
  elif test -e "$link"; then
    echo "$link (a file, not a link)"
  else
    echo none
  fi
}

# Points $1 at $2 by renaming a new link over the old one, and
# appends the switch to the history, so no build the link ever pointed at
# goes unrecorded.
switch_to() {
  local command="$1" target="$2"
  local link="$bin_dir/$command" history="$versions/$command.history"
  local was
  was="$(current "$command")"
  local staged="$bin_dir/.$command.$$"
  mkdir -p "$bin_dir" "$versions"
  if test "$target" = none; then
    rm -f "$link"
  else
    ln -sfn "$target" "$staged"
    mv -f "$staged" "$link"
  fi
  printf '%s %s -> %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$was" "$target" >>"$history"
}

rollback() {
  local record="$versions/coder.previous"
  test -r "$record" || die "no previous build is recorded in $record"
  local previous cli_previous="" cli_record="$versions/openagents.previous"
  previous="$(cat "$record")"
  test -n "$previous" && test "$previous" != none || die "$record is empty, so there is no previous build to roll back to"
  test -x "$previous" || die "the previous build $previous is missing or not executable"
  if test -r "$cli_record"; then
    cli_previous="$(cat "$cli_record")"
    test -n "$cli_previous" || die "$cli_record is empty; both links are unchanged"
    if test "$cli_previous" != none; then
      test -x "$cli_previous" || die "the previous CLI build $cli_previous is missing or not executable; both links are unchanged"
      "$cli_previous" --version || die "the previous CLI build does not run; both links are unchanged"
    fi
  fi
  "$previous" --version || die "the previous Coder build does not run; both links are unchanged"
  local was cli_was
  was="$(current coder)"
  cli_was="$(current openagents)"
  switch_to coder "$previous"
  echo "$was" >"$record"
  say "rolled back: $bin_dir/coder now points to $previous"
  say "replaced:    $was (saved in $record, so a second --rollback undoes this one)"
  if test -n "$cli_previous"; then
    switch_to openagents "$cli_previous"
    echo "$cli_was" >"$cli_record"
    say "rolled back: $bin_dir/openagents restored to $cli_previous"
  fi
}

install() {
  local commit short dirty suffix
  commit="$(git -C "$source_dir" rev-parse HEAD)" || die "$source_dir is not a Git checkout"
  short="$(git -C "$source_dir" rev-parse --short=10 HEAD)"
  dirty=0
  if test -n "$(git -C "$source_dir" status --porcelain --untracked-files=no)"; then
    dirty=1
  fi
  suffix="openagents-$short"
  if test "$dirty" = 1; then
    suffix="$suffix-dirty"
  fi

  say "building coder and openagents at commit $short$(test "$dirty" = 1 && echo ' with uncommitted changes') in $target_dir"
  (
    cd "$source_dir"
    CARGO_TARGET_DIR="$target_dir" \
      CODER_BUILD_COMMIT="$commit" \
      CODER_BUILD_DIRTY="$dirty" \
      "$cargo" build --release --locked -p coder -p openagents-cli --bin coder --bin openagents
  ) || die "the build failed; each installed link is unchanged"
  local command built installed staged was record history
  for command in coder openagents; do
    built="$target_dir/release/$command"
    test -x "$built" || die "the build finished but left no executable at $built; each installed link is unchanged"
    "$built" --version || die "$built --version failed; each installed link is unchanged"
  done

  mkdir -p "$versions"
  for command in coder openagents; do
    installed="$versions/$command-$suffix"
    staged="$versions/.$command-$suffix.$$"
    cp "$target_dir/release/$command" "$staged"
    chmod 755 "$staged"
    mv -f "$staged" "$installed"
  done
  for command in coder openagents; do
    installed="$versions/$command-$suffix"
    record="$versions/$command.previous"
    history="$versions/$command.history"
    was="$(current "$command")"
    switch_to "$command" "$installed"
    if test "$was" != "$installed"; then
      echo "$was" >"$record"
    fi
    say "installed: $bin_dir/$command now points to $installed"
    say "previous:  $was"
    say "history:   $history"
    warn_path "$command"
  done
  say "to connect your phone, see docs/coder/guides/link-devices.md"
  say "to go back to the previous builds, run scripts/install-coder.sh --rollback"
}

warn_path() {
  local command="$1" link="$bin_dir/$1"
  local found
  found="$(command -v "$command" || true)"
  if test -z "$found"; then
    say "$command is not on your PATH; add $bin_dir to PATH, or link ~/.local/bin/$command to $link"
  elif test "$(readlink -f "$found")" != "$(readlink -f "$link")"; then
    say "note: the $command on your PATH is $found, not this install; put $bin_dir earlier in PATH to run $link"
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
