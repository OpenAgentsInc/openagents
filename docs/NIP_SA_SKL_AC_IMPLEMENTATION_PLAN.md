# NIP-SA, NIP-SKL, NIP-AC Implementation Plan (Current State + Follow-On)

## 1. Goal

Maintain an accurate execution plan for SA/SKL/AC that reflects current in-repo implementation reality and clearly separates:

- what is already implemented in `crates/nostr/core`,
- what is integrated in `apps/autopilot-desktop`,
- and what remains for MVP hardening.

This document is aligned to:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PROTOCOL_SURFACE.md`

## 2. Current State Snapshot (2026-02-27)

## 2.1 `crates/nostr/core` status

Implemented protocol modules:

- SA: `crates/nostr/core/src/nip_sa/*`
- SKL: `crates/nostr/core/src/nip_skl/*`
  - `manifest.rs` (`33400`)
  - `version_log.rs` (`33401`)
  - `discovery.rs` (optional `5390/6390` profile)
  - `trust.rs` (NIP-32 based trust evaluation)
  - `revocation.rs` (NIP-09 same-pubkey authority semantics)
  - `yaml_derivation.rs` (deterministic SKILL payload hashing/derivation)
- AC: `crates/nostr/core/src/nip_ac/*`
  - intent/offer/envelope/spend/settlement/default lifecycle
  - `scope_hash.rs` SKL scope linkage and constraints hashing

Export surface:

- `crates/nostr/core/src/lib.rs` currently exports `nip_sa`, `nip_skl`, and `nip_ac`.

## 2.2 Test coverage status

Protocol and integration coverage exists and is active, including:

- `crates/nostr/core/tests/nip_sa_exports.rs`
- `crates/nostr/core/tests/nip_sa_e2e.rs`
- `crates/nostr/core/tests/nip_sa_skl_ac_integration_matrix.rs`
- `crates/nostr/core/tests/nip_ac_skill_linkage.rs`
- `crates/nostr/core/tests/nip44_integration.rs`
- `crates/nostr/core/tests/nip90_integration.rs`
- module-level tests under `nip_skl/*` and `nip_ac/*`

## 2.3 Desktop integration status

`apps/autopilot-desktop` includes typed SA/SKL/AC command lanes and pane-level state mapping:

- runtime lanes: `src/runtime_lanes.rs`
- reducer wiring: `src/input/reducers/skl.rs` and lane peers
- pane descriptors and renderer wiring in pane system/renderer modules

Current lane behavior remains simulation-first for relay transport, but now includes local SKL registry integration:

- local registry discovery + manifest derivation utility:
  - `apps/autopilot-desktop/src/skills_registry.rs`
- SKL manifest/version commands validate against local project skills:
  - single-skill: `skills/<project>/SKILL.md`
  - multi-skill: `skills/<project>/<skill-name>/SKILL.md`

## 2.4 Local skills registry status

Implemented:

- root registry contract: `skills/README.md`
- validation script: `scripts/skills/validate_registry.sh`
- lint gate integration via `scripts/lint/clippy-regression-check.sh`
- first concrete skills:
  - `skills/mezo`
  - `skills/moneydevkit`

## 3. What Remains (MVP Hardening)

## 3.1 Runtime transport authority

Remaining work:

- Replace simulated SA/SKL/AC lane event-id generation with relay-backed publish/subscribe flows.
- Ensure lane snapshots are sourced from authoritative event streams, not synthetic transitions.

## 3.2 SKL productionization

Remaining work:

- Promote local skill derivation flow to real publish path (signed `33400/33401` events over relay lane).
- Add deterministic error and recovery UX for malformed skills, publish failures, and trust/revocation conflicts.

## 3.3 AC settlement + wallet lane linkage

Remaining work:

- Tighten end-to-end linkage between AC settlement events, NIP-90 outcomes, and wallet-confirmed payout states in desktop runtime.

## 3.4 Replay/idempotency + incident posture

Remaining work:

- Expand replay/idempotency regression tests around reconnect and stale cursor transitions in desktop state lanes.
- Keep runbook alignment with:
  - `docs/NIP_SA_SKL_AC_TEST_MATRIX_RUNBOOK.md`

## 4. Follow-On Phases

## Phase A: Transport Promotion

1. Replace SKL lane synthetic publish/search responses with relay-backed flows.
2. Apply same promotion pattern across SA and AC lanes.
3. Keep typed command/response contracts unchanged where possible.

Exit criteria:

- Desktop lane snapshots reflect relay truth for SA/SKL/AC events.

## Phase B: SKL Registry-to-Nostr Publish Path

1. Bind local `skills/` entries to real `33400/33401` publish commands.
2. Persist manifest/version references in pane state from real events.
3. Enforce trust/revocation checks against fetched events before fulfillment actions.

Exit criteria:

- A local skill can be discovered, derived, published, and referenced through real SKL events.

## Phase C: Settlement and Wallet Coherency

1. Ensure AC settlement/default outcomes are cross-linked to NIP-90 and wallet state.
2. Make payout-success UX contingent on authoritative wallet confirmation.

Exit criteria:

- No payout/settlement success state appears without real underlying confirmation.

## Phase D: Ops and Documentation Maintenance

1. Keep `docs/PROTOCOL_SURFACE.md` and this plan synchronized with code.
2. Update test matrix/runbook references whenever lane behavior changes materially.

Exit criteria:

- No stale “not implemented” claims for SA/SKL/AC remain in active docs.

## 5. Acceptance Criteria

This plan is considered current when:

1. Module reality in `crates/nostr/core` matches statements above.
2. Desktop lane authority source (simulated vs relay-backed) is explicitly stated and accurate.
3. `docs/PROTOCOL_SURFACE.md`, this plan, and test matrix docs are mutually consistent.

## 6. Non-Goals

- Reintroducing broad archive pull plans into this pruned MVP repo.
- Defining new SA/SKL/AC kind ranges outside the locked protocol surface without explicit protocol-surface update.
