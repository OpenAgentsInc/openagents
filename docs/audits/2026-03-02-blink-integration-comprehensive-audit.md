# 2026-03-02 Comprehensive Blink Integration Audit

## Scope

This audit covers the full Blink integration surface in OpenAgents as of March 2, 2026:

- `skills/blink` skill package (scripts, references, operational docs).
- `apps/autopilot-desktop` integration points (skill discovery/attachment, swap tools, goal evidence, credentials interaction, live harness).
- Runtime behavior for BTC wallet, stablesat USD wallet, invoice/payment paths, and BTC<->USD swaps.
- Operational/testing coverage and current production-readiness gaps.

Primary alignment references:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`

## Method

- Static review of Blink skill docs/scripts and app integration code.
- Verification of swap/tool policy and receipt persistence paths.
- Targeted test execution:
  - `cargo test -p autopilot-desktop swap -- --nocapture`
  - Result: 21 swap/Blink-related tests passed (0 failed).

## Executive Summary

Blink integration is substantial and now first-class for autonomous earnings workflows: skill attachment, swap quote/execute tools, goal-scoped policy checks, and quote/execution audit receipts are all present. The system is using real Blink infrastructure (no mock swap backend in the main runtime path).

Current maturity is good but not fully production-safe due to three critical/near-critical gaps:

1. `openagents_swap_execute` quote-id consistency can fail by design because execution script currently generates a new quote id.
2. Tool-bridge Blink script execution does not consume app keyring credentials directly; it depends on process env/`~/.profile`.
3. Stablesats quote adapter is modeled but has no concrete runtime client implementation; production behavior is Blink-infrastructure-only.

## Integration Topology

## 1) Skill Package (`skills/blink`)

Core skill definition and operator guidance are comprehensive:

- Skill contract and operations: `skills/blink/SKILL.md`
- References:
  - `skills/blink/references/blink-api-and-auth.md`
  - `skills/blink/references/invoice-lifecycle.md`
  - `skills/blink/references/payment-operations.md`
  - `skills/blink/references/swap-operations.md`

Script surface includes:

- Read/account: `balance.js`, `account_info.js`, `price.js`, `transactions.js`
- Receive: `create_invoice.js`, `create_invoice_usd.js`, `check_invoice.js`, `subscribe_invoice.js`, `qr_invoice.js`
- Send: `pay_invoice.js`, `pay_lnaddress.js`, `pay_lnurl.js`, `fee_probe.js`
- Swap: `swap_quote.js`, `swap_execute.js`, `_swap_common.js`
- Shared client: `_blink_client.js`

## 2) App-Level Integration (`apps/autopilot-desktop`)

Blink is integrated into Autopilot flow at multiple layers:

- Skill discovery + attachment:
  - `input/reducers/skl.rs`
  - `input/reducers/codex.rs`
  - `input/actions.rs`
  - `state/goal_skill_resolver.rs` (Blink prioritized for swap objectives)
- Controlled swap tool API:
  - `openagents_dynamic_tools.rs` (`openagents_swap_quote`, `openagents_swap_execute`)
- Runtime execution bridge:
  - `input/tool_bridge.rs`
- Goal state/policy/evidence:
  - `state/swap_contract.rs`
  - `state/swap_quote_adapter.rs`
  - `state/autopilot_goals.rs`
  - `state/goal_loop_executor.rs`
  - `state/earnings_gate.rs`
- Live integration harness:
  - `bin/codex_live_harness.rs`
- Credential model:
  - `credentials.rs`
  - `app_state.rs` (`sync_credentials_runtime`)
  - `docs/CREDENTIALS.md`

## 3) Runtime Data Flow (Current)

Autonomous swap flow currently runs as:

1. Agent calls `openagents_swap_quote` (goal_id/request_id/direction/amount/unit).
2. Tool bridge validates policy and unit-direction constraints.
3. Tool bridge executes `skills/blink/scripts/swap_quote.js` via `node`.
4. Parsed quote is revalidated against goal swap policy.
5. Quote audit receipt is persisted to goal store with command provenance.
6. Agent calls `openagents_swap_execute` with `goal_id` + `quote_id`.
7. Tool bridge looks up quote audit, executes `swap_execute.js`, parses response.
8. Execution receipt is persisted, timeline event emitted.

This is a real network path through Blink scripts, not a simulated swap provider.

## Detailed Findings

## A) Skill and Wallet Capability Coverage

Status: Strong

- Blink skill exposes full wallet basics (balances, BTC/USD invoices, payments, fee probes, transaction views).
- Invoice lifecycle is explicitly documented as two-phase output (invoice creation then resolution), including USD expiry behavior.
- Swap docs now include:
  - direction/path mapping,
  - deterministic output schema,
  - fee/rounding semantics,
  - operational retry guidance.

Assessment: The skill package itself is production-usable for manual and scripted wallet operations.

## B) First-Class Swap Tooling in Autopilot

Status: Strong with critical correctness caveat

What is implemented well:

- Dynamic tool schemas are explicit and constrained.
- Tool bridge enforces direction/unit constraints for controlled path:
  - BTC->USD must be sats input.
  - USD->BTC must be cents input.
- Goal swap policy gate is checked pre-quote and post-quote.
- Quote and execution receipts are persisted with bounded retention and command provenance.
- Timeline events emitted for observability.

Critical correctness issue:

- `openagents_swap_execute` expects Blink execution payload quote id to match requested quote id.
- `swap_execute.js` currently creates a fresh quote internally (`estimateSwapQuote`) and emits a new random `quoteId`.
- Result: quote-id mismatch is structurally likely, making strict execute path unreliable.

Impact:

- Autonomous swap execution can fail even when Blink settlement path itself is healthy.
- Goal loops may record false negatives around swap execution.

## C) Stablesats Support Model

Status: Partially implemented

Implemented:

- Domain contract and adapter semantics exist in `state/swap_quote_adapter.rs`:
  - `GetQuoteToBuyUsd` / `GetQuoteToSellUsd` style modeling,
  - immediate-execution acceptance logic,
  - fallback to Blink quote with audit trail.

Not implemented:

- No concrete runtime `StablesatsQuoteClient` implementation outside tests.
- No live gRPC/HTTP connector from Autopilot runtime to stablesats quote service.

Current effective behavior:

- Production runtime is Blink infrastructure path for swaps.
- Stablesats adapter remains a contract/fallback abstraction layer.

## D) Credential and Secret Plumbing

Status: Medium with high-risk operational gap

What works:

- Credentials system supports keyring-backed `BLINK_API_KEY` and `BLINK_API_URL`.
- Scope model includes SKILLS/CODEX/GLOBAL and runtime sync logic.

Gap:

- Tool bridge runs Blink scripts via raw `node` subprocess without injecting keyring-resolved Blink credentials directly into that process.
- `_blink_client.js` falls back to process env and `~/.profile`, not Autopilot keyring APIs.

Impact:

- A user can store Blink creds in the app credentials pane and still have swap tools fail if env/profile is not populated.
- This is an operator surprise and reliability risk.

Contrast:

- `codex_live_harness` has stronger Blink env resolution (env + keychain fallback) than runtime tool bridge path.

## E) Fee and Settlement Semantics

Status: Correctly documented; economically subtle

- Swap quote fields currently report `feeSats=0`, `feeBps=0`, `slippageBps=0`.
- Effective cost often appears as integer rounding spread between quote and post-settlement deltas (commonly 1 sat/1 cent effects depending on direction/size).
- `skills/blink/references/swap-operations.md` now documents effective-cost interpretation and formulas.

Assessment: Documentation is aligned with observed behavior; implementation is clear that explicit fee fields are not the full economic picture.

## F) Testing and Verification

Status: Good unit coverage on swap logic

Verified by local run:

- `cargo test -p autopilot-desktop swap -- --nocapture`
- Swap policy, parser, adapter fallback, receipt persistence, and related tests passed.

Gaps in current test profile:

- No end-to-end integration test proving `openagents_swap_quote` -> `openagents_swap_execute` quote-id continuity against live script output contract.
- No runtime test ensuring keyring-only Blink credentials are honored by tool-bridge swap execution path.

## Risk Register (Ranked)

1. Critical: Swap execute quote-id mismatch risk between tool bridge and script output model.
2. High: Tool-bridge Blink scripts not first-class wired to app keyring credential resolution.
3. High: Stablesats quote adapter lacks concrete live client wiring.
4. Medium: No strict freshness guard at execute time beyond receipt lookup (execution relies on script behavior and may not enforce original TTL semantics deterministically).
5. Medium: Runtime Blink swap path depends on local `node` availability and environment hygiene.

## Recommended Remediations

1. Fix quote-id contract:
- Update `swap_execute.js` + tool bridge contract so execution uses the previously accepted quote identity and terms deterministically.
- Alternative: explicitly remove quote-id equality enforcement and store execution quote linkage differently, but preserve audit integrity.

2. Inject Blink credentials into tool-bridge subprocesses:
- Resolve Blink vars from `CredentialRepository` in runtime state and pass env overrides when launching Blink scripts.
- Make keyring-backed credentials authoritative for Autopilot tool execution.

3. Decide and codify stablesats strategy:
- Either:
  - implement a real `StablesatsQuoteClient`, or
  - formally mark Blink infrastructure as the sole production swap provider and simplify adapter complexity.

4. Add swap-path integration tests:
- E2E test for quote -> execute happy path with contract invariants.
- E2E test for keyring-only credential scenario.

5. Add explicit execute-time quote freshness behavior:
- Reject expired quote IDs before launch or force requote with explicit state transition.

## MVP / Ownership Alignment

- MVP fit: strong. Blink now underpins core money movement and swap behavior needed for "earn bitcoin on autopilot".
- Ownership fit: strong. Product logic remains in `apps/autopilot-desktop`; skill implementation remains in `skills/blink`; reusable crate boundaries are respected.

## Overall Audit Verdict

Blink integration is materially advanced and close to production-grade for manual and semi-autonomous operations. For fully reliable autonomous swap execution in goal loops, quote-id contract coherence and runtime credential plumbing need to be fixed first. After those, the remaining work is mainly hardening and explicit provider strategy (Blink-only vs live stablesats client).
