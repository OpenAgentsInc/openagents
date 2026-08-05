# Boltz swap pause — AI-assisted attack pressure and ecosystem SPOF assessment

Status: **incident assessment and discourse ledger** (claims + timeline + ecosystem
impact). Not a legal finding, not a public product claim, and not an
authorization to run, fork, or operate Boltz infrastructure.

Last updated: 2026-08-05 (~06:20Z probe).

Related machine receipt:

- [`receipts/2026-08-05-72h-discourse-sweep.json`](receipts/2026-08-05-72h-discourse-sweep.json)
  — ~336 unique X posts in a ~72h window (2026-08-02 06:00Z → 2026-08-05 06:20Z)
  plus full text of primary posts.

Related OpenAgents architecture work (not this incident’s authority):

- [`../teardowns/2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md`](../teardowns/2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md)
- [`../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md`](../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md)
- [`../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md`](../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md)
- [`../nips/MKT.md`](../nips/MKT.md) / [`../nips/MKT-SWP.md`](../nips/MKT-SWP.md)
- Episode 266 transcript: [`../transcripts/266.md`](../transcripts/266.md)

---

## Working assessment

**Boltz** (`@Boltzhq`), the dominant non-custodial atomic-swap service between
Bitcoin mainchain, Lightning, Liquid, and other rails, **disabled swap services
indefinitely on 2026-08-03**. The operator’s public story is not “one CVE and a
hot patch,” but a **months-long rise in automated, AI-assisted probing and
several contained exploits**, with a **drastic acceleration in the past few
days**, such that attackers iterate faster than a bootstrapped team can find and
patch. They state they are **actively targeted by multiple resourceful groups**
while racing to deploy fixes, call this a **paradigm shift for open-source
Bitcoin services**, and warn **not to expect swaps to resume shortly**.

**User funds:** Boltz and dependent wallets repeatedly claim **no user funds were
at risk** (atomic / non-custodial design). Losses from contained exploits were
**operator-borne**. Cooperative refund API remains; unilateral refunds work
without Boltz infrastructure.

**Ecosystem effect:** Lightning and Liquid **swap UX** failed or degraded across
wallets that treated Boltz as default plumbing (Bull Bitcoin, Aqua / JAN3,
Blockstream app, ZEUS’s own Boltz instance, Geyser, Rootstock Atlas paths,
HodlHodl swap partners, etc.). Liquid itself and on-chain send/receive largely
continued. That is a **availability / SPOF failure**, not a mass custody drain.

**Concurrent context (same week):** Coldcard entropy drains (~$100M+ class
social totals) and a second bridge-class outage (**Satora / LendaSwap**,
2026-08-04 discourse) amplified “Bitcoin infrastructure under attack”
narratives. Correlation in time is documented; **shared operators or shared
attacker identity is not proven** from public posts alone.

**OpenAgents lens:** this week’s events are strong **empirical pressure** for
multi-provider, client-verified, Nostr-discovered liquidity (NIP-MKT /
MKT-SWP) rather than a single HTTP swap monopoly. Architecture teardowns and
Episode 266 already point that way; this document is the **incident ledger**.

---

## Claim table

| ID | Claim | Status | Best public support | What would flip it |
| --- | --- | --- | --- | --- |
| **B1** | Boltz disabled swaps “until further notice” on 2026-08-03 | **Documented** | Official X posts E1–E2 | Operator resume announcement |
| **B2** | Cause is multi-month AI-assisted probing + contained exploits, not one isolated bug | **Operator claim** | E2 full statement | Independent postmortem, CVE list, exploit writeups |
| **B3** | Attack rate exceeds small-team patch rate (asymmetric defender problem) | **Operator claim; widely echoed** | E2; PPQdotAI E12; funding calls | Metrics (patch latency vs exploit cadence) |
| **B4** | No user funds at risk; non-custodial; losses were Boltz’s | **Operator + wallet claims** | E2; Bull/Aqua/Blockstream | On-chain counterexample of user loss via Boltz custody |
| **B5** | Multiple resourceful groups actively targeting Boltz | **Operator claim** | E2 “multiple resourceful groups” | Attribution report; LE/intel |
| **B6** | State-sponsored attackers | **Speculation only** | OpenAgents social line E13 | Hard attribution; do not promote without evidence |
| **B7** | Dependent wallets lost Lightning/Liquid *swap* capability, not custody | **Documented** | E4–E9 wallet PSAs | — |
| **B8** | Industry was over-dependent on one swap provider (SPOF) | **Strong (behavioral)** | Mass simultaneous degradation; Excellion E10; januszg E16 | — |
| **B9** | Vulnerabilities were in Boltz **swap server**, not Lightning consensus | **Expert social claim** | roasbeef E11; Jestopher clarifications | Boltz technical postmortem |
| **B10** | Aug 1 EVM swap disable was a separate “EVM integration bug” phase | **Documented** | E0 | Link-or-separate postmortem |
| **B11** | Satora/LendaSwap outage is independent or cascade of Boltz | **Open / mixed** | milessuter E14; Satora teardown notes Boltz as LN component | Satora root-cause statement |
| **B12** | Multi-vendor “Boltz instances” alone fix SPOF if they share vulnerable core libs | **Open / warned** | francispouliot E17 | Diversity of implementations + independent audits |

**Working preference:** B1, B4, B7, B8 are load-bearing and well supported. B2–B3
are credible operator testimony pending postmortem. B6 is **not** established.
B9 is the best technical framing of “what broke” until Boltz publishes details.

---

## Timeline (UTC, approximate)

| When | Event | Source |
| --- | --- | --- |
| **2026-08-01 ~12:15** | EVM swaps (USDT/USDC/TBTC/WBTC/RBTC) disabled; LN/Liquid/on-chain “unaffected” | E0 `@Boltzhq` |
| **2026-08-03 ~09:54** | All swap services unavailable until further notice; no ETA | E1 `@Boltzhq` |
| **2026-08-03 ~16:12** | Long statement: indefinite disable; AI-assisted probing months; contained exploits; acceleration last few days; multiple groups; refunds OK; no user funds at risk; don’t expect quick resume | E2 `@Boltzhq` (~1323 likes) |
| **2026-08-03 afternoon** | ZEUS pauses own Boltz instance; Bull/Aqua/Blockstream/Manna/Geyser/Rootstock paths notify users | E3–E9 |
| **2026-08-03 evening** | Francis Pouliot “Token War” framing; Coldcard + Boltz same week | E4 |
| **2026-08-03 ~20:14** | OpenAgents flags pause; later Ep. 266 SPOF/Nostr markets | E13, E15 |
| **2026-08-04** | Excellion: “too dependent on Boltz” for LN/LQ redundancy | E10 |
| **2026-08-04** | roasbeef: no critical LN money-losing bug reports; issues were swap server | E11 |
| **2026-08-04** | Second bridge narrative: Satora/LendaSwap down same 24h window | E14; Satora teardown |
| **2026-08-04–05** | Discourse shifts to “who is the next Boltz?” and naming hygiene (“Lightning wallet” vs swap-dependent app) | E16–E18; Atlantis RT cluster |

---

## Part I — Primary operator statements

### E0 — EVM-only disable (2026-08-01)

**Primary:** [2083527047083704753](https://x.com/Boltzhq/status/2083527047083704753)

> EVM swaps (USDT, USDC, TBTC, WBTC, RBTC) are currently disabled while we fix a
> bug in our EVM integration.
>
> Lightning, Liquid and onchain BTC swaps are unaffected and running normally.
> No user funds are at risk.

**Use:** shows **pre-pause partial degradation** two days earlier; later full
pause recasts the week as escalating, not a green-field black swan.

### E1 — Initial full pause (2026-08-03 ~09:54Z)

**Primary:** [2084216278181359930](https://x.com/Boltzhq/status/2084216278181359930)
(~210 likes)

> Boltz Swap Services are currently unavailable until further notice. We can't
> give an ETA as of this time, but will provide an update once we know more 🙏

### E2 — Indefinite disable + AI-assisted attack narrative (2026-08-03 ~16:12Z)

**Primary:** [2084311537502630319](https://x.com/Boltzhq/status/2084311537502630319)
(~1323 likes — highest-engagement primary)

> Update: Boltz will stay disabled until further notice.
>
> Our API remains available to process refunds cooperatively. In any case,
> unilateral refunds will work, as they do not depend on our infrastructure.
>
> Our support team stays reachable.
>
> To be clear: this is not a response to a single incident. Over the past months
> we have seen a steady rise in automated, AI-assisted probing of our
> infrastructure, and we have dealt with several exploits. Each was contained,
> but the pattern is clear: attackers now iterate faster than a team our size
> can find and patch. In the past few days alone we saw a drastic acceleration,
> and we do not believe this asymmetry will reverse. After reviewing the results
> of our own recent security scans, we cannot responsibly re-enable Boltz swaps,
> especially as we are being actively targeted by what appear to be multiple
> resourceful groups while we race to deploy fixes.
>
> What we are seeing is a major paradigm shift for Bitcoin services operating on
> an open source stack, and it needs careful analysis. Do not expect swap
> services to resume shortly.
>
> To be explicit: no user funds were ever at risk. Boltz is non-custodial by
> design. And as a fully bootstrapped company, the losses were ours alone.
>
> We don't know yet how things will continue from here, but we'll keep you
> posted as soon as we have had the time to catch our breath and make a decision 🙏

**Extractable operator claims (for later falsification):**

1. Not a single incident — multi-month pattern.
2. AI-assisted / automated probing.
3. Several exploits — each **contained**.
4. Attacker iteration > defender patch capacity.
5. Acceleration in last few days (same window as Coldcard public crisis).
6. Active targeting by **multiple resourceful groups**.
7. Internal security scans → cannot responsibly re-enable.
8. Refunds: cooperative API + unilateral paths without Boltz infra.
9. User funds never at risk; losses operator-only.
10. Resume not short-term.

Press amplification (secondary): Cointelegraph / TradingView, The Defiant
(2026-08-03), Decrypt, Gadgets360, etc. — useful for reach, not primary facts
beyond quoting E2.

---

## Part II — Ecosystem impact (dependent surfaces)

| Surface | Public response (72h) | Impact class |
| --- | --- | --- |
| **Bull Bitcoin** (`@francispouliot_`, `@BULLBITCOIN_`) | Immediate priority shift to restore LN + Liquid↔BTC; Liquid federation membership → no stuck L-BTC claim; day-of advisory | LN/swap UX down; custody claimed safe |
| **Aqua / JAN3** (`@AquaBitcoin`, `@Excellion`, `@JAN3com`) | LN + Liquid swaps down foreseeable future; help offered to Boltz; later: “too dependent on Boltz” for LN/LQ redundancy; AgenticAQUA / SamRock paths noted | Same |
| **Blockstream app** | In-app swaps unavailable; Liquid unaffected | Swap UX only |
| **ZEUS** (`@ZeusLN`) | Own Boltz instance `swaps.zeuslsp.com` paused “following suit” | Self-hosted instances still share risk model |
| **Manna** | Swaps disabled; Liquid peer send/receive OK | Swap UX |
| **Geyser** | Contributions/payouts as swaps blocked; funds “not at risk” | Fiat-ish product path via swaps |
| **Rootstock Atlas** | Boltz route unavailable; other Atlas routes claimed available | Partial |
| **HodlHodl** | Swap partners Satora + Boltz paused → Lightning *trade* availability hit; LN/Liquid still payment options for on-chain trades | Marketplace UX |
| **DFX / others** | Some claimed independent LN swap paths still up | Competitive alternative signal |

### E3 — ZEUS follows suit

**Primary:** [2084316041673347138](https://x.com/ZeusLN/status/2084316041673347138)
(~182 likes) — pauses own instance after Boltz official statement.

### E4 — Francis Pouliot / Bull (priority shift + “Token War”)

**Primary:** [2084317107890614275](https://x.com/francispouliot_/status/2084317107890614275)
(~564 likes)

- Support Boltz decision; close collaboration.
- Liquid federation → convert L-BTC without third party; no stuck funds claim.
- Until fixed, LN payments and Liquid↔BTC swaps fail without explanation.
- Links **Coldcard exploit** and Boltz offline as attackers “scoring major
  victories” in a “Token War.”

Later advisory: [2084375359521456471](https://x.com/francispouliot_/status/2084375359521456471).

### E5 — Excellion / Aqua restore priority

**Primary:** [2084349344757563695](https://x.com/Excellion/status/2084349344757563695)
(~277 likes) — restore LN/LQ swaps top priority; offered help to Boltz; funds
safe; other L-BTC→BTC/USDT paths claimed.

### E6 — Aqua product account

**Primary:** [2084354313720135702](https://x.com/AquaBitcoin/status/2084354313720135702)
(~139 likes) — all Lightning and Liquid **swaps** unavailable foreseeable future;
other features operational.

### E7 — Blockstream app

**Primary:** [2084340794660388895](https://x.com/Blockstream/status/2084340794660388895)
(~92 likes) — partner interruption; Liquid unaffected.

### E8–E9 — JAN3 / AgenticAQUA note

JAN3 product post (discourse sweep) notes Lightning in Aqua and AgenticAQUA /
SamRock+BTCPay paths affected; on-chain and Liquid non-Boltz paths claimed OK.

### E10 — Explicit SPOF admission (Excellion)

**Primary:** [2084484573056876750](https://x.com/Excellion/status/2084484573056876750)
(~306 likes)

> We were too dependent on Boltz. While we added redundancy in @AquaBitcoin for
> USDT swaps, we didn’t think it was a priority to have redundancy for LN/LQ.
> That was a mistake on our part. It will be fixed.

**Use:** strongest **dependent-operator** confirmation of B8.

---

## Part III — Technical and industry readings

### E11 — roasbeef: swap server, not Lightning consensus bug

**Primary:** [2084553390705504272](https://x.com/roasbeef/status/2084553390705504272)
(~84 likes; reply context with Excellion / peterktodd)

> we've received no reports of any critical money losing bug
>
> the vulnerabilities that caused Boltz to shut down had to do with their swap
> server

**Use:** separates **Lightning protocol health** from **coordinator/server
surface**. Aligns with “non-custodial swap still has a hot operator attack
surface” (API, rate logic, EVM integration, channel management, etc.).

### E12 — PPQdotAI: not unique to Boltz

**Primary:** [2084376158842306729](https://x.com/PPQdotAI/status/2084376158842306729)
(~106 likes)

- Fighting AI-powered exploits every other week for months.
- Points to Shannon tooling as one defensive aid.

**Use:** B3 peer corroboration (small teams under AI probe pressure) without
proving Boltz’s internal details.

### E14 — Second bridge: Satora / LendaSwap

**Primary social:** [2084614621260923119](https://x.com/milessuter/status/2084614621260923119)
(~105 likes) — another bitcoin bridge down within 24h of Boltz.

**Architecture note:**
[`../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md`](../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md)
documents `api.satora.io` 502 and notes Satora regtest/images historically
included Boltz for LN legs — **cascade possible, not proven** as root cause.

### E16 — januszg: best product, bad naming

**Primary:** [2084695018137166230](https://x.com/januszg_/status/2084695018137166230)
(~46 likes)

- Outage proves Boltz was best-in-class swap product.
- Stop calling third-party-swap-dependent apps “Lightning wallets.”

### E17 — francispouliot: shared libraries defeat multi-vendor theater

**Primary:** [2084828745068618050](https://x.com/francispouliot_/status/2084828745068618050)
(~73 likes; reply to calle)

> even if there are many vendors of a particular protocol (like Boltz, there are
> many vendors) and they are all running the same core libraries/dependencies
> and the same libraries all have the same issues, then having multiple vendors
> doesn't matter.

**Use:** B12 — multi-instance Boltz without **implementation diversity** is a
false redundancy.

### E18 — Jestopher: L2 connective tissue / “next Boltz”

**Primary:** [2084794303793238366](https://x.com/Jestopher_BTC/status/2084794303793238366)
(~16 likes)

- Boltz fundamental for connecting layers; **Delegated Lightning Management**
  called out.
- Gap for L2 bridges; Amboss not taking the whole role but can help LN half.

### E19 — calle: under-discussed wallet cascade

**Primary:** [2084811772335145049](https://x.com/callebtc/status/2084811772335145049)
(~208 likes)

> We haven't even talked about Boltz Lightning going offline and taking
> Lightning in Aqua, Bull Bitcoin, Blockstream Wallet, Arkade, etc. with it.

### Other notable 72h voices (receipt)

- `@sms4sats`: critical infrastructure; build decentralized alternatives or
  economy takes a hit; later Nostr market of Boltz instances idea.
- `@dieguito` / Rootstock: support for responsible pause.
- `@steepdawn974`: Electrum swaps as alternative; Breez self-host/federation
  history.
- `@lucasdcf` (Vinteum): funding gap for OSS vs AI-powered attackers (press
  quotes).
- Alternatives discourse: SideSwap, SideShift, Electrum makers, DFX partner
  paths — incomplete competitive map.

---

## Part IV — `@AtlantisPleb` / OpenAgents lane (seed + product reaction)

### Seed for this document (2026-08-05 UTC)

AtlantisPleb posted **no original Boltz commentary** on 2026-08-05; **four
reposts** only (amplification of E16–E19 cluster):

| Time UTC | Atlantis RT | Source |
| --- | --- | --- |
| 00:50 | [2084804241537814825](https://x.com/AtlantisPleb/status/2084804241537814825) | januszg E16 |
| 03:02 | [2084837393673207964](https://x.com/AtlantisPleb/status/2084837393673207964) | calle E19 |
| 03:02 | [2084837515706397054](https://x.com/AtlantisPleb/status/2084837515706397054) | francispouliot E17 |
| 04:56 | [2084866209002954950](https://x.com/AtlantisPleb/status/2084866209002954950) | Jestopher E18 |

### Earlier 72h Atlantis / OpenAgents posts

| Time | Actor | Link | Point |
| --- | --- | --- | --- |
| 2026-08-03 ~20:06 | Atlantis RT | of E2 | Amplifies official indefinite pause |
| 2026-08-03 ~20:14 | **OpenAgents** | [2084372335176397133](https://x.com/OpenAgents/status/2084372335176397133) | Flags pause; quotes “multiple resourceful groups”; social line “state-sponsored” (**B6 — not established**) |
| 2026-08-03–04 | Atlantis RTs | francis advisory, sms4sats, lucasdcf, turtlecute, milessuter, roasbeef | Ecosystem + tech framing |
| 2026-08-04 ~08:15 | **OpenAgents** | [2084553883779559539](https://x.com/OpenAgents/status/2084553883779559539) Ep. 266 | SPOF; tbDEX→Nostr; **NIP-MKT**; Immortal relay; multi-provider swap network |
| 2026-08-04 ~13:54 | OpenAgents + Atlantis RT | architecture/code fan-in | Boltz + Satora + tbDEX + Arkade/Mostro/Cashu/WDK |

Episode 266 transcript (machine, verify before quote-grade use):
[`../transcripts/266.md`](../transcripts/266.md).

---

## Part V — Scenario map (interpretation, not findings)

| # | Scenario | Fit | Notes |
| --- | --- | --- | --- |
| **S1** | Honest small team overwhelmed by AI-augmented attackers | **Best public fit** | Matches E2; PPQdotAI peer story; no user-drain contradiction |
| **S2** | Responsible self-disable after self-scan finds more surface | Compatible with S1 | “cannot responsibly re-enable” while under fire |
| **S3** | EVM bug (Aug 1) was early symptom of same campaign | Plausible | Needs postmortem linkage |
| **S4** | Acceleration tied to same attackers / week as Coldcard | Speculative | Time correlation only |
| **S5** | State-sponsored campaign | **Weak public evidence** | OpenAgents social line; Boltz said “resourceful groups,” not states |
| **S6** | Coordinated wallet industry failure independent of Boltz quality | Weak | Outage pattern is dependency graph, not random |
| **S7** | Multi-instance open-source Boltz clones restore capacity quickly | Incomplete | ZEUS already ran one and paused; francis B12 library-monoculture risk |

**Working preference:** S1–S2. Preserve S3–S4 as open research. Do not treat S5 as
fact.

---

## Assessment

### What is solid

1. **Boltz is (was) critical plumbing** for “Lightning UX” in several major
   Bitcoin wallets — not because LN failed, but because **submarine / reverse /
   chain swaps and delegated LN management** were concentrated on one operator
   API.
2. **Non-custodial physics ≠ high availability.** Users keep keys; the
   **coordinator** can still be burned down by server exploits, ops load, or
   responsible self-disable.
3. **Refund path messaging is consistent** (cooperative API + unilateral
   refunds). That is the correct atomic-swap user-safety story if true.
4. **Industry self-critique is real:** Excellion’s “too dependent,” januszg’s
   naming hygiene, sms4sats’ “critical infrastructure,” OpenAgents Ep. 266 SPOF
   thesis.
5. **Same-week stack:** Coldcard drains + Boltz pause + Satora darkness = worst
   multi-surface Bitcoin infrastructure week of 2026 so far in public discourse.

### What is not solid

1. **Exact vulnerabilities** (CVEs, root causes, which swap types, EVM vs LN
   server components) — withheld pending postmortem.
2. **Attribution** of attacker groups (AI tooling claim is operator testimony).
3. **State sponsorship** — not supported by Boltz’s wording.
4. **Whether Satora failed because of Boltz** — architectural dependency
   possible; public root cause not published.
5. **Resume timeline** — operator says not soon.

### Implications for OpenAgents (product strategy, not admission)

- The week validates **multi-provider discovery + client-verified settlement**
  over “integrate Boltz REST and ship.”
- NIP-MKT / MKT-SWP + Immortal work is a **response class**, not a claim that
  OpenAgents currently replaces Boltz production liquidity.
- Do **not** overclaim “state-sponsored” in product or promise surfaces.
- Watch for: Boltz postmortem; wallet multi-homing announcements; any user-loss
  counterexamples; clone/instance diversity vs library monoculture.

---

## Open watchlist

| Item | Priority | Notes |
| --- | --- | --- |
| Boltz technical postmortem / resume / wind-down decision | High | Flips B2–B5, resume ETA |
| Independent confirmation of “several contained exploits” | High | On-chain operator losses? Bug trackers? |
| Wallet multi-provider launches (Bull, Aqua, Blockstream, ZEUS) | High | B8 remediation evidence |
| Satora root cause vs Boltz cascade | Medium | B11 |
| Library monoculture map (boltz-core, common HTLC stacks) | Medium | B12 |
| Refund backlog / user stuck-swap reports | Medium | Stress-test B4 |
| Relation to Coldcard week acceleration claim | Low/speculative | S4 only |
| NIP-MKT prototype status in this repo | Internal | Architecture docs, not this incident ledger |

---

## Discourse method note

- X API recent search ~72h from 2026-08-02 06:00Z, queries covering `@Boltzhq`,
  pause/AI/exploit language, wallet cascade handles, OpenAgents/Atlantis, and
  secondary bridges.
- ~**336** unique posts retained in the machine receipt; engagement counts are
  probe-time, not frozen truth.
- News outlets used as secondary amplifiers of E2, not independent technical
  authority.

---

## Explicit non-goals

- Running `boltzd`, opening channels, or calling production `api.boltz.exchange`
  as part of this document.
- Publishing exploit PoCs or attack playbooks.
- Treating social “AI attack” as AssuranceSpec-grade proof without operator
  technical evidence.
- Product promises that OpenAgents already provides Boltz-class global liquidity.
