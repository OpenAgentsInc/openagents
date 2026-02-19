# Local CI Policy

OpenAgents CI gates are local-first and executed through tracked git hooks and
scripts in this repository.

GitHub workflow automation is intentionally not used for CI in this repo.

## Install Hooks

From repository root:

```bash
./scripts/install-git-hooks.sh
```

This configures:

- `.githooks/pre-commit` -> `scripts/local-ci.sh changed`
- `.githooks/pre-push` -> `scripts/local-ci.sh changed`

## Local CI Entrypoint

```bash
./scripts/local-ci.sh changed
```

Supported lanes:

- `changed` (default)
- `all`
- `docs`
- `proto`
- `runtime`
- `comms`
- `openclaw`

Examples:

```bash
./scripts/local-ci.sh runtime
./scripts/local-ci.sh comms
./scripts/local-ci.sh all
```

## Temporary Bypass (Use Sparingly)

```bash
OA_SKIP_LOCAL_CI=1 git commit -m "..."
OA_SKIP_LOCAL_CI=1 git push
```

If bypassing, run equivalent lane commands manually before merge.
