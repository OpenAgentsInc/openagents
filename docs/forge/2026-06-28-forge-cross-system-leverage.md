# Forge Cross-System Leverage Synthesis

Date: 2026-06-28

Issue: #6760

## Source Note

This synthesis is grounded in the Forge material present in this checkout:
`docs/blitz/forge/*`,
`docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`,
the live `/forge` and Autopilot code references named by those docs, and the
system docs cited below. The issue also asks for `docs/forge/origin.md`,
`docs/forge/2026-06-28-forge-standup-spec.md`, an owned-coordination-layer
audit, and merged FORGE-1..6 component docs/packages. Those paths are not
present in this bounded checkout, so this document uses the following shorthand
for the stand-up target the issue describes:

| Ref | Stand-up capability |
| --- | --- |
| SU-1 | Forge coordination store: durable workspaces, work orders, state, refs, and source freshness. |
| SU-2 | Git receive and tenant Git auth: repository ingress, tenant identity, branch/ref authority, and safe write boundaries. |
| SU-3 | Packfile archive: content-addressed Git object retention, replay, and proof dereferencing. |
| SU-4 | Dispatch protocol: route work to owned Pylons, hosted lanes, labor, or inference lanes by typed capability and budget. |
| SU-5 | Verification runner: execute named checks, replay proofs, record pass/fail, and refuse "clean means correct" overclaims. |
| SU-6 | Promotion gate: merge/release/deploy/customer-delivery only from evidence-backed terminal states. |
| SU-7 | Operator surface: `/forge`, Autopilot workrooms, dashboards, receipts, and cohort/customer lanes. |

The older Forge docs already define the same shape in product terms: `/forge`
is an auditable factory dashboard whose production line maps work through
Signal, Triage, Code Gen, Validate, Release, Document, Monitor, and Deploy;
Automations create real Autopilot work orders; metrics must be live or tagged
seeded; vertical workspaces are seeded as draft, public-safe inputs; and
Customer #1/cohort evidence must be refs-only with privacy review before public
projection (`docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md`,
`docs/blitz/forge/2026-06-16-forge-automations-surface.md`,
`docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md`,
`docs/blitz/forge/2026-06-17-customer-one-cohort-source-contract.md`).

## Pylon

Pylon is Forge's worker-capacity and local execution substrate. It already owns
contributor identity, the TUI/operator shell, local Codex and Claude execution,
wallet and payout readiness, presence heartbeat, assignment leases, progress,
closeout refs, and the workspace materializer
(`apps/pylon/README.md`, `apps/pylon/docs/codex-agent-task-smoke.md`,
`apps/pylon/docs/workspace-materializer.md`). The Codex task smoke proves the
shape Forge needs: a `codex_agent_task` lease is admitted only under
`capability.pylon.local_codex`, executed in a bounded workspace, verified by a
real command, scanned for redaction, and closed out with public-safe result and
settlement fields (`apps/pylon/docs/codex-agent-task-smoke.md`).

Forge integration point: SU-4 routes Code Gen and Validate jobs to Pylon by
capability and advertised capacity; SU-1 stores the assignment and closeout
refs; SU-5 records the named verification command and result; SU-7 renders the
progress/closeout lane in `/forge` and Autopilot workrooms. Pylon's dev doctor
and context projection also feed Forge's Context lane: repo identity,
instruction refs, dirty-state counts, adapter readiness, and current-job refs
without raw local paths or credential material (`apps/pylon/README.md`,
`docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`).

The future leverage is to make Pylon the default Forge runner, not just a proof
runner. The missing pieces are operational rather than conceptual: multi-ref
assignment runner ergonomics, live progress while Codex is running,
assignment-to-workspace lookup in closeout, and a first-class Forge
orchestrator that leases up to advertised capacity. These map to SU-4 and SU-7;
they should not widen authority. Public command paths still reject danger flags,
and owner-local Khala/Pylon/Codex delegation remains distinct from third-party
labor or paid marketplace execution (`AGENTS.md`, `apps/pylon/README.md`).

## Tassadar

Tassadar strengthens Forge's verification runner and promotion gate by making
one class of work exactly replayable. The Tassadar docs describe digest-pinned
compiled executor workloads, exact trace replay, typed refusals, and independent
validator replay: a verifier re-executes the claimed workload and compares
digests instead of judging prose (`docs/tassadar/README.md`,
`docs/tassadar/work-that-proves-itself.md`,
`packages/tassadar-executor/README.md`).

Forge integration point: SU-5 should treat Tassadar as the highest-confidence
verification class for deterministic/kernel work. A Forge work order can carry
a verification class such as exact replay, store the claimed digest and replay
window in SU-1/SU-3, and require the independent replay verdict before SU-6
promotes. This directly addresses the key warning in the stand-up request:
clean is not correct. Passing tests are one evidence ref; exact replay is a
stronger evidence ref when the workload class supports it.

The future leverage is economic as much as technical. Cheap replay lets Forge
price and route low-margin validation work to weak devices while reserving
human or model review for fuzzy tasks. That makes the verification ladder
explicit: unit tests, deterministic replay, model review, second-agent review,
human review, and owner acceptance are separate rungs, not interchangeable
labels. Promotion should name which rung was used.

## Artanis

Artanis is the production cloud mind and the operator brain Forge can reuse for
standing work selection, diagnosis, and fleet supervision. It already runs as a
bounded scheduled loop inside the OpenAgents Worker, persists tick/runtime
records, coordinates through Forum/Nostr, and respects explicit spend and
operator approval gates (`docs/artanis/README.md`). The 2026-06-28
`autonomous-ops-v1` spec is the sharper Forge lesson: every consequential
autonomous step needs a typed Blueprint Signature, ordered evidence predicates,
and a terminal state before the action unlocks
(`docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`).

Forge integration point: Artanis can choose the next Forge work item, diagnose
stalled capacity, and report fleet health, but SU-6 must only accept its action
when the relevant signature gate is terminal. The spec's five signatures map
directly: fleet liveness gates dispatch health, diagnosis grounding gates
remediation, issue-close-safe gates issue closure, command-execution-source
verified gates proposed verifier commands, and merge-deploy-gate gates release
claims. Those are not generic advice; they are reusable state machines for
Forge's promotion layer.

The future leverage is to make Forge's operator loop impossible to overstate.
Artanis should be able to say "blocked: missing quota ledger read" or "blocked:
no source-read evidence for command" before it can say "healthy" or "safe to
merge." That makes the M-plan concrete: typed evidence first, action second,
public claim last.

## Payments

Payments turn Forge from an internal work tracker into an accepted-outcome
economy. The MDK/Ark audit defines the broader agent-wallet path: MDK handles
public L402 paid APIs and checkout flows while Ark/Bark can support
low-liquidity, high-frequency inter-agent transfers in the future
(`docs/2026-06-09-ark-mdk-agent-payments-audit.md`). The live Khala MPP docs
show the concrete production pattern: unauthenticated inference requests get a
402 challenge, payment is verified, the completion is served, and receipts name
the rail and status; Lightning, USDC, and card rails are handled behind honesty
gates (`docs/mpp/README.md`,
`apps/openagents.com/docs/launch/2026-06-23-khala-billing-mpp-production-proof.md`).

Forge integration point: SU-1 stores bounty, budget, receipt, and settlement
refs; SU-4 routes according to own-capacity/no-spend, buyer-debit, or paid
labor lane; SU-6 refuses merge, payout, or settlement claims without the
receipt-backed terminal state. The existing Forge spend-routing doc already
keeps this distinction: `buyerDebitRequired` is placement metadata, not proof
that money moved (`docs/blitz/forge/2026-06-16-customer-one-spend-routing.md`).

The future leverage is a closed loop: bounty -> claimed assignment -> verified
artifact -> accepted outcome -> settlement. Own-capacity metering and paid
labor settlement must remain separate. A Forge work order can be no-spend and
still metered for token velocity; it can be paid and still not payable until
verification, acceptance, and payout target checks all pass.

## Blueprint

Blueprint is Forge's governance and evidence vocabulary. The OpenAgents
Blueprint boundary defines authority modes of `evidence_only`,
`approval_gated`, and `export_only`; Program Runs are evidence, not write
authority (`apps/openagents.com/docs/blueprint/2026-06-05-openagents-blueprint-package-boundary.md`).
The shared `@openagentsinc/blueprint-contracts` package is intentionally narrow
but security-critical: it centralizes the recursive private-data-safety
predicate so exported Blueprint projections do not drift back to weak regex
checks (`packages/blueprint-contracts/README.md`). The seeded Autopilot
signature catalog shows the intended action vocabulary: continue, test, fix,
summarize, request context, retry account, stop, prepare review, route
selection, research policy, email decisioning, and proof projection
(`apps/openagents.com/docs/blueprint/2026-06-05-autopilot-continuation-signature-catalog-v1.md`).

Forge integration point: every SU-6 promotion should be a Blueprint decision
with evidence refs and receipt refs, not an ad hoc state flip. SU-4 route
selection should resolve typed capabilities and signatures, not prose keywords.
SU-7 should render the signature state, evidence refs, blockers, and non-
authority caveats next to each work item.

The future leverage is consistency. Forge can use Blueprint to make all major
state changes look the same: dispatch, verification, review, merge, deploy,
delivery, settlement, and public-promise promotion each has inputs, predicates,
terminal states, receipts, and rollback blockers. That is the way to avoid a
parallel "Forge-specific" authority model that later conflicts with the
platform.

## Autopilot And OpenAgents Web

Autopilot and `openagents.com` are Forge's current user-facing surface. The web
app already owns the `/forge` dashboard, `/autopilot` workrooms, Runs
projection, provider-account pool projection, work-order API, and the product
surface rules for live-vs-seeded metrics (`apps/openagents.com/README.md`,
`docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md`,
`docs/blitz/forge/2026-06-16-forge-automations-surface.md`). The Autopilot
Coder roadmap records that Forge already has many terminal-agent primitives:
change capture, delivery readiness, diff-review artifacts, progress lane,
session navigation, context snapshot, repository memory, and retrieval plan
projections (`docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`).

Forge integration point: SU-7 is not a marketing page. It is the operator
cockpit for SU-1..SU-6: work order creation, routing, progress, evidence,
review, blockers, release, and cohort/customer lanes. `/forge` should continue
to tag each value as live, configured, seeded, absent, blocked, or stale. The
vertical templates already show the customer workspace pattern: seed public-
safe draft memory, require account/customer authority before live writes, and
keep review gates visible (`docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md`,
`docs/blitz/forge/2026-06-16-legal-prefilled-workspace.md`,
`docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md`).

The future leverage is unification. Autopilot workrooms should be the per-run
drilldown, and `/forge` should be the factory-level view over the same
authoritative refs. A work item should not have one truth in a chat/workroom and
another in a factory dashboard.

## Khala

Khala gives Forge low-friction inference and owner-capacity coding delegation.
The Khala docs define the product as an OpenAI-compatible endpoint
(`openagents/khala`, base `https://openagents.com/api/v1`) over a network of
agents (`docs/khala/README.md`). The bare-agent Pylon MCP smoke proves the
Forge-relevant path: a vanilla coding agent calls typed `khala.request` with
`workflow: "codex_agent_task"`, a caller-owned linked Pylon executes the
assignment no-spend, durable resume returns the closeout frame, and the proof
projection exposes exact token usage plus owner-only trace/raw-event refs
without raw Codex payloads (`docs/khala/2026-06-25-bare-agent-pylon-mcp-khala-e2e-smoke.md`).
The promise reconciliation narrows the current claim: explicit typed
owner-capacity delegation is green only when accepted closeout and exact
`token_usage_events` rows exist; broad automatic routing, third-party capacity,
paid work, payout eligibility, and public raw event visibility remain out of
scope (`docs/promises/2026-06-27-khala-cli-own-capacity-reconciliation.md`).

Forge integration point: SU-4 can route no-spend owner-capacity work through
Khala/Pylon when the caller owns the target capacity and the workflow is typed.
SU-1 and SU-5 should store demand attribution, exact token rows, assignment
refs, and closeout refs. SU-7 can show token velocity and capacity burn, but it
must not treat public counter movement alone as assignment proof.

The future leverage is cost and throughput. Forge agents can use Khala as the
$0/own-capacity lane for planning, code work, review, and verification
assistance while still preserving exact demand attribution. That lets the
factory optimize around useful accepted output per token without hiding which
owner, Pylon, assignment, and workflow generated the usage.

## Psionic

Psionic is the ML substrate Forge should attach to, not reimplement. The Pylon
Psionic connector docs define a deliberately attach-only boundary: Pylon does
not bundle Psionic binaries or model weights, does not download them on
startup, and projects connector state as `absent`, `configured`, `negotiated`,
or `refused` with typed blockers and no raw endpoint/path leakage
(`apps/pylon/docs/psionic-connector.md`). The connection audit keeps ownership
split: Psionic owns model serving, training jobs, evals, artifact identity, and
ML worker receipts; Pylon owns connector state, assignment/presence/wallet
posture, sandbox policy, and public-safe closeout refs
(`apps/pylon/docs/2026-06-09-pylon-psionic-ml-connection-audit.md`). The
coordinator roadmap adds the future learning loop: hidden-state extraction,
coordinator heads, separable CMA-ES, scalar terminal rewards, and worker-pool
binding over verified-work receipts (`docs/sakana/psionic-coordinator-roadmap.md`).

Forge integration point: SU-4 dispatch can select Psionic-backed inference,
training, eval, or coordinator jobs only after the connector reports an
eligible capability. SU-5 imports Psionic eval/verification receipts rather
than inventing a Forge ML verifier. SU-6 promotes model/coordinator artifacts
only through candidate/shadow/governance states.

The future leverage is learned routing. Forge will generate enough work-order,
verification, token, and settlement data to train a coordinator on
verified-work-per-sat. Psionic should own that training substrate; Forge should
own the work graph and terminal reward receipts.

## Nostr, NIP-90, And World Projection

Nostr is Forge's fallback and external market bus, not its authority layer. The
OpenAgents relay accepts NIP-90 job requests/results/feedback and NIP-DS
listing/offer events, plus a write-gated set of general coordination kinds for
authorized pubkeys; it grants no payment, identity, moderation, assignment,
payout, or settlement authority (`apps/nostr-relay/README.md`). The
`@openagentsinc/nip90` package keeps protocol helpers Effect Schema-backed and
public-safe, including labor request/result helpers and a NIP-LBR closeout
receipt that composes request, quote, acceptance, and result events into one
content-addressed receipt without moving sats (`packages/nip90/README.md`).
The world Worker is another projection example: it ingests public-safe rows and
explicitly does not become source of truth for run, proof, settlement, or
product-promise state (`apps/openagents-world/README.md`).

Forge integration point: SU-4 can publish or consume labor/dataset/job refs
over NIP-90 when external market discovery is useful; SU-1 should dereference
those refs into Forge rows only after schema validation; SU-6 must still require
the platform's receipt-backed acceptance/payment authority before promotion or
settlement. World projection can visualize Forge activity, but only from
already public refs.

The future leverage is resilience and distribution. If the primary web/API path
is impaired, Nostr can keep market coordination and discovery visible. If Forge
needs external agents, NIP-90/LBR gives it a protocol-native request/result
shape. Neither path should bypass the Forge coordination store or Blueprint
promotion gates.

## Biggest Leverage First

1. **Make the SU-6 promotion gate Blueprint-signature governed.** Reuse the
   `autonomous-ops-v1` signatures immediately for command verification,
   issue-close safety, and merge/deploy claims. This prevents the most expensive
   class of Forge errors: actions without evidence.
2. **Route Code Gen and Validate through Pylon/Khala owner capacity by default
   where ownership is proven.** This gives Forge cheap throughput now, while
   exact token rows and closeout refs keep attribution honest.
3. **Upgrade SU-5 into a verification ladder, not a boolean.** Named checks,
   deterministic tests, Tassadar exact replay, model review, human review, and
   owner acceptance should be distinct evidence classes with separate refs.
4. **Unify `/forge` and Autopilot workrooms over one SU-1 row model.** Factory
   dashboards, per-run workrooms, vertical workspaces, and cohort lanes should
   all render the same work-order, routing, progress, evidence, review, and
   blocker refs.
5. **Keep money and settlement receipt-first.** Budget/routing metadata is not
   payment proof. Forge should only claim bounty, payout, or settlement states
   from durable receipt rows and explicit terminal gates.
6. **Attach Psionic after the connector and receipt boundary is clean.** Use
   Psionic for ML jobs and learned coordination, but keep Pylon/Forge from
   claiming model serving or training authority directly.
7. **Use Nostr/NIP-90 as the external/fallback bus.** It is excellent for
   discovery, public market events, and outage coordination; it is not a
   shortcut around Forge's store, verification, or settlement authority.

The common rule across all systems is simple: Forge should accelerate work by
sharing capacity and evidence, not by sharing authority. Every system above is
most useful when it contributes one typed capability, one proof class, or one
operator surface to the same evidence-backed Forge state machine.
