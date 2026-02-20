# Protocol Docs

This directory is the system-of-record for protocol contracts and drafts used by OpenAgents.

Entry points:
- `docs/protocol/PROTOCOL_SURFACE.md` (canonical contract surface: receipts, hashes, ids)
- `docs/protocol/reasons/runtime-policy-reason-codes.v1.json` (canonical runtime policy reason taxonomy)
- `docs/protocol/LAYER0_PROTOBUF_MAPPING.md` (proto3 Layer-0 mapping guidance for JSON/SSE)
- `docs/protocol/OA_SYNC_WS_MAPPING.md` (Khala sync proto-to-WS mapping)
- `proto/README.md` (proto package layout + generation verification policy)
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
  - `apps/openagents-runtime/docs/*CONTRACT*.md`
  - `apps/openagents-runtime/lib/openagents_runtime/contracts/**`
  - `apps/openagents-runtime/lib/mix/tasks/runtime.contract.check.ex`
- Branch protection follow-up:
  - Ensure required code-owner reviews are enabled for `main` in repository settings.
