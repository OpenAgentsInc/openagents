# 33. Spending approval

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A wallet that an agent can reach needs a rule about when money moves. This
chapter builds it: "send a thousand sats" opens your wallet and waits for your
yes.

## Draft

[Episode 289](../../docs/transcripts/289.md): "Whoa, send a thousand sats."
"Opening your wallet." The agent understood, found the wallet and stopped. It
did not send.

That is the whole design. The front routes the message to the wallet member. A
parser reads the amount. The wallet prepares the payment and shows you what
will happen: how much, to whom, the fee, and the balance left after. Nothing
moves until you approve. The approval is for this payment only.

Money is the effect where a mistake costs the most, so the rules are strict.
The amount is a typed quantity in sats, never a string. A payment has an ID,
and paying the same ID twice does nothing the second time. Every payment
writes a receipt.

An agent that works for you needs to spend without asking each time, or it
can't buy a paid call while you sleep. [Episode
235](../../docs/transcripts/235.md) shows bounded spending by an agent. Here
the bound is yours. You may give an agent an allowance: an amount, a period,
and what it may be spent on. We do not set a limit for you, and any limit you
set is shown wherever the agent's spending appears.

The cap check exists today. An agent's grant carries sats a day and sats a
payment. You approve it after a fresh strong login, and the agent can never
raise its own cap (chapter 8).

An agent's wallet is separate from yours. Its key is its own, and what it can
lose is what you put in it.

## What we build

A plan for what this chapter ships once the draft is approved:

- The payment approval step: amount, destination, fee and remaining balance,
  on every surface.
- Payment IDs that make a repeated send harmless, and a receipt for every
  payment.
- Allowances a person sets for an agent, shown wherever that agent spends.

It builds on what exists today:

- [`pay/wallet.coax`](../../pay/wallet.coax)
- [`pay/wallet-policy.coax`](../../pay/wallet-policy.coax)
- [`coax/lib/quantity.coax`](../../coax/lib/quantity.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`coder/tui/approvals.coax`](../../coder/tui/approvals.coax)
- [`coax/lib/bitcoin/treasury.coax`](../../coax/lib/bitcoin/treasury.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| An agent's allowance | `openagents.account.v1alpha1` `Grant` with `SpendCaps`: sats a day and sats a payment ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf | In use for the cap check at `POST /api/agent/spend` ([chapter 8](ch008-accounts-and-login.md)). The approval step that uses it is this chapter's plan |
| A spend request and its receipt | `openagents.payment.v1` `SpendRequest`, `SpendReceipt` ([spec 26.5](../../docs/protocol/openagents-protocol-spec.md#265-openagentspaymentv1)) | Nostr: private records in a kind 3188 envelope (NIP-HOST) | Spec sketch; no `.proto` file |
| Approval of one payment | `openagents.policy.v1` `ApprovalRequest`, `ApprovalDecision` ([spec 19.2](../../docs/protocol/openagents-protocol-spec.md#192-openagentspolicyv1)) | With the plan's stream | Spec sketch; no `.proto` file |

Planned paths: `pay/spend-approval-policy.coax`, an `#authority` file beside
[`pay/wallet.coax`](../../pay/wallet.coax) and
[`pay/wallet-policy.coax`](../../pay/wallet-policy.coax).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:30, "send a thousand sats"
  - [Episode 63, *Agent Pays L402 Endpoint*](../../docs/transcripts/063.md): an agent pays an endpoint
  - [Episode 97, *User Pays Agent*](../../docs/transcripts/097.md): a user pays an agent
  - [Episode 235, *Agents Earn Bitcoin Tips*](../../docs/transcripts/235.md): bounded agent spending
  - [Episode 212, *Autopilot Learns Bitcoin*](../../docs/transcripts/212.md): an agent learns Bitcoin
- **Old code at `coder-v1.0.0`, crates:** `wallet`, `commercial-spend`, `receipts`, `secret-screen`, `oa-seal`
- **NIPs:** [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md), [NIP-POL](../../nostr/nips/openagents/NIP-POL.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P8

## Open questions for the owner

1. May a person give an agent a standing allowance in 1.0, or does every
   payment ask?
