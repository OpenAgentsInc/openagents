#!/usr/bin/env bash
# One pass of the code graph publisher (#11622): fetch main, and when its
# code changed since the published graph, build the graph of main and
# force-push it as the single commit of the `code-graph` branch. A pass
# with no code change (docs, assets) does nothing.
set -euo pipefail

base="${CODE_GRAPH_HOME:-$HOME/.cache/openagents/code-graph}"
bin="$HOME/.local/libexec/openagents-code-graph"
git -C "$base/repo.git" fetch --quiet --no-tags origin '+refs/heads/main:refs/heads/main'
exec nice "$bin/code-graph" publish --root "$base/repo.git" --rev main --remote origin
