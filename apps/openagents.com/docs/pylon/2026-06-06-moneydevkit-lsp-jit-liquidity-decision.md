# MoneyDevKit LSP/JIT Liquidity Decision

Issue #353 / `OPENAGENTS-L-005` evaluates whether OpenAgents should port, wrap, or
defer MoneyDevKit-style LSP and just-in-time receive-liquidity behavior for
Pylon.

## Decision

Superseded decision on 2026-06-07: **wrap MoneyDevKit's agent-wallet as
Pylon's default wallet runtime**, while preserving OpenAgents authority
boundaries.

MoneyDevKit should now be treated as:

- the default local self-custodial wallet runtime for ordinary Pylon installs;
- a strong reference for LSPS4/JIT receive-liquidity behavior;
- a checkout and L402 reference for customer/product payments;
- an agent-facing JSON CLI integration surface; and
- a VSS/remote-state reference for later optional Pylon backup work.

The default wrapped runtime is `@moneydevkit/agent-wallet`. It gives Pylon a
JSON-first local wallet daemon, BOLT11 receive invoices, BOLT12 offers, send,
balance, status, and payment-history commands without forcing Pylon to operate
raw Lightning liquidity on day one.

The wrapped runtime must not become:

- the source of truth for accepted-work eligibility;
- the source of truth for whether OpenAgents owes a payout;
- the Treasury payout dispatcher;
- the Nexus reconciliation authority; or
- a hidden custody sidecar outside Pylon's explicit wallet/runtime status.

Pylon owns the product contract around wallet runtime selection, local wallet
state directory selection, redacted telemetry, receipts, and Nexus payout
target registration metadata. Nexus/Treasury still own accepted-work
eligibility, payout dispatch, settlement reconciliation, and production payout
receipts. Native `ldk_node` remains an explicit lower-level wallet path for
direct LDK regression and liquidity work, not the default user path.

The previous "port only, do not wrap" conclusion is retired. The long-term
work is now to keep the MDK wrapper honest and to port the most useful
MoneyDevKit LSPS/JIT/VSS patterns into OpenAgents-owned runtimes where direct
control is required.

## Reviewed Local Sources

- `/Users/christopherdavid/work/2026-05-26-moneydevkit-liquidity-offload-analysis.md`
- `/Users/christopherdavid/work/2026-05-22-pylon-built-in-ldk-wallet-audit.md`
- `/Users/christopherdavid/work/projects/moneydevkit/README.md`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/lightning-js/src/lib.rs`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/lightning-js/src/splice_manager.rs`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/lightning-js/src/max_sendable.rs`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/mdkd/README.md`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/agent-skills/skills/mdk-agent-wallet/SKILL.md`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/agent-skills/skills/mdk-l402-api/SKILL.md`
- `/Users/christopherdavid/work/openagents/docs/2026-05-15-ldk-nexus-treasury-transition-audit.md`

## What MoneyDevKit Solves Well

### LSPS4/JIT Receive Reliability

`lightning-js/src/lib.rs` configures `ldk-node` with an LSPS4 liquidity source
using an LSP node id and address. That is directly relevant to Pylon because
the weak point in contributor payouts is reliable receive capacity, not the
existence of a wallet.

The target product behavior is:

- a Pylon has a local wallet identity;
- the wallet registers a durable receive target with Nexus;
- the operator can see receive readiness clearly; and
- the user does not have to manually reason about channel liquidity before
  receiving accepted-work bitcoin payouts.

### Liquidity Consolidation And Sendability

`lightning-js/src/splice_manager.rs` contains a background splice manager that
moves confirmed on-chain funds into the largest usable LSP channel, with
in-flight tracking and a BDK resync grace period.

`lightning-js/src/max_sendable.rs` has a useful pattern for projecting max
sendable value without overstating outbound capacity. It distinguishes no
usable LSP channel from dust-level availability and reserves routing-fee
headroom.

Both are useful Pylon patterns, but they are wallet-mutation logic. They belong
in the native Pylon/LDK runtime after testing, not in the OpenAgents product surface Worker and not
in the current read-only projection issues.

### Hosted Checkout And L402

`mdkd` and `mdk-checkout` are strong references for merchant and customer
checkout flows. The daemon has REST, webhook, invoice metadata, read-only and
full-access Basic Auth tiers, and platform-token integration.

That is useful for:

- Autopilot Sites commerce;
- paid agent API access;
- L402-style route recovery;
- customer checkout experiments; and
- product billing prototypes.

It is not the same problem as accepted-work payout custody.

### Agent Wallet Runtime

`mdk-agent-wallet` is a good agent-facing tool lane. It gives agents JSON CLI
commands for balance, receive, send, history, and L402 payment retries.

It is now also the default Pylon wallet runtime wrapper. The distinction is
authority, not whether the CLI is wrapped: MDK can manage local wallet mechanics
behind Pylon, but Pylon and Nexus still produce the provider contract,
registration metadata, accepted-work eligibility, and settlement evidence.

## Port, Wrap, Or Defer

| Option | Assessment | Decision |
| --- | --- | --- |
| Wrap `@moneydevkit/agent-wallet` as Pylon wallet | Fastest correct default for ordinary contributors. Provides JSON CLI commands, local daemon lifecycle, BOLT11/BOLT12 receive artifacts, send, balance, and history while Pylon still owns runtime selection, redacted telemetry, receipts, and Nexus registration metadata. | Preferred default |
| Port LSPS/JIT patterns into owned Pylon wallet code | Still useful where OpenAgents needs direct lower-level control, native regression coverage, or a future owned LSP/VSS path. Requires fork diffing and signet tests before live use. | Later hardening path |
| Wrap `mdkd` beside Pylon | Faster to experiment with checkout/service flows, but creates extra daemon lifecycle, HTTP auth, platform token, mnemonic, and wallet-state surfaces. It risks confusing checkout state with accepted-work payout state. | Defer for Pylon, keep as checkout reference |
| Defer all MDK liquidity work | Safe but leaves users exposed to manual Lightning liquidity friction. | Only acceptable until the read-only readiness/projection layer and signet harness are complete |

## Required Boundaries

Pylon must keep:

- wallet-owned payout target identity;
- local recovery and encrypted backup semantics;
- local receipt history;
- Nexus payout target registration;
- provider eligibility boundaries; and
- redacted wallet telemetry.

Nexus/Treasury must keep:

- work acceptance authority;
- payout eligibility authority;
- payout dispatch authority;
- settlement reconciliation;
- public-safe receipts; and
- operator controls for retry, pause, or block states.

MoneyDevKit can inform liquidity and checkout behavior, but it cannot decide
whether OpenAgents owes a payout or whether a payout is settled.

## Secret And State Boundaries

The following must never enter public projections, Site receipts, Forum posts,
customer-visible docs, or agent-readable manifests:

- wallet mnemonics;
- MDK platform access tokens;
- LSP credentials;
- webhook secrets;
- payment preimages;
- raw invoices;
- raw channel monitor state;
- raw VSS payloads;
- raw telemetry payloads; or
- raw payout targets.

The MDK wrapper must emit only redacted refs into OpenAgents product surface and preserve the
read-only projection boundary already used by:

- `workers/api/src/pylon-ldk-readiness-projections.ts`;
- `workers/api/src/pylon-payout-target-admission.ts`;
- `workers/api/src/pylon-wallet-liquidity-readiness.ts`; and
- `workers/api/src/pylon-wallet-telemetry.ts`.

## Test And Deployment Posture

Do not add mainnet wallet mutation or live LSP API calls from OpenAgents product surface.

The next implementation stage should be:

1. Keep the wrapped `@moneydevkit/agent-wallet` path as the default Pylon
   runtime and verify its JSON command surface: `init`, `status`, `balance`,
   `receive`, `receive-bolt12`, `send`, and `payments`.
2. Keep MDK wallet state under a Pylon-scoped local home so it does not collide
   with a user's unrelated `~/.mdk-wallet`.
3. Preserve Pylon-owned receipts, redacted telemetry, and Nexus payout-target
   registration metadata around the wrapped runtime.
4. Fork-diff MoneyDevKit `ldk-node`, `rust-lightning`, and `lightning-js`
   against the owned/upstream LDK path for later native hardening.
5. Build a signet/regtest harness for LSPS4/JIT receive behavior using a
   dedicated test wallet.
6. Add Pylon operator telemetry for LSP configuration, channel readiness,
   receive capacity, route failures, and blocked receive state.
7. Project only redacted readiness state into OpenAgents product surface.

## Roadmap Implication

This issue's original decision gate is superseded by the 2026-06-07 MDK wrapper
decision. The current code direction is implemented in the `openagents` repo:
Pylon wraps MoneyDevKit agent-wallet as the default runtime and keeps native
`ldk_node` available as an explicit low-level path.

The next roadmap work should continue with:

- #354: VSS as optional Pylon remote state backend;
- #355: accepted-work payout SLO projection;
- #356: safe public payout rows;
- #357: read-only Lightning/Pylon graph API contract; and
- #358: accepted-work proof links in Sites/order receipts.

After that read-only surface is complete, a later Pylon/Rust issue should add
the LSPS4/JIT signet harness and fork-diff checklist for the native hardening
path while the MDK wrapper remains the ordinary default.
