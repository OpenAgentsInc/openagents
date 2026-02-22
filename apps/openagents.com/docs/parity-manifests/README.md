# Parity Manifests and Scoreboard

This directory contains baseline manifests and parity scoreboard metadata for the Laravel -> Rust migration program.

## Baseline Manifests

Refresh baseline route/page/command manifests:

```bash
cd apps/openagents.com
php artisan ops:export-parity-manifests --output=docs/parity-manifests/baseline
```

## Scoreboard Domains

Domain ownership + migration gating config:

- `scoreboard-domains.json`

Each domain controls:
- `baseline_path` (committed golden input)
- `captured_path` (fresh capture path)
- `migrated` gate (`true` => drift becomes merge-blocking regression)

## Scoreboard Run

Run local parity scoreboard:

```bash
./apps/openagents.com/scripts/archived-laravel/run-parity-scoreboard.sh
```

CI workflow: removed (workflow automation disabled by invariant).
