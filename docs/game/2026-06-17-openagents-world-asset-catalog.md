# OpenAgents World Asset Catalog And Provenance Record

Date: 2026-06-17
Status: Initial owned catalog and production-eligibility policy.
Source issue: OpenAgents #5275.
Source plan: `docs/game/2026-06-17-quick-3d-mmorpg-full-mechanics-harvest-plan.md`.

## Purpose

This document is the first owned OpenAgents world asset catalog for the
Tassadar/MMO direction. It records what visual asset classes OpenAgents should
own, what anchors and label offsets those assets must expose, and what
provenance is required before any imported or generated asset can appear on a
public launch surface.

No runtime assets are added by this issue. `/tassadar` remains procedural:
`three-effect` glyphs, rings, labels, HUD text, and row-backed world entities.
If `/tassadar` later consumes models, textures, icons, or authored materials,
they must be approved by this catalog or by a successor machine-readable
manifest that preserves the same fields.

## Eligibility States

| State | Meaning | Production use |
| --- | --- | --- |
| `approved_owned` | Created by OpenAgents, generated specifically for OpenAgents, or authored under an OpenAgents-owned contract with recorded rights. | Allowed on public launch surfaces. |
| `approved_public_domain` | Public-domain/CC0 asset with source URL, license file, and attribution preference recorded. | Allowed only after asset-specific provenance is copied into the catalog. Prefer owned variants for brand-critical surfaces. |
| `candidate_prototype` | Useful reference or prototype asset, but not production-approved. | Internal/local prototype only. Not allowed on `/tassadar` production. |
| `blocked_pending_review` | Source exists, but license, attribution, trademark, model-release, redistribution, or derivative-use terms need explicit review. | Not allowed. |
| `reference_only` | Used to study mechanics or style, not to import visual assets. | Not allowed as a runtime asset. |
| `rejected` | Deliberately excluded from OpenAgents runtime use. | Not allowed. |

## Required Provenance Fields

Every imported, commissioned, or generated asset record must include:

- `asset_id`: stable OpenAgents identifier.
- `version`: asset version or digest.
- `file_path`: owned repo path if the file is committed.
- `source_kind`: `owned`, `generated`, `commissioned`, `public_domain`,
  `open_source_reference`, or `third_party_candidate`.
- `source_url`: canonical upstream/source URL when applicable.
- `source_repo_ref`: repo and commit/tag when the source is a repository.
- `source_file_refs`: source file paths for extracted or adapted assets.
- `license`: exact license or rights instrument.
- `license_file`: local copied license path when a third-party asset is
  committed.
- `attribution`: required or preferred attribution text.
- `eligibility`: one of the catalog states above.
- `public_surface_allowed`: boolean.
- `allowed_surfaces`: for example `local_prototype`, `docs`, `tassadar`,
  `openagents_world`.
- `review_notes`: compatibility caveats and reviewer/date.
- `derived_from`: IDs of any source assets, prompts, sketches, or generated
  variants. Store prompt digests or public-safe prompt summaries when raw
  prompts contain private material.

## Current `/tassadar` Runtime Asset Ledger

These entries describe what the current production page uses. They are
procedural, not imported model/texture assets.

| Asset id | Runtime form | Source | Eligibility | Notes |
| --- | --- | --- | --- | --- |
| `oa_proc_run_core_ring_v1` | Three.js procedural rings and center glyph. | `@openagentsinc/three-effect` `trainingRun.ts`. | `approved_owned` | Represents the canonical run entity only. |
| `oa_proc_pylon_station_marker_v1` | Three.js procedural station marker. | `@openagentsinc/three-effect` entity layer plus OpenAgents SpacetimeDB `pylon_station` rows. | `approved_owned` | Draw only when backed by a real pylon station row/ref. |
| `oa_proc_agent_avatar_marker_v1` | Three.js procedural avatar marker. | `@openagentsinc/three-effect` entity layer plus `agent_avatar` and `avatar_position` rows. | `approved_owned` | Draw only when both identity and current position rows exist. |
| `oa_proc_nameplate_text_v1` | Canvas/text label texture or CSS/HUD text. | `@openagentsinc/three-effect` text and billboard primitives. | `approved_owned` | Must face the camera in perspective modes. |
| `oa_proc_status_hud_v1` | Text-only overlay legend. | `apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts`. | `approved_owned` | Lifecycle/status counters are HUD context, not spatial nodes. |

## Target Avatar Model Types

These are owned design targets. They are not yet model files.

| Asset id | Purpose | Required anchors | Label offset | Material policy | Eligibility |
| --- | --- | --- | --- | --- | --- |
| `oa_avatar_viewer_operator_v1` | Local human/operator body in first/third-person world mode. | `anchor.root`, `anchor.head`, `anchor.nameplate`, `anchor.hand.left`, `anchor.hand.right`, `anchor.back`, `anchor.feet`. | `anchor.nameplate` plus 0.35m Y. | Muted operator material; no combat silhouette on training pages. | `approved_owned` once authored. |
| `oa_avatar_pylon_agent_v1` | Visible pylon agent/avatar tied to public pylon rows. | `anchor.root`, `anchor.head`, `anchor.nameplate`, `anchor.capability`, `anchor.status`. | `anchor.nameplate` plus 0.3m Y. | Role color comes from row-backed pylon/agent status. | `approved_owned` once authored. |
| `oa_avatar_service_agent_v1` | Bridge/service actor for public-safe system events. | `anchor.root`, `anchor.nameplate`, `anchor.status`. | `anchor.nameplate` plus 0.25m Y. | Distinct service material; must not imply human presence. | `approved_owned` once authored. |
| `oa_avatar_guest_v1` | Anonymous/public visitor body. | `anchor.root`, `anchor.head`, `anchor.nameplate`. | `anchor.nameplate` plus 0.3m Y. | Low-detail neutral material; local-only until joined region row exists. | `approved_owned` once authored. |

## Target Station And Prop Types

These are OpenAgents-specific props to commission or generate before public
visual expansion.

| Asset id | Purpose | Required anchors | Label offset | Runtime data boundary | Eligibility |
| --- | --- | --- | --- | --- | --- |
| `oa_prop_pylon_station_v1` | Physical station for a row-backed pylon in a run region. | `anchor.root`, `anchor.nameplate`, `anchor.visitor_count`, `anchor.proof_slot`, `anchor.chat`. | 0.55m above station. | Needs `pylon_station` row and public `home_pylon_ref`. | `approved_owned` once authored. |
| `oa_prop_proof_gate_v1` | Inspectable proof/verdict object. | `anchor.root`, `anchor.nameplate`, `anchor.proof_ref`, `anchor.event_burst`. | 0.45m above object. | Needs public proof/challenge/receipt ref. | `approved_owned` once authored. |
| `oa_prop_settlement_terminal_v1` | Settlement/receipt inspection terminal. | `anchor.root`, `anchor.nameplate`, `anchor.receipt_ref`, `anchor.amount_badge`. | 0.5m above terminal. | Needs public settlement/receipt ref; no fake payout motion. | `approved_owned` once authored. |
| `oa_prop_registry_marker_v1` | Product-promise/registry marker. | `anchor.root`, `anchor.nameplate`, `anchor.status_badge`. | 0.45m above marker. | Needs product-promise registry ref. | `approved_owned` once authored. |
| `oa_prop_run_core_v1` | Center object for a canonical run. | `anchor.root`, `anchor.nameplate`, `anchor.staleness`, `anchor.selection`. | 0.65m above core. | Needs canonical run ref. | `approved_owned` once authored. |

## Role And Status Materials

Materials should remain restrained on `/tassadar`: they are status coding, not
decorative noise.

| Material id | Meaning | Suggested palette | Data requirement |
| --- | --- | --- | --- |
| `oa_mat_status_active_v1` | Active/current row or run. | white plus soft mint accent. | Current state row or live-at-read summary. |
| `oa_mat_status_qualified_v1` | Passed a public gate. | mint/green accent. | Public gate/proof row. |
| `oa_mat_status_pending_v1` | Planned or waiting. | muted blue/gray. | Public pending state row. |
| `oa_mat_status_stale_v1` | Stale projection or expired presence. | amber muted accent. | Staleness contract or expiry row. |
| `oa_mat_status_blocked_v1` | Real blocker/rejected state. | red muted accent. | Public blocker/rejection ref. |
| `oa_mat_status_unknown_v1` | Missing/unknown state. | low-opacity neutral. | Explicit unknown/missing state, never inferred drama. |

## Capability And Equipment Adornments

The first world assets should represent capabilities, not fantasy weapons.

| Asset id | Purpose | Anchor | Data requirement | Eligibility |
| --- | --- | --- | --- | --- |
| `oa_adorn_compute_v1` | Agent has compute/runtime capability. | `anchor.capability` or `anchor.back`. | Public capability row/ref. | `approved_owned` once authored. |
| `oa_adorn_verifier_v1` | Agent can verify proof/work. | `anchor.capability`. | Public verifier capability row/ref. | `approved_owned` once authored. |
| `oa_adorn_wallet_ready_v1` | Wallet/payment path ready. | `anchor.status`. | Public wallet readiness row/ref. | `approved_owned` once authored. |
| `oa_adorn_training_contributor_v1` | Contributed model progress. | `anchor.status`. | Public training contribution row/ref. | `approved_owned` once authored. |
| `oa_adorn_blocker_v1` | Real blocker exists. | `anchor.status`. | Public blocker/ref. | `approved_owned` once authored. |

## Quick 3D MMORPG Reference Asset Provenance

The reference repository is useful source material, but its asset set is mixed.
None of these assets are production-approved for OpenAgents by this document.

| Reference set | Source evidence | License/provenance state | OpenAgents eligibility |
| --- | --- | --- | --- |
| Quick source code mechanics | `projects/repos/Quick_3D_MMORPG/LICENSE` says MIT, copyright 2021 simondevyoutube; `client/LICENSE` says MIT, copyright 2020 simondevyoutube. | Code can be studied and reimplemented with attribution preserved where copied. | `reference_only` for mechanics; do not vendor the app. |
| Quaternius trees/nature/nature2/weapons | `client/resources/{trees,nature,nature2,weapons}/License.txt` says Quaternius and CC0 1.0 Universal. | Asset-specific provenance can be recorded. Credit is optional but preferred. | `candidate_prototype`; may become `approved_public_domain` only after copied license/provenance per asset. |
| Mixamo character GLBs | `client/resources/characters/readme.txt` says free asset from `https://www.mixamo.com/`. | Broad source note only. Character/model terms, redistribution, and derivative-use compatibility need review. | `blocked_pending_review`; no production use. |
| Game-icons UI/weapon icons | `client/resources/icons/readme.txt` says free asset from `https://game-icons.net/`. | Broad source note only. Each icon needs exact author/license/attribution review. | `blocked_pending_review`; no production use. |
| Terrain textures | `client/resources/terrain/README.txt` says most textures came from `freepbr.com` or an OpenGameArt 36-ground-texture page and were resized. | Broad source note only. Each texture needs exact source URL, license, attribution, and derivative/redistribution review. | `blocked_pending_review`; no production use. |

## Public Surface Rules

- `/tassadar` may consume `approved_owned` procedural assets now.
- `/tassadar` may consume authored GLB/texture assets only after this catalog or
  a successor manifest marks them `approved_owned` or
  `approved_public_domain`.
- Mixamo, game-icons, and terrain texture references must not appear on
  production `/tassadar` until explicit compatibility review changes their
  status.
- Quaternius CC0 assets can be useful for local prototypes, but public launch
  should prefer OpenAgents-owned/generated station, agent, proof, settlement,
  and registry props.
- An asset cannot create a fake product claim. Status materials, animation
  states, badges, beams, bursts, and motion must still be backed by public rows
  or source refs.
- Store cataloged third-party licenses beside imported assets. Do not depend on
  `projects/repos` paths as the runtime source of truth.

## Next Manifest Shape

When real files are added, mirror this document in a machine-readable manifest,
for example:

```json
{
  "asset_id": "oa_prop_pylon_station_v1",
  "version": "sha256:...",
  "file_path": "apps/openagents.com/apps/web/public/world/oa_prop_pylon_station_v1.glb",
  "source_kind": "owned",
  "source_url": null,
  "source_repo_ref": "OpenAgentsInc/openagents@...",
  "source_file_refs": [],
  "license": "OpenAgents-owned launch asset",
  "license_file": null,
  "attribution": "OpenAgents",
  "eligibility": "approved_owned",
  "public_surface_allowed": true,
  "allowed_surfaces": ["tassadar", "openagents_world"],
  "review_notes": "Approved for row-backed pylon_station rendering only.",
  "derived_from": []
}
```

Until that manifest exists, this document is the authoritative catalog.
