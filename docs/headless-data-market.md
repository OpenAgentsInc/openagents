# Headless Data Market

This document describes the current no-window Data Market control path.

## Purpose

Run the existing desktop-owned Data Seller control plane without opening the UI
window.

This path intentionally does **not** create a second seller implementation.
It reuses:

- the normal desktop startup path
- the normal desktop-control runtime
- the same typed `Data Market` desktop-control actions that `autopilotctl`
  already drives

## Current MVP shape

The current MVP-safe headless path is:

- start `autopilot-headless-data-market`
- let it write the normal desktop-control manifest
- target it with `autopilotctl data-market ...`

Because it is the same desktop-control host, confirmation and read-back
discipline remain unchanged:

- asset publish still requires preview + explicit confirm
- grant publish still requires preview + explicit confirm
- delivery and revocation still route through the same seller logic
- kernel read-back remains the canonical authority confirmation surface

## Start the runtime

```bash
cargo run -p autopilot-desktop --bin autopilot_headless_data_market -- \
  --manifest-path /tmp/openagents-data-market-desktop-control.json
```

The runtime starts the Autopilot app with its window hidden and writes the
standard desktop-control manifest at the usual manifest location.

In another shell, target it with `autopilotctl`:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/openagents-data-market-desktop-control.json \
  --json data-market seller-status
```

If `--manifest-path` is omitted, the runtime uses the default desktop-control
manifest location under `~/.openagents/logs/autopilot/desktop-control.json`.
Using an explicit manifest path is safer when a normal desktop session might
already be running on the same machine.

## Typical flow

1. Package local material:

```bash
scripts/autopilot/data_market_package.py \
  --source ./my-data \
  --output-dir ./tmp/package \
  --title "My Data Bundle" \
  --price-sats 250
```

2. Draft and preview the asset:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market draft-asset \
  --file ./tmp/package/listing-template.json

cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market preview-asset
```

3. Continue with confirm-gated publish, grant, payment, delivery, and
   revocation using the same `autopilotctl data-market ...` command tree.

## Smoke harness

The repo now includes a starter no-window smoke harness:

```bash
scripts/autopilot/headless-data-market-smoke.sh
```

That harness verifies:

- the hidden runtime starts
- `autopilotctl` can reach it through the standard manifest
- deterministic packaging output can be drafted into seller state
- asset preview works without opening the UI window

## Boundary note

This is currently a **no-window desktop host**, not yet a fully displayless
minimal seller daemon.

That is deliberate for the MVP because it keeps the headless path on the same
seller logic and desktop-control contract while avoiding a second unofficial
control plane.
