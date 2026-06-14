# Labor-Market Owner Default Policy (for review)

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
- **The settlement bridge USD→sats (P4 #4780) is unbuilt and hard-gates any paid
  Lane C** (roadmap P4/P7 rows). So #4783's *live* proof cannot honestly close
  until P4 exists; the policy gate itself is already landed default-off.
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
| Funding | **customer USD credits → P4 USD→sats bridge** | **Hard gate**: paid Lane C cannot run until P4 (#4780) exists. Until then Lane C stays a default-off gate with no live spend. |

### Settlement & visibility

| Knob | Default | Reasoning |
|---|---|---|
| Provider settlement | **sats via reliable-tips ladder**, escrow → release → ladder | The proven path (#4777 settled this way: escrow reserve → released_to_provider → ledger). |
| Earnings visibility | **per-job settled sats shown in Pylon + web UI** | P9 visibility law: every payout rung public. |
| Refund on expiry | **escrow refunds to funder on expiry** | Already built (reserve/release/refund arms). No funds stranded. |

## What these defaults unblock vs. what still hard-gates

- **Unblocked now** (no owner value missing): the pricing/consent/preemption
  config surface and earnings visibility for #4782 can be wired and shipped with
  the values above; the Lane C placement policy + opt-in + public-tier floor for
  #4783 can be wired as a default-off gate.
- **Still hard-gated (honest, not owner-fixable by a default):**
  - #4783's *live* proof needs **P4 (#4780) the USD→sats settlement bridge** —
    genuinely unbuilt. A default can't substitute for it.
  - #4782's *acceptance* ("a stranger's paid job settled to the owner's wallet
    same day") needs a **real non-owner counterparty** + P4. The owner-operated
    provider Pylon cannot stand in for a stranger honestly.

So with these defaults applied, the **buildable engineering** of #4782/#4783
proceeds without waiting on the owner; the **live-proof acceptance** of each
remains correctly pending on P4 (#4780) and a real external party, which this
doc records rather than fabricates.
