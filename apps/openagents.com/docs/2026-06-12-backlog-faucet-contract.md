# Backlog Faucet Contract (2026-06-12, #4781)

## Scope honesty, first

This lane lands the **faucet contract and the no-spend path only**.

- What exists after this change: a typed contract that turns one budgeted
  backlog item into a validated NIP-LBR kind-5934 work-request payload plus a
  faucet record whose lifecycle is operator-spend-gated, and a dry-run path
  that builds, validates, and records **without publishing and without
  escrowing**.
- What does **not** exist after this change: no real backlog issue has been
  published to the live market through this contract, no escrow has been
  reserved, no provider has quoted, and nothing has settled. The live paid
  leg — three real backlog issues listed, at least one quoted and completed by
  a non-owner-operated provider, settled with public receipts — **remains
  P5's open evidence gap** (#4781's acceptance criteria). Per the standing
  invariant, P1/P5/P6/P7 live paid-labor claims stay blocked until those
  receipts are public and dereferenceable.

## Why now

The preconditions for the paid leg landed tonight:

- **#4863** assigned `relay.openagents.com` as the canonical market relay
  domain (live-verified NIP-11 + ws REQ/EOSE), so work-request twins have a
  stable public address.
- **#4864** published the provider Nostr pubkey, relay ref, and lane refs in
  `/api/pylons`, so external providers can discover who serves the market and
  where.
- **#4777** (P1) landed the env-gated live market-key publisher
  (`workers/api/src/forum-work-request-live-publisher.ts`), which signs the
  ref-only kind-5934 draft and publishes it to the relay — but only when the
  operator has configured `FORUM_WORK_REQUEST_MARKET_SECRET_KEY`.

What was missing between "we can publish" and "the backlog flows" was the
typed seam: a contract that validates a budgeted issue, produces the exact
payload that would publish, and refuses to publish without an explicit
operator decision. That seam is this lane.

## The contract (`workers/api/src/backlog-faucet.ts`)

### Input: a budgeted backlog item

`BacklogFaucetBudgetedItem` — `issueRef` (the public GitHub issue URL; the
objective stays a dereferenceable ref, never an issue-body copy), `title`,
`boundedScope` (a single 8–280 character public-safe line), `budgetSats`,
`verificationCommandRef`, `deadlineDate`, optional capability refs and relist
generation.

Validation, all typed (`BacklogFaucetGateError` with a `reasonRef`, or
`ForumWorkRequestUnsafe` from the shared work-request validation):

- budget bounds: positive integer sats, capped at
  `DefaultBacklogFaucetMaxBudgetSats` (50,000) unless the caller passes an
  explicit lower/higher cap;
- public-safe refs: the item, the projection, operator approvals, publish
  receipts, and transition refs all pass the shared work-request unsafe-material
  scanner; issue bodies never enter the filing;
- verification command required: acceptance downstream is
  validator-verdict-gated per the labor escrow invariants, so a filing without
  a verification command ref is rejected at draft time.

### Output: payload + record

- The **NIP-LBR payload**: the record carries `previewDraft`, the unsigned
  kind-5934 event built through the same `packages/nip90` LBR builders
  (`makeLbrAgenticCodingRequest` → `lbrAgenticCodingRequestToDraft`) that the
  live Forum work-request surface uses, validated end-to-end at draft time.
  The filing's `ForumWorkRequestInput` is exactly the body that
  `POST /api/forum/work-requests` accepts; live publication rebuilds the
  draft with the real Forum topic ref through that existing surface rather
  than inventing a parallel publish path.
- The **faucet record** (`BacklogFaucetRecord`): states are
  `drafted → approved_for_publication → published → quoted → quote_accepted →
  running → delivered → accepted → settled`, with `expired`/`cancelled`
  terminal arms. Every transition appends a bounded history entry (max 32)
  carrying refs, and rebuilds `publicProjectionJson` with `generatedAt` and
  the declared staleness contract (`rebuilt_on_faucet_state_transition`).

### The spend gate (typed, not bypassable)

`approved_for_publication` exists only through
`approveBacklogFaucetForPublication`, which requires a
`BacklogFaucetOperatorApproval`: a typed `operator.*` ref, an ISO approval
instant, and an integer spend cap covering the filing budget. The generic
`advanceBacklogFaucetState` cannot reach `approved_for_publication` or
`published` — those states are excluded from its input type, not just checked
at runtime. `markBacklogFaucetPublished` additionally requires the spend gate
to be in `operator_approved` state and a relay-accepted publish receipt with a
real 64-hex job event id, so an unpublished or rejected publish attempt can
never mark a record published.

The `delivered → accepted` transition requires a validator verdict ref on top
of the receipt ref — acceptance authority is the validator policy, never the
worker, matching the Labor Escrow Credit Ledger invariants.

### The no-spend dry run

`dryRunBacklogFaucetItem` returns `{ published: false, escrowed: false,
previewDraft, record }` with the record in `drafted` state, the spend gate
`not_approved`, and the publication `not_published` — by construction, not by
configuration. The regression suite demonstrates the full path on an
**example payload built from issue #4781 itself** (clearly labeled as an
example, not a live filing) in `workers/api/src/backlog-faucet.test.ts`.

## Rails reused, not rebuilt

- `forum-work-requests.ts`: `ForumWorkRequestInput`,
  `normalizeForumWorkRequestInput`, `buildForumWorkRequestLbrDraft`, and the
  (newly exported) public-safe scanner `assertPublicSafeWorkRequestMaterial`.
- `packages/nip90` LBR builders (kinds 5934/6934/7000 per `lbr.test.ts`).
- The existing backlog-faucet selection/decoration surface from the
  marching-orders lane (#4757): channel exclusivity, objective/idempotency
  refs, relist policy, and issue-comment mirrors are unchanged and reused by
  `draftBacklogFaucetRecord` via `buildBacklogWorkRequestFiling`.
- The live publisher connector from #4777 is the intended producer of the
  publish receipt that `markBacklogFaucetPublished` consumes; the dry-run path
  never touches it.
- Escrow stays on the labor credit ledger (`labor-escrow.ts`); this contract
  records escrow receipts as transition refs and never moves balances itself.

No new operator HTTP route is added in this lane: like
`artanis-labor-requester.ts`, the contract is a pure dependency-injected
module whose live side effects (publication, escrow) remain explicit operator
configuration tasks. Wiring a route adds spend surface and belongs to the
operator-gated publication lane, not the contract lane.

## Remaining P5 work (honest list)

1. Operator approves and publishes real budgeted backlog issues through
   `POST /api/forum/work-requests` with the live market key configured.
2. Escrow reserves on acceptance from the maintainer/platform ledger balance.
3. Lifecycle receipts mirror to the Forum thread and the GitHub issue
   (comment bodies already exist in this module).
4. The acceptance criteria: three real listings, one completed by a
   non-owner-operated provider, settled with public receipts.

Refs: #4781 (this lane), #4777 (live publisher), #4863/#4864 (relay and
pubkey discoverability preconditions), #4757 (the in-house precursor),
`docs/labor/2026-06-10-open-agent-labor-market-roadmap.md`.
