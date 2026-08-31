#!/bin/sh
# Refuse a release when the OpenAgents forge and the GitHub mirror disagree.
#
# The forge, https://openagents.com/OpenAgentsInc/openagents.git, is the
# release authority. The GitHub repository is a mirror. A release that was
# tagged or built from one side while the other side is different ships a
# commit the other side cannot resolve, so this gate exits nonzero before any
# artifact is produced.

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
cd "$repo_root"

forge_remote="openagents"
mirror_remote="github"
forge_ref="refs/remotes/${forge_remote}/main"
mirror_ref="refs/remotes/${mirror_remote}/main"

die() {
  echo "$@" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || die "git is required"

pin=0
if [ $# -gt 0 ]; then
  case "$1" in
    --pin) pin=1 ;;
    *) die "unknown argument: $1 (use --pin)" ;;
  esac
fi

git fetch "$forge_remote" main || die "could not fetch $forge_remote main"
git fetch "$mirror_remote" main || die "could not fetch $mirror_remote main"

forge_sha=$(git rev-parse --verify "$forge_ref" 2>/dev/null) ||
  die "could not resolve $forge_ref; $forge_remote main is missing"
mirror_sha=$(git rev-parse --verify "$mirror_ref" 2>/dev/null) ||
  die "could not resolve $mirror_ref; $mirror_remote main is missing"

if [ "$pin" = 1 ]; then
  pin_file="$script_dir/.release-pinned-commit"
  [ -f "$pin_file" ] || die "$pin_file does not exist. Create it with the forge main SHA to pin."
  pinned=$(tr -d '[:space:]' < "$pin_file")
  [ "$pinned" = "$forge_sha" ] ||
    die "pinned commit $pinned does not match $forge_remote main $forge_sha"
  echo "release source parity: OK $forge_sha (pinned)"
else
  if [ "$forge_sha" = "$mirror_sha" ]; then
    echo "release source parity: OK $forge_sha"
    exit 0
  fi

  {
    echo "release source parity: REFUSED"
    echo "  $forge_remote main: $forge_sha"
    echo "  $mirror_remote main: $mirror_sha"
    echo
    echo "commits on $forge_remote that $mirror_remote does not have:"
    git log --oneline "${mirror_sha}..${forge_sha}" 2>/dev/null || true
    echo
    echo "commits on $mirror_remote that $forge_remote does not have:"
    git log --oneline "${forge_sha}..${mirror_sha}" 2>/dev/null || true
    echo
    echo "Push the forge state to the mirror, or investigate the divergence, before publishing."
  } >&2
  exit 1
fi
