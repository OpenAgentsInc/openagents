# Autopilot Desktop From-DMG + Builtin Compute Gate Audit

> Status: public-safe issue #7023 audit, 2026-06-29. This record flips no
> product-promise state. It narrows the remaining closure gates for
> `autopilot.desktop_gui_client.v1` and
> `autopilot.builtin_compute_agent.v1` without claiming owner-gated installer
> proof exists.

## Scope

Issue #7023 asks for the clean-Mac from-DMG proof needed before the desktop GUI
and built-in compute promises can move past yellow. The source registry already
keeps both promises yellow and names the relevant blockers:

- `blocker.product_promises.autopilot_desktop_from_dmg_proof_owner_gated`
- `blocker.product_promises.autopilot_desktop_live_runtimes_not_wired`
- `blocker.product_promises.autopilot_desktop_remote_cloud_lane_not_wired`
- `blocker.product_promises.autopilot_desktop_pricing_distribution_undecided`
- `blocker.product_promises.builtin_compute_agent_signed_recut_missing`
- `blocker.product_promises.builtin_compute_agent_live_from_install_smoke_missing`
- `blocker.product_promises.openagents_compute_metering_live_smoke_missing`

This audit keeps those blockers intact. It records the exact evidence bundle a
future owner-run release proof must attach so the next pass can be mechanical
rather than interpretive.

## Current Evidence

The desktop GUI has source and test evidence for the local shell, local Pylon
loopback, first-run onboarding, AO-3 identity choice, AO-4 status projection,
black-screen guard, and AO-6 headless smoke. The runtime cores for PDF,
loopback Sites preview, asset ingestion, and browser automation are built behind
test seams, but their live desktop runtime wiring is not yet proven from the
installer.

The built-in compute agent has source and test evidence for the first-screen
Go online path, built-in-agent RPC, hosted-compute readiness checks, managed
scratch workspace, local daily-start cap, bounded session start, and the
metering smoke projection. The already-published installer is not evidence for
that source because it predates the source change.

## Required From-DMG Bundle

A closure-quality proof for `autopilot.desktop_gui_client.v1` needs public-safe
refs for all of the following:

- Fresh signed and notarized DMG built from the target release commit.
- Clean external Apple Silicon Mac install launched from Finder, not from a
  source checkout or terminal.
- Rendered Autopilot Desktop window screenshot or video proving no black-screen
  regression.
- Production `/api/public/pylon-stats` appearance for the freshly installed
  local Pylon.
- Local session list, decision cards, and event timeline populated from
  public-safe projections.
- Live runtime proof for PDF rendering, loopback Sites preview, asset ingestion,
  and browser automation through the packaged desktop runtime.
- Distribution and pricing decision refs, or an explicit registry narrowing
  that removes those claims from the desktop promise.

## Required Builtin Compute Bundle

A closure-quality proof for `autopilot.builtin_compute_agent.v1` needs
public-safe refs for all of the following:

- Signed and notarized recut that includes the built-in-agent source.
- Packaged OpenAgents compute entitlement or credential resolution, represented
  only by public-safe refs.
- From-install Go online session with no user-supplied provider key.
- Bounded session closeout proving the daily session and token ceilings held.
- Exact usage or quota ledger row refs for the metered compute path.
- Public quota projection refs showing sessions remaining and reset timing.
- Operator approval refs for any live hosted-compute spend or production
  activation.

The existing `builtin-compute-agent-metering-smoke` projection is the expected
shape for the built-in compute bundle. CI-mode projection tests are not a
substitute for live signed-install evidence.

## Promise Decision

No promise state changes in this audit:

- `autopilot.desktop_gui_client.v1` stays yellow.
- `autopilot.builtin_compute_agent.v1` stays yellow.

No blocker is cleared. The audit moves #7023 forward by making the release proof
checklist explicit and adding it as dereferenceable evidence on both promise
records. A future green transition still requires the owner-run proof bundle,
owner sign-off, and a receipt-first transition record via
`POST /api/operator/product-promises/transitions`.

## Focused Verification

Run the product-promise registry test after updating the registry refs:

```sh
bun run --cwd apps/openagents.com/workers/api test -- src/product-promises.test.ts
```

Run the built-in compute projection test when touching the smoke shape:

```sh
bun run --cwd apps/openagents.com/workers/api test -- src/builtin-compute-agent-metering-smoke.test.ts
```
