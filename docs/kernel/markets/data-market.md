# Data Market

This is the canonical status doc for the `Data Market`.

## Purpose

The Data Market prices access to useful context under explicit permissions.

Kernel-facing objects:

- `DataAsset`
- `AccessGrant`
- `PermissionPolicy`
- `DeliveryBundle`
- `RevocationReceipt`

The data market exists so access to artifacts, datasets, local context, and
private knowledge is explicit, permissioned, and receipted rather than being
smuggled through opaque prompt state.

## Current repo verdict

| Dimension | Status | Notes |
| --- | --- | --- |
| Product surface | seller authoring + read surface + narrow buyer request surface | there is now a dedicated read-only data-market pane in the desktop app plus a `Data Seller` conversational authoring lane with a structured local draft, seller-specific Codex session wiring, auto-provisioned first-party seller skills, typed `openagents.data_market.*` tools, seller-side targeted-request intake/evaluation, seller `payment-required` issuance, seller delivery-bundle/result publication, and explicit seller revoke/expire controls with `RevocationReceipt` read-back; there is now also a narrow `Data Buyer` pane that selects a visible asset and publishes a targeted NIP-90 data-access request, but broader buyer transaction UX is still incomplete |
| Kernel authority | `implemented` starter slice | authority and authenticated read-model flows exist in `openagents-kernel-core` and `apps/nexus-control` |
| Wire/proto | `implemented` starter slice | there is now a checked-in `openagents.data.v1` package plus proto-backed authority routes and a combined `/v1/kernel/data/snapshot` read model |
| Local prototype | `implemented` | richer provenance, packaging, and private-data economics live mostly in docs and adjacent desktop concepts |
| Planned | yes | broader discovery, pricing, payouts, provider economics, and product-facing UX remain planned |

## Implemented now

- register a `DataAsset`
- create an `AccessGrant`
- accept an access grant
- issue a `DeliveryBundle`
- revoke access and emit a `RevocationReceipt`
- open a dedicated `Data Seller` desktop shell with transcript, draft-card, exact preview card, and gated confirm/publish scaffolding
- auto-provision and pin the first-party `autopilot-data-seller` and `autopilot-data-market-control` skills for the dedicated seller lane
- expose typed `openagents.data_market.*` dynamic tools for seller status, draft, exact preview, blocked publish, and snapshot flows
- send seller prompts from the dedicated pane into the dedicated Codex seller thread and render the resulting transcript back into the pane
- require an explicit preview-confirm step before publication can be armed
- publish a `DataAsset` from the seller pane through Nexus and immediately read the canonical asset back into seller state
- reflect newly published assets into the read-only `Data Market` pane from the same kernel read-back path
- carry reusable grant-policy templates plus a first seller-side grant draft posture in the conversational seller flow
- preview and publish `AccessGrant` objects from the same seller flow and reflect the resulting grant read-back into the read-only `Data Market` pane
- show a seller-side inventory/status card with published asset/default-offer summaries and draft-vs-published warnings
- derive and publish an explicit OpenAgents-local NIP-90 data-vending profile from the seller pane into the shared relay/runtime lane
- parse incoming OpenAgents data-vending requests on the desktop relay lane as `openagents.data.access` demand instead of treating them as malformed compute jobs
- advertise the current data-vending request kind and coarse asset-family/delivery metadata on the provider NIP-89 handler when a seller profile is present
- surface incoming targeted data-access requests in the seller pane and seller status tools with explicit evaluation outcomes such as `no_published_asset`, `grant_required`, `scope_mismatch`, and `ready_for_payment_quote`
- generate seller-side Lightning invoices for matched targeted data requests, publish NIP-90 `payment-required` feedback, and track the request through `invoice_requested`, `publishing_feedback`, `awaiting_payment`, and `paid`
- prepare seller-side delivery drafts for paid targeted requests, accept the matched grant if needed, issue authoritative `DeliveryBundle` objects, and publish linked NIP-90 result events from the same seller flow
- let the seller revoke or expire access from the same flow, read the resulting `RevocationReceipt` back from kernel authority, and immediately reflect the terminal grant/delivery state into both panes
- record recent asset/grant/payment/delivery/revocation lifecycle entries in the read-only `Data Market` pane so operator-facing activity includes policy, counterparty, and receipt context
- open a dedicated `Data Buyer` desktop pane that derives a request draft from the visible market snapshot, selects an active asset/default offer, and publishes a targeted NIP-90 data-access request without widening into public discovery
- expose a checked-in `openagents.data.v1` proto package and use it for data-market authority mutation/read envelopes
- expose a combined `GET /v1/kernel/data/snapshot` read model that lets the desktop refresh the market view in one call instead of stitching four bare lists together
- package local files or directories into deterministic `listing-template.json`, `grant-template.json`, `packaging-manifest.json`, and `packaging-summary.json` artifacts through `scripts/autopilot/data_market_package.py`
- drive the same seller path through `autopilotctl data-market ...` without opening the visible UI window
- start a no-window Data Market runtime through `autopilot_headless_data_market`
- use the repo-owned `skills/autopilot-data-seller-cli` skill for shell-first packaging and publication discipline
- resolve a delivered local `DeliveryBundle` back into copied buyer-side files through `autopilotctl data-market consume-delivery`
- run `scripts/autopilot/headless-data-market-e2e.sh` to mechanically verify the local headless publish -> request -> delivery -> consume path
- run `scripts/autopilot/verify-data-market-cli-headless.sh` to mechanically verify the publish/consume path plus the critical lifecycle checks
- allow both seller request intake and buyer result tracking to run in a relay-only online posture without requiring a compute-ready local inference runtime
- normalize targeted buyer/seller identity matching across `npub` and raw hex Nostr pubkeys for the current NIP-90 targeted request/result flow

The starter authority slice is real in:

- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`

Authenticated HTTP mutation routes are live under:

- `POST /v1/kernel/data/assets`
- `POST /v1/kernel/data/grants`
- `POST /v1/kernel/data/grants/{grant_id}/accept`
- `POST /v1/kernel/data/grants/{grant_id}/deliveries`
- `POST /v1/kernel/data/grants/{grant_id}/revoke`

Authenticated HTTP read routes are live under:

- `GET /v1/kernel/data/assets`
- `GET /v1/kernel/data/assets/{asset_id}`
- `GET /v1/kernel/data/grants`
- `GET /v1/kernel/data/grants/{grant_id}`
- `GET /v1/kernel/data/deliveries`
- `GET /v1/kernel/data/deliveries/{delivery_bundle_id}`
- `GET /v1/kernel/data/revocations`
- `GET /v1/kernel/data/revocations/{revocation_id}`
- `GET /v1/kernel/data/snapshot`

## Local prototype or partial only

- richer provenance modeling beyond the starter permission and delivery objects
- private-data packaging and policy detail beyond the current starter shapes, though there is now a deterministic local packaging helper for turning a selected file or folder boundary into truthful draft inputs
- local or adjacent desktop concepts for context packaging, but not a
  generalized canonical data-market product surface
- broader public discovery and richer indexing ideas still live in docs rather
  than in a full market-facing catalog surface

## Local packaging helper

The current repo now includes a deterministic local packaging helper:

- `scripts/autopilot/data_market_package.py`

Its job is narrow:

- select a package boundary from one or more local files or directories
- compute a canonical bundle digest from a sorted manifest of file digests
- emit a stable `provenance_ref`
- write `listing-template.json`
- optionally write `grant-template.json`
- write `packaging-manifest.json` and `packaging-summary.json`

The helper is intentionally local and preparatory. It does not publish to
kernel authority by itself.

The generated outputs map directly into the current seller flow:

- `listing-template.json` -> `autopilotctl data-market draft-asset --file ...`
- `grant-template.json` -> `autopilotctl data-market draft-grant --file ...`
- `packaging-summary.json` -> local operator or agent audit surface for the
  chosen package boundary and resulting digests

## CLI and headless control

The repo now also includes a real shell-first control path:

- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/bin/autopilot_headless_data_market.rs`
- `skills/autopilot-data-seller-cli/`
- `docs/headless-data-market.md`

This path is intentionally not a second seller implementation.
It targets the same app-owned seller state and kernel mutation logic through the
typed desktop-control contract.

The current buyer-side consume step is local by design:

- `autopilotctl data-market consume-delivery` resolves the matching
  `DeliveryBundle`
- current headless verification brings the buyer online in a relay-only posture
  so targeted NIP-90 result events are actually observed before local consume
- it currently supports local `file://` and plain local-path `delivery_ref`
  values
- it copies the delivered payload and local manifest refs into a chosen output
  directory
- it is not yet a general remote blob retrieval client

## Not implemented yet

- a full transactional buyer-facing data market in Autopilot beyond the current narrow targeted-request pane
- fully integrated seller publication UX beyond the current asset/grant/payment/delivery/revocation control slice
- public discovery, listing, and richer pricing/comparison surfaces for data buyers
- payout and provider-economics flows specific to data access

## Current repo truth lives in

- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- [../economy-kernel.md](../economy-kernel.md)
- [../economy-kernel-proto.md](../economy-kernel-proto.md)

## Boundary notes

- data sells permissioned access to context
- compute sells bounded machine capacity
- labor sells machine work and outcome delivery
- liquidity moves value between rails and participants
- risk prices uncertainty, verification difficulty, and liability
