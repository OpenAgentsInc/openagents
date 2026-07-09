# Sarah-first as product architecture

- Date: 2026-07-09
- Status: Sol analysis; interpretive, non-authoritative
- Primary sources:
  [`Sarah-first thesis`](../fable/2026-07-09-sarah-first-product-thesis.md),
  [`Sarah–Khala assessment`](../fable/2026-07-09-sarah-khala-connection-assessment.md),
  and [`Blueprint Map audit`](../sarah/2026-07-09-blueprint-map-surface-audit.md)

## The important claim

“Sarah-first” should not mean “put a face in front of every feature.” It means
that OpenAgents chooses a **persistent relationship as the organizing unit of
the product**.

Most software is organized around objects and destinations: projects,
dashboards, tickets, repositories, settings, and apps. Sarah-first organizes
around an ongoing relationship that can traverse those objects. The user says
what they want, Sarah maintains context, the system exposes the relevant
working state, and the relationship continues after the task finishes.

That is a deeper move than choosing a home screen.

## Sarah is a chassis, not merely a persona

The name and face matter because they create continuity, but the reusable
product is the chassis beneath them:

- a disclosed AI identity;
- durable, prospect- or owner-scoped memory;
- a typed tool inventory;
- code-enforced deal and authority rules;
- authenticated capability unlocks;
- a live canvas for state, work, and evidence;
- approval requests at the moment authority is needed;
- exact receipts after work completes;
- evaluation suites that constrain behavior.

The sales role is the first role carried by this chassis. Coding supervision
is the first major capability added to it. Standing employees and the company
brain generalize the same chassis rather than beginning beside it.

## Conversation and canvas are one interface

Conversation alone is too lossy for serious delegated work. It is good for
intent, negotiation, explanation, and correction, but poor at showing large
state, provenance, parallel activity, or exact diffs. A dashboard alone is
too rigid and makes the user translate their goal into the software's object
model.

Sarah's split surface resolves the tension:

- **Conversation is the control plane for human intent.** It is where the
  user asks, clarifies, steers, and approves.
- **The Blueprint canvas is the inspectability plane.** It is where memory,
  unknowns, plans, code, activity, costs, blockers, and receipts become
  spatially legible.

The left/right split is therefore not a decorative layout. It expresses the
product architecture: relationship beside state. As capability grows, the
canvas can add panes without forcing the conversation to carry every detail.

## The canonical product loop

Every serious Sarah capability should fit this loop:

1. **Understand.** Sarah receives natural language and retrieves only the
   context allowed for this relationship.
2. **Propose.** She produces a typed candidate action, plan, or question. A
   model utterance is not yet authority.
3. **Resolve authority.** The system identifies the owner scope, tool policy,
   budget, pricing rule, and approval requirement.
4. **Dispatch.** A typed workflow selects an execution rail and named
   capacity. The user does not need to know which harness or host is chosen,
   but substitutions and failures remain explicit.
5. **Stream.** Progress appears in conversation and on the canvas using
   durable, resumable state.
6. **Verify.** The executor's claim is checked against the workflow's
   verification contract.
7. **Receipt.** Exact usage, artifacts, lifecycle, and approval evidence are
   recorded under their owning authorities.
8. **Learn.** The Blueprint is updated with provenance-bearing results. The
   next turn starts from evidence, not synthetic recollection.

This loop should be visible in product behavior even when implementation is
distributed across many services.

## Relationship modes must remain explicit

One Sarah surface serves materially different relationships. Conflating them
would be dangerous.

| Mode | Typical user | Allowed posture |
| --- | --- | --- |
| Prospect | Unauthenticated or lightly identified visitor | Explain, qualify, draft intake, offer code-enforced products, request handoff |
| Customer | Authenticated buyer | Use owned data, create bounded work, pay through receipted paths, inspect results |
| Operator | Authenticated owner or team member | Dispatch to owned fleets, approve sensitive actions, inspect capacity and private receipts |
| Administrator | Explicit elevated scope | Configure policies and infrastructure through separate typed authority, never persona implication |

The account link is not just convenience. It is a capability boundary. Tone,
retrieval scope, tools, pricing behavior, and available actions should change
from authenticated policy, not because the model infers that the person
“sounds like” an owner.

## The first decisive vertical slice

The Blueprint Map is now implemented. The highest-value next proof of the
Sarah-first thesis is narrower than “put all of Khala Code inside Sarah”:

> An authenticated owner asks Sarah to run one bounded public issue on their
> linked fleet, sees the typed plan and target, watches resumable progress,
> receives the verified closeout in the canvas, and can ask a follow-up from
> the same conversation.

The slice is successful only if it proves all of the following:

- Sarah invokes the existing typed Khala → Pylon → worker rail rather than a
  prompt-only imitation.
- Account and Pylon selection stay owner-scoped.
- Progress survives reconnect and is understandable without terminal access.
- Approval is requested only where the policy requires it.
- The result includes verification and exact usage evidence.
- The conversation and Blueprint update from safe projections, not raw worker
  events or private prompts.
- Failure is typed and actionable; a text-only path remains usable if video
  degrades.

That is the smallest loop that turns Sarah-first from a framing decision into
a product fact.

## “One front door” does not mean “one tool for everyone”

CLI and desktop power surfaces should remain. Experts need batch operations,
deep logs, repository context, and infrastructure controls that would be
awkward in a general conversation. The Sarah-first rule is about default
orientation and shared state:

- Sarah is where a person begins and returns.
- Power tools are alternate projections over the same work, not separate
  realities.
- A run started from desktop should be legible to Sarah.
- A run started through Sarah should be inspectable in the cockpit.
- Neither surface invents a second approval, accounting, or memory model.

This distinction prevents “one front door” from becoming an excuse to make a
chat UI absorb every specialist interaction.

## Product quality shifts under Sarah-first

Once Sarah is the core product, several things previously treated as feature
quality become availability:

- Avatar freeze, silent audio, and long turn latency are front-door outages.
- Persona bleed from a shared model lane is a product-integrity failure.
- Prospect-memory leakage is a security incident.
- An ungrounded Blueprint edge is a false statement by the product.
- An invisible fleet failure breaks the relationship even if a worker later
  succeeds.
- A missing receipt makes “done” untrustworthy.

The owned avatar program, persona-neutral Khala lane, cross-prospect isolation,
semantic retrieval, and deploy simulator belong on the core-product path for
this reason.

## Metrics that reflect the thesis

Page views and avatar-session starts are not enough. Better indicators are:

- time from first utterance to a correctly scoped useful action;
- percentage of sessions that recover gracefully from video or provider
  degradation;
- authenticated conversations that reach a verified work receipt;
- work runs understood and steered without terminal fallback;
- cross-device continuation success;
- percentage of Blueprint facts with dereferenceable provenance;
- approval requests accepted, revised, or rejected without confusion;
- repeat conversations that reuse valid context without leaking invalid
  context;
- operator minutes per completed outcome.

## The strategic bet

Sarah-first bets that people prefer to build trust with one capable,
inspectable relationship rather than learn a suite of agent products. The
Blueprint canvas prevents that relationship from becoming a black box; the
typed execution system prevents it from becoming a role-play; the receipt
system prevents “trust” from meaning mere confidence.

The right slogan is not “Sarah can do everything.” It is:

**Sarah can help you direct increasingly powerful work, while the system
shows what she knows, what she is doing, what authority she has, and what
actually happened.**
