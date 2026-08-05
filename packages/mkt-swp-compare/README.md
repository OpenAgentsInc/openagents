# @openagentsinc/mkt-swp-compare

Multi-provider quote comparison for the MKT-SWP swap widget
(openagents#9318, SWAP-3): the compare table over competing signed
`kind:39605` Quotes, quote-class and reservation-tier disclosure, expiry
enforcement, per-route custody disclosure, and the verify-before-fund
checklist rendered from the engine's report.

This is the surface Boltz structurally cannot have — Boltz shows one price
because Boltz *is* the provider. NIP-MKT Quotes are signed records from
independent providers, so identity, expiry, and competing commitments all
exist as data.

## What lives here

- `model.ts` — the `CompareQuote` projection: quote class
  (`indicative`/`firm`), reservation class (`none`/`soft`/`hard`) with the
  §5 proof-class table, custody disclosure (six §6 dimensions plus BOTH
  duration bounds), fees, expiry, and the finite `selectable` lists.
  `quoteConformance` types the §5 violations (`swp_reservation_missing`,
  `swp_reservation_proof_invalid`).
- `ranking.ts` — the explicit best-execution rule: commitment tier is the
  major key (firm+hard > firm+soft > indicative > nonconforming > expired);
  a cheaper indicative Quote never outranks a firm one. Total order, so
  ranking is deterministic and stable for equal terms (final tiebreak:
  event id).
- `expiry.ts` — countdown state with client-side enforcement. The effective
  bound is the earlier of quote and reservation expiry; an expired Quote is
  `swp_quote_expired` and unusable, not merely styled stale.
- `reservation.ts` — `swp_reservation_fork` detection as a pure function of
  the observed set: retained and attributable, never resolved by arrival
  time.
- `custody.ts` — the fail-closed custody strip. A route reads noncustodial
  only when every control entry is principal/contract/consensus/HTLC; a
  mint or federation route (MKT-MINT enforces `custody_class` at the relay)
  and any unrecognised holder classify custodial.
- `verify.ts` — the engine report port and the funding gate. Verification
  truth arrives from the MKT-SWP engine behind the SWAP-0 boundary as a
  typed §7.1–§7.4 row report; `fundingGate` keeps the fund action disabled
  (`swp_funding_not_authorized`) while any row is unresolved or failed, the
  report is missing or stale, or the engine verdict is not
  `verification_passed`. The UI never computes a profile-level verdict.
- `selection.ts` — Order selection discipline (§4.4): an Order commits the
  exact Quote event id, may choose only from the Quote's finite lists
  (`swp_order_selection_invalid` otherwise), and indicative acceptance is
  only a provider `Status state=accepted` — silence, relay acceptance, an
  invoice, or an address is never acceptance.
- `view.ts` — render-ready table and checklist view models. Every
  (class, tier) commitment gets a distinct badge token so firm/hard can
  never be styled identically to indicative/none; every failing verify row
  is individually identifiable.
- `messages.ts` — local `swap.compare.*` message table. Protocol error
  identifiers render through `@openagentsinc/swap-i18n` (`swap.error.*`).
- `testkit.ts` — the seeded fixture corpus (two independent providers,
  every gate-relevant report shape) used until immortal#14 lands a second
  live provider.

Everything here is UX pre-checking and presentation truth. The MKT-SWP
engine owns verify-before-fund truth; nothing in this package authorises
funding.

## Behaviour contracts

Registry: `@openagentsinc/behavior-contracts` (`market-swap-compare`):

- `openagents_web.swap_compare.firm_indicative_distinct.v1`
- `openagents_web.swap_compare.reservation_proof_class_distinct.v1`
- `openagents_web.swap_compare.quote_expiry_enforced.v1`
- `openagents_web.swap_compare.funding_disabled_until_checks_pass.v1`

## Checks

```
pnpm --dir packages/mkt-swp-compare run check
```
