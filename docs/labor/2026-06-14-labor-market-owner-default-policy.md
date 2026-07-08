# Labor-Market Owner Default Policy (for review)

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-14
Status: **proposed defaults — applied to the landed gates; owner may override any value later**

The owner asked for sensible defaults so the labor-market provider/fanout
features (#4782 P6 spare-capacity provider mode, #4783 P7 Lane C fanout) are not
blocked waiting on per-value decisions. This file is the decision record: each
default, the value, and the reasoning, grounded in the existing roadmap and code.

Authority note: these are **conservative-but-functional** defaults chosen to be
safe to ship before the owner reviews. They never enable spend without the
hard-gated settlement bridge (P4 #4780), and they keep the provider on the
public trust tier only. The owner can override any value; nothing here is
irreversible.

## Background the defaults are grounded in

- **The provider side is paid in sats** over the proven reliable-tips ladder
  (`docs/payments/reliable-tips.md`); Lane A (own jobs) meters USD credits, Lane
  C (market) escrows and settles **sats**
  (`docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`, the lane
  table).
- **Repo trust tiers are the lane selector**: regulated → Lane A only; private →
  Lane A or owner-verified B; **public → any lane**. Paid Lane C is
  **public-tier-only at first** (roadmap "trust tiers", P7 row).
- **The settlement bridge USD→sats (P4 #4780) is built and closed** (the roadmap
  doc's "[new]/L" marker is stale; verified against issue state + code). It funds
  the sats escrow from a buyer's USD credit debit with a rate ref + conversion
  ref. So paid Lane C is no longer bridge-gated; see the dependency-reality
  section below.
- **Existing code defaults**: NIP-90 generic price floor
  `DEFAULT_PROVIDER_PRICE_MSATS = 1_000` (1 sat,
  `apps/pylon/src/provider-nip90.ts`); labor-market policy default
  `priceMsats = 1_000_000` (1000 sats, `apps/pylon/src/labor-market.ts`),
  `maxConcurrentJobs = 1`, `allowedJobKinds = [5934]`, `autoQuote = false`
  (opt-in). First-run operator approval + auth-exfiltration blocking are
  mandatory before any execution (`apps/pylon/src/labor.ts`).
- **P9 settlement-visibility law**: every payout rung is publicly visible
  (roadmap "Settlement visibility law").

## Defaults

### Provider pricing (#4782 pricing face)

| Knob | Default | Reasoning |
|---|---|---|
| Currency | **sats** | The provider side is sats-only by design; no fiat on the provider side. |
| Price floor | **1000 sats/job** (`priceMsats = 1_000_000`) | Matches the landed labor-market default. Above a dust/spam threshold, below a meaningful job's value. The first live job ran at 1 sat for a trivial proof; 1000 sats is the standing floor for real work. The contributor's price is always **their own**, never the platform's. |
| Quoting | **opt-in (`autoQuote = false`)** | Already the code default. A provider must deliberately turn quoting on (`PYLON_LABOR_MARKET_AUTO_QUOTE=true`); silence is never a quote. |

### Job-size bounds (#4782)

| Knob | Default | Reasoning |
|---|---|---|
| Max accepted budget/job | **25,000 sats** | Caps single-job exposure for an unattended provider. Large enough for substantive bounded tasks; small enough that a bug or a bad counterparty can't drain capacity. |
| Max concurrent market jobs | **1** (`maxConcurrentJobs = 1`) | Landed default. Serialize until preemption + earnings are proven under load; raise later. |
| Allowed task kinds | **`code_task` only** | The only kind with a sandbox + verification-command path proven end-to-end (#4777). `review`/`document_work` stay off until each has its own validator. |
| Verification | **required, command-ref'd** (e.g. `command.public.pylon.labor.bun_test`) | No release without validator re-execution passing. Already enforced. |

### Capacity / preemption policy (#4782)

| Knob | Default | Reasoning |
|---|---|---|
| Own-work priority | **own jobs always preempt market work** | The owner's own jobs (Lane A/M4) are the reason the machine exists; a stranger's job must never starve them. Market work runs **only when idle**. |
| Serve-others trigger | **idle only** | The "spare-capacity" contract: unused capacity, not contended capacity. |
| GO ONLINE consent | **default OFF** | Landed default-off. Serving strangers for sats is an explicit owner action, surfaced as a toggle in Pylon + web UI. This default does **not** flip it on. |

### Lane C fanout (#4783)

| Knob | Default | Reasoning |
|---|---|---|
| Customer opt-in | **default OFF, per-order** | A product order only bursts to the market if the **customer** explicitly opts in, per order. Never implicit. |
| Trust-tier floor | **`public` only** (server-enforced) | Only public-tier repos may leave the first-party lanes; private/sensitive/regulated never fan out. Enforced server-side, not client-trusted. |
| Per-order budget ceiling | **customer-set, required** | No fanout without an explicit USD ceiling; quotes auto-accept only under the cap. |
| Funding | **customer USD credits → P4 USD→sats bridge** | The P4 bridge (#4780) is **built/closed**; the funding path exists. Lane C stays a default-off gate; its remaining gate is #4781 + a real product order, not the bridge. |

### Settlement & visibility

| Knob | Default | Reasoning |
|---|---|---|
| Provider settlement | **sats via reliable-tips ladder**, escrow → release → ladder | The proven path (#4777 settled this way: escrow reserve → released_to_provider → ledger). |
| Earnings visibility | **per-job settled sats shown in Pylon + web UI** | P9 visibility law: every payout rung public. |
| Refund on expiry | **escrow refunds to funder on expiry** | Already built (reserve/release/refund arms). No funds stranded. |

## Dependency reality (corrected 2026-06-14)

An earlier draft of this doc repeated the unified roadmap's stale claim that P4
(USD→sats settlement bridge, #4780) was unbuilt. **That is wrong.** Verified
against GitHub issue state + code:

- **#4780 (P4, settlement bridge): CLOSED/COMPLETED** (2026-06-11). The USD→sats
  conversion seam, payout eligibility, and ladder settlement exist
  (`apps/openagents.com/workers/api/src/market-provider-policy.ts`,
  `pylon-bitcoin-accounting-receipts.ts`).
- **#4778 (P2), #4774 (A2), #4762 (M4): CLOSED.**
- **#4777 (P1): CLOSED** (the first live negotiated labor job, this run).
- **#4781 (P5, backlog faucet): OPEN** — the only remaining infra issue in the
  cluster.

So the dependency picture is:
- **#4782** (spare-capacity provider): **all deps satisfied** (M4 ✓, P1 ✓, P4 ✓).
- **#4783** (Lane C fanout): deps P2 ✓, P4 ✓; **blocked only by #4781**.

## What still hard-gates (honest, not owner- or default-fixable)

The remaining gate across #4781/#4782/#4783 is **not** missing plumbing or a
missing owner value — it is **real market participation** that a solo operator
cannot fabricate:

- **#4781** acceptance: "...at least one [issue] quoted and completed by a
  **non-owner-operated provider**, settled with public receipts."
- **#4782** acceptance: "...a **stranger's** paid job settled to the owner's
  wallet the same day..."
- **#4783** acceptance: "One **real product order**... completes via a market
  provider end to end."

All three require a genuine second party (a non-owner provider, a stranger's
paid job, or a real customer order). The owner-operated provider Pylon
(`3fd9b3f1…`) cannot honestly stand in for a stranger — doing so would be the
agent-claims-completion anti-pattern INVARIANTS forbid.

**Therefore:** the **buildable engineering** of #4781/#4782/#4783 (faucet
adapter, pricing/consent/preemption config, Lane C placement policy + opt-in +
tier floor, earnings visibility) proceeds now with these defaults — no owner
input needed. The **live-proof acceptance** of each remains correctly pending on
a real external market participant, which this doc records rather than fakes. The
owner's only genuine decision here is whether to (a) accept these as
"engineering-complete, live-proof-pending-real-market," or (b) recruit/seed a
real second provider; the defaults above remove every *other* blocker.
