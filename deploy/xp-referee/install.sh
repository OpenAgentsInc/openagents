#!/usr/bin/env bash
# Builds microcoder from a commit of OpenAgentsInc/openagents and installs
# the XP referee job as a user service and timer on this host. Run it on
# the referee host, the one that holds ~/.openagents/nostr/referee-key.
#
#   deploy/xp-referee/install.sh [COMMIT]    (default: origin/main)
#
# It builds in its own worktree and target directory, so no other checkout
# or install build is touched, and prints the commit the job now runs.
set -euo pipefail

repo="${OPENAGENTS_REPO:-$HOME/openagents}"
base="${XP_REFEREE_HOME:-$HOME/.cache/openagents/referee}"
bin="$HOME/.local/libexec/openagents-referee"
units="$HOME/.config/systemd/user"

if [ ! -s "$HOME/.openagents/nostr/referee-key" ]; then
  echo "there's no referee key at ~/.openagents/nostr/referee-key; install it first" >&2
  exit 1
fi

git -C "$repo" fetch --quiet origin
commit="$(git -C "$repo" rev-parse "${1:-origin/main}")"
if [ -e "$base/src" ]; then
  git -C "$base/src" checkout --quiet --detach "$commit"
else
  git -C "$repo" worktree add --detach "$base/src" "$commit"
fi

(cd "$base/src" && CARGO_TARGET_DIR="$base/target" nice cargo build --release -p microcoder)

mkdir -p "$bin" "$units"
install -m 0755 "$base/target/release/microcoder" "$bin/microcoder.new"
mv "$bin/microcoder.new" "$bin/microcoder"
echo "$commit" > "$bin/REVISION"
install -m 0644 "$base/src/deploy/xp-referee/openagents-xp-referee.service" "$units/"
install -m 0644 "$base/src/deploy/xp-referee/openagents-xp-referee.timer" "$units/"
systemctl --user daemon-reload
systemctl --user enable --now openagents-xp-referee.timer
systemctl --user start openagents-xp-referee.service
echo "the XP referee runs $(cat "$bin/REVISION")"
systemctl --user --no-pager status openagents-xp-referee.timer | head -5
