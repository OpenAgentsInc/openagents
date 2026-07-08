# RX-9 Reactor Need-To-Know Access Receipt

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04
Issue: [#8277](https://github.com/OpenAgentsInc/openagents/issues/8277)
Status: source fixture landed; no external customer deployment or public
availability claim.

RX-9 adds a typed, deny-by-default need-to-know access layer for Reactor
corpus retrieval. The contract package now exports:

- `openagents.reactor.need_to_know_ruleset.v1`
- `openagents.reactor.corpus_document.v1`
- `openagents.reactor.corpus_access_decision_receipt.v1`
- `evaluateReactorNeedToKnowAccess`

The hard layer checks workspace, matter, and role-or-user scope before any
soft model-oracle verdict is considered. The oracle is downstream only: it can
deny a hard-allowed request when need-to-know is not plausible or missing, but
it cannot rescue a hard-rule denial.

## Adversarial Fixture

The seeded fixture models the explicit "Bob must not see Alice" case from the
Reactor plan:

- Alice may retrieve Alice's strategy memo only when her hard scope matches and
  the oracle returns `need_to_know_plausible`.
- Bob is denied Alice's citation-mode request before the oracle layer.
- Bob is denied Alice's summary-mode request before the oracle layer, even when
  a plausible oracle verdict is supplied.
- Alice is denied her own summary request when the hard rules pass but the
  oracle returns `not_need_to_know`.
- A missing oracle verdict for a hard-allowed request fails closed.
- A deliberately broken allow-all ruleset fails schema decode before
  evaluation.

Every access receipt records who, what, workspace, matter, query intent,
rule-set ref, rule-set version, selected/denied document refs, selected/denied
citation refs, and oracle verdict refs. Receipts explicitly set
`rawDocumentContentLogged: false` and `generatedSummaryContentLogged: false`;
fixtures contain no raw memo text.

## Verification

The guard runs in two places:

- `packages/reactor-contracts/src/index.test.ts`
- `apps/openagents.com/workers/api/src/reactor-need-to-know-access.test.ts`

The Worker test is included in `apps/openagents.com` `check:deploy`, so normal
deployment verification exercises the Bob/Alice citation and summarization
leakage cases.

## Boundary

This clears only the source-level need-to-know access fixture blocker. It does
not create a deployed customer corpus store, customer premises install,
customer data-custody proof, external pilot, public pricing, compliance claim,
payout, settlement, or owner-approved public case-study copy.
