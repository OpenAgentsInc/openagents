#!/usr/bin/env bash
# Push the current branch (feature/left-side-panel) to your fork and open a PR to upstream.
# Run from openagents/ repo root. Requires: gh, git, and push access to fork.
#
#   cd /path/to/openagents && ./scripts/push-and-pr-left-side-panel.sh

set -e

UPSTREAM="OpenAgentsInc/openagents"
BRANCH="feature/left-side-panel"

if [[ "$(git branch --show-current)" != "$BRANCH" ]]; then
  echo "Expected to be on branch $BRANCH. Current: $(git branch --show-current)"
  exit 1
fi

echo "Pushing $BRANCH to remote 'fork' ..."
git push -u fork "$BRANCH"

echo "Opening PR to $UPSTREAM ..."
gh pr create \
  --repo "$UPSTREAM" \
  --base main \
  --head "lightbulbmomentlabs:$BRANCH" \
  --title "feat(autopilot): add left side panel with sidebar, settings, and Go Online toggle" \
  --body "## Left side panel

- **Resizable/collapsible sidebar** — Right-hand panel with a draggable handle (grab cursor). Click handle to collapse/expand; drag to resize.
- **Settings** — 16px icon in bottom-right of sidebar with hover tooltip (\"Settings\").
- **Panes** — Window panes auto-resize when the sidebar is resized so they never overlap the panel.
- **Canvas** — Background dots grid stays fixed (no jitter when dragging the sidebar).
- **Go Online** — Minimal toggle at top of panel: \`Offline | [pill] | Online\`, right-aligned. Single control to go online/offline; thumb and track reflect state.

Files: \`app_state.rs\`, \`input.rs\`, \`pane_system.rs\`, \`render.rs\`."

echo "Done. PR created."
