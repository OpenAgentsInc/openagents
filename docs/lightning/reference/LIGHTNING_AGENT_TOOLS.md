# Lightning Agent Tools Integration

Integration plan for [Lightning Labs’ Lightning agent tools](https://lightning.engineering/posts/2026-02-11-ln-agent-tools/) (February 2026) with OpenAgents. This doc is **planning and documentation only**; no code changes are specified here.

## Summary of the Lightning Labs Release

Lightning Labs open-sourced a set of tools that let AI agents use the Lightning Network natively: pay for APIs, host paid endpoints, and run node operations with scoped credentials and key isolation.

- **Repo:** [github.com/lightninglabs/lightning-agent-tools](https://github.com/lightninglabs/lightning-agent-tools)
- **Article:** [The Agents Are Here and They Want to Transact](https://lightning.engineering/posts/2026-02-11-ln-agent-tools/) (2026-02-11)

### Core Components

| Component | Purpose |
|-----------|--------|
| **L402** | Protocol for Lightning machine-payment auth. Uses HTTP 402 "Payment Required", a Lightning invoice, and a macaroon. Agent pays invoice → gets preimage → presents preimage + macaroon to access the resource. No signup, API key, or identity. |
| **lnget** | L402-aware CLI HTTP client (like wget/curl). On 402, it parses the challenge, pays the invoice via the configured Lightning backend, caches the auth token, and retries. Agents that can run shell commands can use it to consume L402-gated APIs. |
| **lnd / lnget skills** | Composable agent skills: run a Lightning node, configure lnget, pay for L402 APIs. Installable via npx, Claude Code plugin marketplace, or ClawHub. |
| **Remote signer** | LND remote-signer setup: private keys live on a signer machine; the agent runs a watch-only node and delegates signing over gRPC. Compromise of the agent machine does not expose keys. |
| **Macaroon bakery** | Skills to bake scoped credentials (pay-only, invoice-only, read-only, channel-admin, signer-only). Enables least-privilege spending and caps. |
| **Aperture** | Lightning Labs’ L402 reverse proxy. Backend APIs sit behind Aperture; it handles 402 negotiation, dynamic pricing, and payment. Lets any developer expose pay-per-use endpoints without implementing Lightning. |
| **MCP server** | Model Context Protocol server over Lightning Node Connect (LNC): 18 read-only tools for balances, channels, invoices, payments, network graph. Pairing phrase only; no credentials on disk. |
| **Commerce meta-skill** | End-to-end buyer/seller workflows: stand up node, host paid API (Aperture), buy from another agent’s API (lnget), with natural-language prompts. |

### lnget Details (Relevant for Agent Integration)

- **Usage:** `lnget https://api.example.com/premium-data.json` (optional `--max-cost` for per-request cap).
- **Backends:** Direct gRPC to local lnd; Lightning Node Connect (pairing phrase); or embedded Neutrino for experiments.
- **Caching:** Tokens cached per domain; subsequent requests reuse without extra payment.
- **Guardrails:** `--max-cost` plus node-level macaroon caps give budget control for autonomous agents.

### Security Model

- **Recommended:** Remote signer (keys off agent machine).
- **Dev/test:** Standalone lnd with keys on disk, restrictive permissions.
- **Read-only:** MCP over LNC, ephemeral session, no credentials written to disk.

---

## How This Fits OpenAgents

### Payment Rails and Receipts

- OpenAgents already treats **Lightning** as a rail (see [GLOSSARY.md](../../GLOSSARY.md): Rail, AssetId, payment proofs).
- [ADR-0013](../../adr/ADR-0013-receipt-schema-payment-proofs.md) defines `payment_proof` with type `lightning_preimage`. L402 payments produce exactly that: pay invoice → get preimage → use for auth. Session receipts can record L402 spends as payment receipt entries with `rail`, `asset_id`, `amount_msats`, and `payment_proof: { type: "lightning_preimage", value: "..." }`.
- **No change to receipt schema** is required; L402 fits the existing Lightning proof type.

### Lanes and Providers

- Lanes (Local, Cloud, Swarm) describe **inference/execution** routing. Payment is orthogonal: Swarm jobs can be paid in sats; **consuming** paid APIs (e.g. via lnget) is an agent capability, not a new lane.
- L402/lnget enables:
  - **Agent as buyer:** Pay for L402-gated APIs (data, compute, services) using lnget (or a tool that wraps it).
  - **Agent as seller:** Expose pay-per-use endpoints behind Aperture; payments settle over Lightning and align with existing receipt semantics.

### Tools and Runtime

- Today, tools are defined in the Laravel app (`apps/openagents.com`, tool contracts + handlers). Adding an **lnget-style capability** would mean either:
  - A first-class tool (e.g. `lnget_fetch`) that invokes the lnget CLI when available and returns response body + receipt-relevant data (e.g. amount paid, preimage for logging), or
  - Documentation and prompts so that agents running in environments where they can execute shell (e.g. OpenClaw, Codex, desktop) use the lightning-agent-tools skills and lnget directly.
- Runtime invariants (ADR-0007): any new tool must have a JSON schema, validation before execution, and receipt/replay events. If the tool runs lnget, receipt should include payment_proof when a payment occurred.

### Documentation and Glossary

- **GLOSSARY.md** should define: **L402**, **lnget**, **Macaroon** (in our context: scoped Lightning credentials), **Aperture**, and optionally **Lightning Node Connect (LNC)** so all docs use consistent terms.
- This doc and the integration plan live under `docs/lightning/reference/` and are the single place for “how we integrate Lightning agent tools.”

---

## Integration Plan (No Code; Phases Only)

### Phase 1: Documentation and Vocabulary

- Add Lightning agent tools terms to [GLOSSARY.md](../../GLOSSARY.md): L402, lnget, Macaroon, Aperture, LNC (brief).
- Keep this doc as the canonical integration plan and reference to the Lightning Labs repo and article.
- In any payment/receipt docs, state that L402 payments are recorded as `lightning_preimage` and that agent-facing payment flows can use lnget/Aperture as the implementation.

### Phase 2: Receipt and Observability Alignment

- Confirm that when an agent pays via L402 (whether through a future lnget tool or external lnget usage), session receipts can attach payment receipt entries with existing fields (rail, asset_id, amount_msats, payment_proof).
- Document in ARTIFACTS/receipt docs (or equivalent in current stack) that L402 spend is a first-class payment receipt type using `lightning_preimage`.
- No schema change; documentation and runbook only.

### Phase 3: Capability Surface (When Implementing)

- **Option A – First-class tool:** Add an `lnget_fetch` (or similar) tool in the worker: params (e.g. url, optional maxCostSats), schema, handler that runs lnget when configured, returns body + payment metadata, and emits receipt with payment_proof when a payment occurred. Requires lnget and Lightning backend (lnd/LNC/Neutrino) to be available in the deployment environment.
- **Option B – Agent-facing docs only:** Document how to install lightning-agent-tools skills and use lnget in environments where the agent can run shell (OpenClaw, Codex, etc.). OpenAgents docs and prompts direct users/agents to the official repo and Aperture for hosting.
- **Option C – Hybrid:** Document Option B for maximum compatibility; add Option A in deployments that can run lnget (e.g. self-hosted or desktop) and want first-class receipt integration.

### Phase 4: Hosting (Optional)

- Document how to put an OpenAgents-backed or third-party API behind Aperture for L402 pay-per-use, so agents can sell as well as buy. No code in this repo required; runbook and architecture only.

### Phase 5: Stale Doc Cleanup (Ongoing)

- When touching docs that still reference Rust/crates, update or add a pointer to [RUST_DOCS_ARCHIVE_2026-02-11.md](../../RUST_DOCS_ARCHIVE_2026-02-11.md) and [PROJECT_OVERVIEW.md](../../PROJECT_OVERVIEW.md) so readers know the active stack is web/TypeScript/Effect.

---

## References

- [Lightning Labs – The Agents Are Here and They Want to Transact](https://lightning.engineering/posts/2026-02-11-ln-agent-tools/) (2026-02-11)
- [lightning-agent-tools repo](https://github.com/lightninglabs/lightning-agent-tools)
- [GLOSSARY.md](../../GLOSSARY.md) — Rail, AssetId, payment proofs
- [ADR-0013: Receipt schema, payment proofs](../../adr/ADR-0013-receipt-schema-payment-proofs.md)
- [ADR-0007: Tool execution contract](../../adr/ADR-0007-tool-execution-contract.md)
- [PROJECT_OVERVIEW.md](../../PROJECT_OVERVIEW.md) — Active codebase (web-first)
