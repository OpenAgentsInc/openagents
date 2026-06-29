# Autopilot World Visualization Gate Decision

Date: 2026-06-29

Issue: #7030

## Decision

Keep the Autopilot world visualizations intentionally flag-gated for now:

- `autopilot.agent_world_scene.v1` stays behind `CHAT_WORLD_SCENE`.
- `autopilot.bitcoin_payment_visualization.v1` stays behind `CHAT_WORLD_PAYMENTS`.
- `autopilot.pylon_growth_visualization.v1` rides the same chat-world flags and is not default-on.

This records the conservative product decision for the current registry. The
source-level and live in-app wiring remain valid evidence, but there is no new
default-on, staged-rollout, or broad shipped-feature claim without a fresh
owner-approved rollout receipt.

## Scope

This decision only covers public product-promise copy and blocker wording for
the three visual-scene promises. It does not change runtime flags, desktop app
defaults, payment logic, Pylon earning logic, multiplayer authority, payout
authority, or settlement authority.

## Registry Effect

The three promises remain yellow. Their blockers are intentional gate blockers,
not stale wiring blockers:

- `blocker.product_promises.agent_world_scene_intentionally_flag_gated`
- `blocker.product_promises.payment_visualization_intentionally_flag_gated`
- `blocker.product_promises.pylon_growth_intentionally_flag_gated`

Green still requires a default-on or staged-rollout receipt, or an owner-signed
green-scope decision recorded through the receipt-first product-promise process.

## Safety Notes

The scene is presentational. Payment particles must stay bound to public payment
or settlement evidence and stale beams must expire. Growth tiers must remain a
visualization of public settled-sats data only.
