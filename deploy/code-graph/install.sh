#!/usr/bin/env bash
# Installs the code graph publisher (#11622) as a user service and timer on
# this host: the one writer of the `code-graph` branch. Run it on one host
# only, one that can push to OpenAgentsInc/openagents.
#
#   deploy/code-graph/install.sh [COMMIT]    (default: origin/main)
#
# It builds `code-graph` in its own worktree and target directory, keeps a
# bare clone for publishing at ~/.cache/openagents/code-graph/repo.git, and
# prints the commit the job now runs.
set -euo pipefail

repo="${OPENAGENTS_REPO:-$HOME/openagents}"
base="${CODE_GRAPH_HOME:-$HOME/.cache/openagents/code-graph}"
bin="$HOME/.local/libexec/openagents-code-graph"
units="$HOME/.config/systemd/user"
remote="$(git -C "$repo" remote get-url origin)"

git -C "$repo" fetch --quiet origin
commit="$(git -C "$repo" rev-parse "${1:-origin/main}")"
if [ -e "$base/src" ]; then
  git -C "$base/src" checkout --quiet --detach "$commit"
else
  git -C "$repo" worktree add --detach "$base/src" "$commit"
fi
(cd "$base/src" && CARGO_TARGET_DIR="$base/target" nice cargo build --release -p code-graph)

if [ ! -d "$base/repo.git" ]; then
  git clone --quiet --bare "$remote" "$base/repo.git"
fi

mkdir -p "$bin" "$units"
install -m 0755 "$base/target/release/code-graph" "$bin/code-graph.new"
mv "$bin/code-graph.new" "$bin/code-graph"
install -m 0755 "$base/src/deploy/code-graph/publish.sh" "$bin/publish.sh"
echo "$commit" > "$bin/REVISION"
install -m 0644 "$base/src/deploy/code-graph/openagents-code-graph.service" "$units/"
install -m 0644 "$base/src/deploy/code-graph/openagents-code-graph.timer" "$units/"
systemctl --user daemon-reload
systemctl --user enable --now openagents-code-graph.timer
systemctl --user start openagents-code-graph.service
echo "the code graph publisher runs $(cat "$bin/REVISION")"
systemctl --user --no-pager status openagents-code-graph.timer | head -5
