#!/usr/bin/env bash
# Builds the hosted eval runner and the Coder it runs from a commit of
# OpenAgentsInc/openagents, and installs the runner as a user service on
# this host. Run it on the runner host, the one that holds
# ~/.openagents/nostr/eval-runner-key and ~/.config/openagents/eval-runner.env.
#
#   deploy/eval-runner/install.sh [COMMIT]    (default: origin/main)
#
# It builds in its own worktree and target directory, so no other checkout
# or install build is touched. The service reads the catalog, the question
# sets, and the Gym's gates from that worktree, so it's checked out at the
# commit the binaries were built from. It prints the commit the service now
# runs. docs/deployment/eval-runner.md is the runbook.
set -euo pipefail

repo="${OPENAGENTS_REPO:-$HOME/openagents}"
base="${EVAL_RUNNER_HOME:-$HOME/.cache/openagents/eval-runner}"
bin="$HOME/.local/libexec/openagents-eval-runner"
units="$HOME/.config/systemd/user"
env_file="$HOME/.config/openagents/eval-runner.env"

if [ ! -s "$HOME/.openagents/nostr/eval-runner-key" ]; then
  echo "there's no runner key at ~/.openagents/nostr/eval-runner-key; install it first" >&2
  exit 1
fi
if [ ! -s "$env_file" ]; then
  echo "there's no $env_file; copy deploy/eval-runner/eval-runner.env.example and fill it in" >&2
  exit 1
fi
if [ "$(stat -c %a "$env_file" 2>/dev/null || stat -f %Lp "$env_file")" != "600" ]; then
  echo "$env_file holds keys; chmod 600 it" >&2
  exit 1
fi

git -C "$repo" fetch --quiet origin
commit="$(git -C "$repo" rev-parse "${1:-origin/main}")"
if [ -e "$base/src" ]; then
  git -C "$base/src" checkout --quiet --detach "$commit"
else
  git -C "$repo" worktree add --detach "$base/src" "$commit"
fi

(cd "$base/src" && CARGO_TARGET_DIR="$base/target" nice cargo build --release \
  -p eval-runner --bin eval-runner -p coder --bin coder)

# The catalog is tracked in the repository (deploy/eval-runner/catalog), so
# a deploy carries it: the service reads EVAL_RUNNER_CATALOG from this file
# after the environment file, which keeps the keys and nothing else that
# changes with the code.
catalog=""
while IFS= read -r line; do
  case "$line" in ''|'#'*) continue ;; esac
  catalog="${catalog:+$catalog:}$base/src/$line"
done < "$base/src/deploy/eval-runner/catalog"
printf 'EVAL_RUNNER_CATALOG=%s\n' "$catalog" > "$base/catalog.env.new"
mv "$base/catalog.env.new" "$base/catalog.env"

mkdir -p "$bin" "$units"
install -m 0755 "$base/target/release/eval-runner" "$bin/eval-runner.new"
install -m 0755 "$base/target/release/coder" "$bin/coder.new"
mv "$bin/eval-runner.new" "$bin/eval-runner"
mv "$bin/coder.new" "$bin/coder"
echo "$commit" > "$bin/REVISION"
install -m 0644 "$base/src/deploy/eval-runner/openagents-eval-runner.service" "$units/"
systemctl --user daemon-reload
# Check the configuration before the service takes requests.
(set -a; . "$env_file"; . "$base/catalog.env"; set +a; "$bin/eval-runner" check)
systemctl --user enable openagents-eval-runner.service
systemctl --user restart openagents-eval-runner.service
echo "the eval runner runs $(cat "$bin/REVISION")"
systemctl --user --no-pager status openagents-eval-runner.service | head -5
