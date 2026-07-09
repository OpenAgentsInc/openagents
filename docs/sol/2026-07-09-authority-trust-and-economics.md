# Authority, trust, and economics in the Sarah-first system

- Date: 2026-07-09
- Status: Sol analysis; no authority or pricing change
- Governing sources: [`INVARIANTS.md`](../../INVARIANTS.md),
  [`MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md), and owning-surface
  contracts

## The central law

Sarah is the product's interpreter and presenter. She is not its universal
authority.

Her model may understand a request, explain a choice, draft a plan, and ask
for approval. It must not decide by implication that money may move, an email
may send, another account may be used, a public claim may turn green, or a
repository may be mutated. Those decisions belong to typed services and
owner-scoped policy.

This separation is what makes a highly personable front end compatible with a
serious operational system.

## The authority chain

For an externally meaningful action, the safe chain is:

```text
natural-language request
  -> typed candidate intent
  -> authenticated owner/relationship scope
  -> policy + budget + pricing evaluation
  -> explicit approval when required
  -> bounded executor capability
  -> independent verification
  -> exact/private receipt
  -> public-safe projection, if allowed
```

No stage may be replaced by “Sarah said so” or “the worker reported success.”

## Boundaries that must survive product consolidation

### Customer and commercial authority

The `openagents.com` Worker remains authoritative for CRM, credits, checkout,
receipts, and promise state. Sarah is a client even though she lives in the
same monorepo. In-repo proximity must not become a database or internal-import
bypass.

Pricing is code-enforced. Retrieval and conversation can improve explanation
but cannot invent a discount, bundle rule, cap, or transaction amount outside
the signed rule set.

Outbound email remains approval-gated through the shared CRM send rail. A
conversational instruction can create a draft; it cannot silently become a
send receipt.

### Execution authority

Owner-local Pylon capacity and OpenAgents-owned Agent Computers are additive
but distinct:

- owner-local work uses named isolated connected accounts for that owner's
  work;
- org-cloud work uses brokered, scoped credentials inside managed execution;
- neither rail may become pooled subscription resale;
- provider account selection, quota state, and substitution must be typed and
  visible;
- raw credentials and private worker events remain out of public projections.

Sarah may hide engine-room complexity, but she must not hide a change in
authority rail.

### Knowledge authority

Blueprint facts need provenance and access scope. The model can propose a
fact; a safe projection establishes whether it is stored, where it came from,
and who may see it.

An illuminated graph edge is a claim. The Arbiter/Blueprint law that edges
light only from dereferenceable evidence is therefore not visual polish; it is
truthfulness expressed in the interface.

### Public-claim authority

Implementation, an internal receipt, and a public product promise are three
different states. Public copy can broaden only through the promise system and
its evidence/copy gates. Sarah's own language must remain bounded by that same
registry.

## Trust should be visible, not merely logged

Receipts are often treated as backend compliance artifacts. In Sarah-first
they should become a primary UX material.

The canvas can answer:

- What did Sarah understand?
- What did she propose?
- What authority did the action use?
- Which account or capacity class performed it?
- What is happening now?
- What failed, and is retry safe?
- What verification passed?
- What did it cost?
- What changed in the Blueprint because of it?

The user does not need raw traces or provider payloads. They need bounded,
comprehensible projections backed by the private evidence. This is the
difference between observability and data leakage.

## The economic rails

Several economic models coexist and should remain explicitly separate.

### Hosted inference

Khala gateway turns consume provider capacity and need exact usage, cost caps,
quota-aware fallback, and honest metering. The public token counter is a
projection, not proof of an individual task.

### Owner subscription capacity

Connected Codex, Claude, or other subscription accounts serve their owner's
work. More distinct accounts can increase owner throughput, but that capacity
is not inventory OpenAgents may resell. Account health and quota are operating
facts, not fungible marketplace supply.

### OpenAgents org-cloud compute

Agent Computers consume OpenAgents-controlled compute and use brokered
credentials. Model token charging and compute charging can have different
truth sources. Receipts must distinguish them instead of collapsing them into
one vague “cost.”

### Customer payments

Sarah can facilitate a purchase only through the code-enforced checkout and
receipt path. A quote, a checkout link, a settled payment, provisioned credit,
and completed work are separate events.

### Outbound and template economics

Sales outcomes require a full attribution chain. Templates require external
outcome evidence before listing. Operator minutes per engagement are the
anti-agency metric: revenue growth that depends on proportional hidden human
work is not the intended product economics.

## Why exactness matters strategically

Exact accounting is not only financial hygiene. It lets the orchestration
layer make honest decisions:

- choose between free, subscription, and API-metered harnesses;
- detect exhausted accounts without inventing generic session failures;
- compare renderer or model quality against real cost;
- bound standing-employee budgets;
- show the user when a fallback changes economics;
- prove that scale reduces operator effort rather than hiding it.

An `unknown` or `not_measured` value is more valuable than a fabricated
precision because policy can branch safely on honest uncertainty.

## Approval as a product primitive

Approval should not be a modal sprinkled onto risky features. It is a typed
state transition shared by conversation, inbox, mobile push, and cockpit.

A good approval record includes:

- the proposed action and affected scope;
- the authority and budget it would consume;
- material side effects;
- the evidence available before approval;
- expiry and idempotency behavior;
- the approve, reject, or revise outcome;
- the eventual execution receipt.

Sarah's job is to make this understandable at the right moment. The policy
service's job is to enforce it even if Sarah's explanation is wrong.

## Trust architecture as product differentiation

Many agent products can demonstrate a model calling a tool. The harder product
is one that can answer, after thousands of actions:

- who authorized each action;
- which data and account were used;
- whether the action stayed within policy;
- what result was independently verified;
- what cost was exact versus estimated;
- what became public;
- what memory was updated and why.

OpenAgents' receipts, Blueprint provenance, exact token ledger, and typed
authority system are not side constraints on Sarah. They are what can make a
persistent AI relationship credible enough to entrust with real work.
