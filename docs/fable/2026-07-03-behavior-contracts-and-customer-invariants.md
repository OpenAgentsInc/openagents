# Behavior Contracts — Our Product, Customer Products, and New Services

Date: 2026-07-03
Status: design + first-cut invariant catalog. The internal layer landed the
same day (see §9d of [`ROADMAP_QA.md`](./ROADMAP_QA.md)); the customer layer
is the QA Swarm deliverable this document scopes.

## 1. The idea

A behavior contract is a stated product expectation, recorded verbatim, with
a machine oracle that enforces it in an automated sweep and alerts on
deviation. It borrows the product-promise registry discipline
(`docs/promises/registry.md`): dotted versioned ids, exactly one "good"
state, and a mechanical green gate — an `enforced` contract requires at
least one oracle, an automated enforcement tier, and zero blocker refs.

Three concentric uses, same schema:

1. **Our own product** (landed): owner-stated Khala Code UX expectations
   live in `clients/khala-code-desktop/src/contracts/ux-contracts.ts`,
   enforced by DOM oracles in the normal pre-push test sweep. Nothing the
   owner states in a session may remain only in conversation.
2. **Customer products** (this doc): the QA Swarm sells the same machinery —
   we sit down with a client, turn their stated expectations into a contract
   registry for *their* product, run oracles against their staging/prod on a
   cadence, and alert them the moment reality deviates.
3. **New OpenAgents services** (standing rule): every new service or surface
   we ship starts with a contract registry in its first PR — the same
   strictness we sell is the strictness we run under.

The implementation substrate is Bun + Effect (`@openagentsinc/behavior-contracts`:
Effect Schema records, registry validation, oracle-coverage checking behind a
swappable `BehaviorContractOracleSource` service). The contract **data** is
deliberately plain JSON — any runner in any stack can validate a registry and
execute oracles against it. Effect gives *us* swappable execution layers
(file-backed, in-memory, qa-scenario, remote-target); it is not a customer
requirement.

## 2. The invariant catalog — what we offer to enforce

These are the contract categories we propose to every client, each with the
oracle style that enforces it. A client engagement starts by picking from
this menu and writing the client's own statements into their registry.

1. **Indicator truthfulness.** Every spinner, badge, progress bar, and
   status label means exactly what it claims — a loading indicator during a
   fetch is a different fact than a streaming indicator during generation.
   Oracle: DOM/AX assertions on mounted components per state.
   (This category exists because we shipped the violation ourselves:
   `khala_code.chat.sidebar_spinner_streaming_only.v1`.)
2. **Stated-flow availability.** The client's top user journeys (sign-up,
   login, checkout, the core action) complete end to end on their real
   deployment. Oracle: scripted browser/RPC scenario per journey, run on a
   schedule and after each deploy.
3. **Latency budgets.** Named interactions stay under stated budgets,
   measured on the real app, not in fixtures (the
   `docs/qa/khala-code-latency-budgets.md` pattern: budget ids as data,
   sweep evaluates and files offenders).
4. **Error-state honesty.** Induced failures surface a visible, accurate
   error; no silent failure, no infinite spinner, no success copy on a
   failed action. Oracle: fault-injection scenarios with DOM assertions.
5. **Dead-control detection.** Every visible interactive control does
   something observable. Oracle: crawler + monkey explorer with an
   "interaction had no effect" detector.
6. **Cross-surface data consistency.** The same fact renders identically
   everywhere it appears (list count vs detail count, badge vs page).
   Oracle: consistency comparisons across two reads, the qa-harness
   `consistency` oracle shape.
7. **Copy/claim safety.** User-facing copy never claims beyond verified
   state — the promise-registry discipline applied to a client's marketing
   and product copy, with safe/unsafe copy recorded per claim.
8. **Stated-expectation pinning (the meta-invariant).** Anything the
   client's founder/PM states as "how it should work" enters the registry
   with an oracle or an explicit `pending` + blocker. The deliverable is
   that they stop discovering their own product's regressions by clicking
   around — which is exactly the failure our owner reported on 2026-07-02.
9. **Accessibility floor on critical flows.** Labels, focus order, and
   keyboard operability on the flows named in (2). Oracle: AX-tree
   assertions (qa-runner Mode V lineage).
10. **Money-path integrity** (where applicable). Checkout/billing flows
    verified in test mode each sweep; live-mode smoke only with the
    client's explicit arming, mirroring our own live-tier invariants.

## 3. The deviation loop

- **Cadence:** oracles run in the client's chosen tiers — on-deploy,
  nightly, weekly live. Internally contracts ride the same loop as QA-1
  (nightly matrix) once it exists; until then they run in the pre-push
  test sweep.
- **Receipts:** every sweep produces a per-contract pass/fail receipt
  (promise-transition-receipt shape: checks, evidence refs, checkedAt) so
  "it was green on date X" is a lookup, not a memory. Receipts never flip
  registry state by themselves; state changes are maintainer actions.
- **Alerts:** a deviation notifies the client (webhook/email/forum thread)
  with the contract id, the statement in their own words, and the failing
  evidence — a screenshot, trace, or diff, not a stack trace.
- **Evidence surface:** the QA Swarm shareable evidence URL (QS lanes,
  epic #8071) renders the registry + latest receipts as the client-facing
  scoreboard, the same way `/api/public/product-promises` renders ours.

## 4. Engagement shape (customer onboarding)

1. Intake session: capture stated expectations verbatim (recorded, then
   written as `statement` fields — the client signs off on the wording).
2. We author the registry + oracle pack against their staging URL; every
   contract starts `pending`.
3. Contracts flip to `enforced` one by one as oracles land and pass —
   mechanical gate, no vibes.
4. Cadence + alert channel configured; receipts and the evidence URL go
   live.
5. Ongoing: new expectations enter through the same intake rule; deviations
   file as strict bugs with the contract id in the title.

Pricing/packaging, the swarm board, and the sales motion stay owned by
[`2026-07-02-qa-swarm-product-plan.md`](./2026-07-02-qa-swarm-product-plan.md);
this catalog is the substance those packages sell.

## 5. Internal standing rule for new services

Every new OpenAgents service, app, or client surface ships its first PR with:

- a `src/contracts/` registry (schema from
  `@openagentsinc/behavior-contracts`),
- at least the indicator-truthfulness and error-state-honesty contracts
  stated for its primary surface,
- oracle tests in the package's normal test glob so the package `verify`
  and the repo `test:*` chain enforce them from day one,
- a human rendering under `docs/<service>/` kept in sync by a doc-sync test
  (the `docs/khala-code/khala-code-ux-contract.md` pattern).

The rule is recorded in `AGENTS.md` (Working Rules, 2026-07-03 owner
mandate). Deviating from an enforced contract to make a change pass is a
contract change and needs the owner's sign-off.

## 6. Boundaries

- Contracts bind behavior claims; they grant no authority — no deploy,
  spend, moderation, or data-access rights follow from a green registry.
- Client registries and receipts are client-private by default; the public
  scoreboard pattern applies only when the client opts in.
- Fixture tiers never touch client production credentials; live-mode oracles
  run only under the client's explicit arming, mirroring our own live-tier
  invariants in `ROADMAP_QA.md` §11.
