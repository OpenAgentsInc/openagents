# Storybook (historical)

The Effuse-based storybook and visual regression suite lived in the former **apps/web** app, which has been removed. The repo no longer includes that storybook.

For the current web app:

- **apps/openagents.com** — Laravel + Inertia + React. Component docs and local dev: see `apps/openagents.com/README.md`.
- **packages/effuse-test** — E2E runner now targets `apps/openagents.com` (default `--project ../../apps/openagents.com`). Visual snapshot baselines: `apps/openagents.com/tests/visual/storybook/` (if added).
