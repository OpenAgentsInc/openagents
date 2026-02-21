# Protocol Docs

This directory is the system-of-record for protocol contracts and drafts used by OpenAgents.

Entry points:
- `docs/protocol/PROTOCOL_SURFACE.md` (canonical contract surface: receipts, hashes, ids)
- `docs/protocol/reasons/runtime-policy-reason-codes.v1.json` (canonical runtime policy reason taxonomy)
- `docs/protocol/LAYER0_PROTOBUF_MAPPING.md` (proto3 Layer-0 mapping guidance for JSON/SSE)
- `docs/protocol/OA_SYNC_WS_MAPPING.md` (Khala sync proto-to-WS mapping)
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md` (control + Khala version negotiation and support-window policy)
- `docs/protocol/fixtures/khala-frame-v1.json` (Khala envelope replay/live/stale-cursor fixtures)
- `docs/protocol/fixtures/runtime-orchestration-v1.json` (runtime lifecycle/receipt/replay fixture set)
- `docs/protocol/fixtures/control-auth-session-v1.json` (control-plane auth/session/scope/sync-token fixture set)
- `docs/protocol/fixtures/codex-worker-events-v1.json` (codex worker/notification/replay fixture set)
- `docs/protocol/fixtures/compatibility-negotiation-v1.json` (supported/unsupported version handshake matrix fixture)
- `docs/protocol/control-auth-session-v1.md` (WorkOS/control authority split + auth/session compatibility notes)
- `docs/protocol/codex-worker-events-v1.md` (codex worker/event/auth compatibility notes)
- `proto/openagents/control/v1/auth.proto` (control-plane auth/session/scope schema authority)
- `proto/openagents/runtime/v1/orchestration.proto` (runtime orchestration schema authority)
- `proto/openagents/codex/v1/events.proto` (codex worker/event stream schema authority)
- `proto/openagents/codex/v1/workers.proto` (codex worker summary/snapshot schema authority)
- `proto/openagents/codex/v1/auth.proto` (codex auth hydration/state schema authority)
- `proto/openagents/lightning/v1/control_plane.proto` (Lightning control-plane schema authority)
- `crates/openagents-proto/` (Rust proto wire crate generated from `proto/`)
- `proto/README.md` (proto package layout + generation verification policy)
- `proto/PACKAGE_MAP.md` (Rust-era proto package ownership + placement rules)
- `docs/protocol/extensions/` (base extension manifest contract + specialization links)
- `docs/protocol/comms/` (comms tool-pack contracts and integration manifest schema)
- `docs/protocol/coding/` (coding tool-pack contracts and GitHub integration manifest schema)
- `proto/` (canonical proto contracts for generated language bindings)
- `crates/nostr/nips/` (draft NIPs: SA, AC — canonical location for links)

Related:
- `docs/adr/` (ADRs define architectural intent and compatibility rules)
- `docs/GLOSSARY.md` (canonical terminology)

## Governance and Ownership

- Review ownership for proto and protocol surfaces is enforced by `.github/CODEOWNERS`.
- Contract-governed paths include:
  - `proto/**`
  - `docs/protocol/**`
  - `apps/runtime/docs/*CONTRACT*.md`
  - `apps/runtime/lib/openagents_runtime/contracts/**`
  - `apps/runtime/lib/mix/tasks/runtime.contract.check.ex`
- Branch protection follow-up:
  - Ensure required code-owner reviews are enabled for `main` in repository settings.
