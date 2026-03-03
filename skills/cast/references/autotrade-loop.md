# Autotrade Loop

Use this guide to run CAST stages continuously from CLI with deterministic receipts.

## What It Runs

`cast-autotrade-loop.sh` orchestrates stage wrappers:

- `check` -> `cast-spell-check.sh`
- `prove` -> `cast-spell-prove.sh`
- `sign` -> `cast-sign-and-broadcast.sh`
- `inspect` -> `cast-show-spell.sh`

Each iteration writes stage receipts and an `autotrade_loop` summary receipt.

## One Iteration (Recommended First)

```bash
skills/cast/scripts/cast-autotrade-loop.sh \
  --config skills/cast/assets/autotrade-loop.config.example \
  --once
```

Default safety behavior:

- mock prove enabled
- sign stage uses `--dry-run`

## Continuous Loop

```bash
skills/cast/scripts/cast-autotrade-loop.sh \
  --config /absolute/path/to/autotrade.env \
  --interval-seconds 60 \
  --max-iterations 0 \
  --continue-on-error
```

To enable live broadcast explicitly:

```bash
skills/cast/scripts/cast-autotrade-loop.sh \
  --config /absolute/path/to/autotrade.env \
  --real-prove \
  --broadcast \
  --interval-seconds 120
```

## Receipts

Per iteration:

- `<run_root>/<timestamp>-<n>/receipts/check.json`
- `<run_root>/<timestamp>-<n>/receipts/prove.json`
- `<run_root>/<timestamp>-<n>/receipts/sign.json`
- `<run_root>/<timestamp>-<n>/receipts/inspect.json`
- `<run_root>/<timestamp>-<n>/receipts/autotrade_loop.json`

Use `--summary-file` to keep a stable latest summary path for UI/process monitors.

## Failure Handling

- default behavior stops on first failed stage
- `--continue-on-error` keeps the loop running and records failure in summary receipt
- use `--stages` to isolate a subset (`check,prove` or `sign,inspect`)
