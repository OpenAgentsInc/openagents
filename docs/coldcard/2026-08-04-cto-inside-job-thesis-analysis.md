# Coinkite CTO / “inside job” thesis — claim ledger

Status: **claim ledger + scenario map + collection plan.**  
Detailed evidence rows live in
[`2026-08-04-cto-thesis-evidence-detail.md`](2026-08-04-cto-thesis-evidence-detail.md).  
Forensic closes of 2026-08-04 Fable pass: **E24** in that file.  
Archive digests: [`receipts/2026-08-04-cto-thesis-archive-index.md`](receipts/2026-08-04-cto-thesis-archive-index.md).  
On-chain next steps: [`2026-08-04-on-chain-keyspace-prior-drain-plan.md`](2026-08-04-on-chain-keyspace-prior-drain-plan.md).

This is **not** a legal finding and **not** a public product claim. It records
what public evidence supports, what is open, and what single artifact would
change each claim’s status.

Last updated: 2026-08-04 (switck X archive, Galaxy collectors, public RNG PoC).

---

## Working thesis

The Coldcard entropy failure and 2026 drains involve more than a random junior
mistake — **at minimum Coinkite CTO Peter Gray** (`@DocHex` / GitHub `doc-hex`)
is central to how the defective code entered the product (including work under
GitHub nym `switck`). Social discourse further claims intentional theft /
complicity. **Identity and authorship are strongly evidenced; intentional theft
and staff participation in drains are not.**

---

## Claim table (A–I)

| ID | Claim | Status | Best public support | **What single artifact flips status** |
| --- | --- | --- | --- | --- |
| **A** | Gray’s personal OpenPGP key signed `switck`-attributed libngu commits (incl. critical RNG adapter) | **Strong** | Local `git verify-commit` Good signature as Peter D. Gray; 77/123 commits (detail E3) | Demonstrated **key compromise** with alternate signer timeline, or Coinkite proof key was shared with a third party for those commits |
| **B** | Gray authored/integrated the security-critical seed path (libngu + firmware) | **Strong** | Dual authorship on `switck/libngu`; firmware `b18723dd` “First pass w/ libNgU” by `peter@conalgo.com` (E4–E5); `switck/firmware` is overwhelmingly Gray commits pre-2021 (E24) | Proven **misattribution** of those git identities (forgery) |
| **C** | Defect was **intentionally** planted as a long-term theft backdoor | **Open / not established** | Empty subjects, nym OPSEC, retirement-attack joke (E7/E10) — all dual-use | Contemporaneous **intent doc** (ticket/chat/design) or insider testimony with corroboration |
| **D** | Gray or Coinkite staff **participated in or directed** 2026 drains | **Open / not established** | None on-chain to staff | **Staff ↔ collector/proceeds** link (KYC, device, confession, LE) |
| **E** | Remediation messaging is cover for ongoing theft | **Ambiguous** | Dual-use with standard incident response | Evidence migration guidance was **chosen to maximize** attacker window (internal) |
| **F** | 2021 “retirement attack” language implies premeditated backdoor | **Partial** | Official definition is real (E10); heist inference speculative | Link from that joke to **implemented** plant design |
| **G** | Vendor had prior victim reports / notice years before Jul 2026 | **Collecting** | Multi-source social claims (E12/E18); conflicts with E17 “unaware until today” | **Primary ticket/email** *or* pre-2026 victim address ∈ weak keyspace (on-chain) |
| **H** | Early public advisory understated model blast radius | **Documented** | Day-0 “Mk4/Q/Mk5 not affected” → later ~72-bit impact (E11) | N/A (already documented); further flips only refine timeline |
| **I** | Official tech post admits first-person responsibility for zeroed RNG macro / wrong path | **Documented** | [entropy-technical-backgrounder](https://blog.coinkite.com/entropy-technical-backgrounder/) (E17) | Attribution of “I” to a different named engineer with evidence |

---

## Scenario map (speculative)

These are **interpretation sketches**, not findings. The compact table is a
navigation aid; the essays below carry the real argument. Full cross-links to
evidence rows live in
[`2026-08-04-cto-thesis-evidence-detail.md`](2026-08-04-cto-thesis-evidence-detail.md).

| # | Scenario | Fit to public evidence | Theft guilt |
| --- | --- | --- | --- |
| **S1** | Tragic integration negligence | Best fit for mechanism (E17, afilini) | Low |
| **S2** | Negligence + defensive denial / mis-triage of early drains | Explains G vs E17 tension if tickets exist | Medium process |
| **S3** | Reckless opacity (nym/OPSEC) without heist plan | Fits A/B + empty commits + dual identity | Reputation high; theft unproven |
| **S4** | Informed non-fix after signals | Needs internal docs | Moral/civil |
| **S5** | Inside-assisted exploit of known weak space | Needs proceeds link | Criminal if true |
| **S6** | Long-game intentional plant | Weak vs MicroPython-fallback composition + “I didn’t know PRNG in tree” | Max allegation |
| **S7** | Day-0 RFC6979 AI post as “tell” | E23 timing + AI theme | Weak alone |
| **S8** | Day-0 RFC6979 AI post as honest wrong theory | Wrong class + darosior correction | Non-guilt on this artifact |

**Working preference:** S1–S3 are the least strained under the public record.
S5–S6 remain the viral story without a staff↔proceeds bridge. E23 alone sits
with S7/S8, not S6.

### S1 — Tragic integration negligence

In this reading the story is almost ordinary, which is what makes it
devastating. A small team moves elliptic-curve work onto Bitcoin Core’s
libsecp256k1 through an embedded helper (libNgU). Seed generation is redirected
from a direct hardware-RNG call onto the helper’s `random.bytes()` path. Two
implementations share a symbol name; a preprocessor guard tests existence rather
than truth; MicroPython’s software PRNG fills the gap; the build stays green.
Nobody runs an end-to-end proof that *wallet seed generation* still reaches the
board TRNG. Years later, AI-cheap search turns a latent composition bug into
mass theft by outsiders who never needed Coinkite’s servers.

This scenario fits the official technical backgrounder almost line for line: the
cryptographic *choice* was sound, the *integration* was not; review confirmed
TRNG code was present in the binary without verifying which `rng_get` seed
generation actually resolved to. It also fits afilini’s correction that the
fallback PRNG is normal upstream behavior for boards without hardware RNG—the
sin is accidentally landing on it for seed creation, not inventing a custom
backdoor PRNG in Coldcard’s own tree. Dual identity and empty commit subjects
still look bad under S1, but they read as OPSEC and culture failure, not as a
finished heist plan. Theft guilt is low; product and process guilt are high.

### S2 — Negligence plus defensive denial

S2 accepts S1’s mechanism and adds an institutional second act. Sparse early
drains (2022–2024 social claims), a May 2025 engineer concern, or other noisy
signals reach the company and are mis-triaged—malware, user error, “we’d already
know”—or acknowledged only as unrelated patches. When the 2026 mass waves hit,
the public line becomes “we were unaware until today,” which may be narrowly
true about *this root cause* while still false as a story about never having
heard of anomalous Coldcard losses.

This is the cleanest way to hold E17’s unawareness claim and the prior-notice
chorus in one head without forcing intentional plant. It still needs primary
tickets to graduate from plausible to demonstrated. Process and moral exposure
rise; criminal theft remains unproven.

### S3 — Reckless opacity without a heist plan

Here the dual identity is the emotional center. Security-critical libngu work
lands under a sparse GitHub nym while the same human also commits as Peter Gray
on the same repos and authors the firmware migration under a real email. Commit
messages like `x`, missing review, and an official account’s old joke defining
“retirement attack” as a planted entropy bug all feed a narrative of people who
liked dark humor and burner handles more than transparent process. When the AI
era arrives, *external* actors cash an option the culture left on the table.

S3 explains why the nym discourse feels decisive even when it does not prove
who moved the coins. Real-name firmware commits slightly cut against a pure
“burner only” story—they show the same person was not exclusively hiding—but
they do not erase the opacity. Reputation and governance guilt run high; theft
still needs a proceeds bridge.

### S4 — Informed non-fix

Someone inside eventually understands that device-generated seeds on affected
firmware are weaker than advertised. Forcing a loud migration would destroy
trust, sales, and inventory narrative. The company (or a decision-maker) hopes
the issue ages out, stays rare, or stays below attacker cost until something
else intervenes. No one necessarily plans to steal; they plan *not to be the
ones who announce the fire*.

S4 is morally serious and almost impossible to prove from public data. It
directly contradicts the plain reading of “unaware until today.” Treat it as a
hypothesis that only internal mail, tickets, or testimony can promote.

### S5 — Inside-assisted exploit

Staff, former staff, or someone with insider knowledge of the weak seed space
(not necessarily the original 2021 author) runs or helps the 2026 drains. The
public attack pattern—paid blockchain-analytics accounts, automated sweeps, wave
behavior—can still look external if the insider only supplies knowledge or a
seed list.

S5 is the first scenario that is *structurally criminal* if true. Public
evidence does not currently identify Coinkite staff as operators. Block’s lead
that a well-known provider’s paid account was used is compatible with a
sophisticated outsider *or* with anyone who can buy analytics. Without
staff↔collector linkage, S5 remains an open allegation class, not a finding.

### S6 — Long-game intentional plant

This is the maximal social story: the entropy failure was designed so that
“project makers” could later retrieve funds—the retirement-attack joke made
flesh. The nym, the empty commits, the disaster scale, and the multi-year
latency are recruited as choreography.

S6 is the weakest fit to the *full* public technical record. The dangerous PRNG
is upstream MicroPython fallback, not a bespoke Coinkite backdoor generator; the
failure is a cross-module composition that looks more like chaos than craft;
Gray’s own technical prose claims he did not know that PRNG was in the tree and
that he zeroed a macro for the wrong reason. Empty `x` commits look more like
haste than a careful trap. S6 should stay on the board as the hypothesis social
media wants, but it demands intent evidence the current set does not supply.

### S7 — RFC6979 post as a tell

On day 0 of the public crisis, `@DocHex` writes that he wonders whether a smart
AI found a hole in RFC6979. Same-day timing, the AI theme shared with the
official attacker narrative, and the scarcity of other original CTO posts that
week invite a reading that he was circling the real threat model—AI-scale search
against crypto assumptions—while naming the wrong layer.

As a *standalone* proof of inside knowledge of the entropy bug, S7 is weak. The
named standard is about deterministic ECDSA nonces, not seed generation; the
actual Coldcard defect does not require breaking RFC6979; engagement was low;
and a correct entropy hypothesis appeared in-thread within about ninety minutes
from someone else. Useful as color; dangerous as load-bearing structure.

### S8 — RFC6979 post as honest wrong theory

The same artifact, kinder reading: a senior engineer under extreme stress
free-associates to the scariest AI-crypto failure mode in his head—broken
deterministic nonces—and posts it. He is wrong about the class. The community
corrects toward entropy. No further CTO correction appears, but silence after
chaos is ambiguous.

S8 is the non-guilt reading of E23. It neither helps nor hurts identity claims
A/B; it only resists over-reading one tweet into a confession.

---

## Steelman pointers

Full steelman tables and best-form narratives: detail file sections
**“Steelman: guilt”** and **“Steelman: non-guilt”**.

- **Guilt-leaning best public posts:** jamesob nym/report chain; zherbert GPG;
  AtlantisPleb complicity logic; Zenul/BlockUnmasked prior drains; 2021
  retirement-attack definition.
- **Non-guilt best public posts:** official technical backgrounder; afilini
  MicroPython fallback clarification; darosior entropy+LLM ops; zherbert’s own
  “likely negligence” caveat.

---

## Fable collection plan (ordered by flip value)

### 1. On-chain (only path that moves C/D/S5/S6 and can flip G)

See [`2026-08-04-on-chain-keyspace-prior-drain-plan.md`](2026-08-04-on-chain-keyspace-prior-drain-plan.md).

1. Reproduce ~40-bit Mk3 weak keyspace independently; derive addresses; match
   known drain set (Galaxy/Kelbie).
2. If any pre-2026 claimed victim shares an address, test membership in
   weak keyspace → can upgrade **G**.
3. Ingest drain ledger; cluster collectors; watch exchange deposits.

### 2. Cheap forensic closes (done / partial this pass)

| Item | Result (2026-08-04) |
| --- | --- |
| 10 “PGP not good under Gray” commits | **Closed (E24):** 8× GitHub web-flow `4AEE18F83AFDEB23` (merge/init); 2× scgbckbone keys — **not** a third mystery on the critical path |
| studentofthings “credit” claim | **Nuanced (E24):** reporter says a *different* bug was patched without public credit; RNG was AI-ranked High and **not** fully disclosed because of that — official “unrelated” can be true for the *reported* bug while still leaving process failure |
| TZ stylometry-lite | **Supportive (E24):** Switck and Peter D. Gray both **US Eastern** (`-04`/`-05`); scgbckbone EU (`+01`/`+02`) |
| `switck/firmware` fork | **Closed (E24):** ~1013 commits by Gray; last push **2021-01-12**, **before** Mar 2021 libNgU seed migration — fork is Gray’s Coldcard work mirror, not a parallel secret RNG plant post-migration |
| Full switck X (54 posts) | **Blocked for recent-search:** 0 posts in 7-day window; account live (54 total). Needs full-archive / user-timeline product or scrape outside recent search |

### 3. Preservation / monitoring

- Local digests for 17 key URLs: archive index receipt.
- Wayback save attempted (mixed 302/523/timeouts) — retry queue in collection plan.
- Watch lanes: DocHex/switck rebuttal, firmware releases, drain addresses,
  PACER / Ontario courts (Coinkite Toronto).

### 4. Doc structure

- **This file:** claims, flip artifacts, scenarios, summary.
- **Detail file:** append-only E-rows, steelmans, full links.
- **On-chain plan / archive index:** satellite files.

---

## Summary of all we know so far

### Locked down

1. **Peter D. Gray’s personal OpenPGP key** produces **Good signatures** on the
   critical libngu RNG adapter commit and on **dozens** of other
   `switck`-authored commits (77 good of 123 total on `switck/libngu`).
2. The same person commits as **Switck** and as **Peter D. Gray
   `<peter@conalgo.com>`** on that repo.
3. Coldcard firmware commit **“First pass w/ libNgU”** (`b18723dd`) is authored
   by **Peter D. Gray** — the seed-path migration under real name.
4. The defect mechanism (libNgU + `#ifndef` + MicroPython PRNG fallback + seed
   API move) is independently documented in-repo and in Coinkite’s own
   technical backgrounder.
5. Official day-0 advisory **under-scoped** Mk4/Q/Mk5; later text admits ~72-bit
   impact on those models.
6. Official backgrounder uses **first person** (“I set `MICROPY_HW_ENABLE_RNG`
   to zero”, “PRNG I didn’t know was in the tree”) and claims **unawareness
   until disclosure day**.
7. The 10 non-Gray PGP commits are **explained** (GitHub web-flow merges +
   scgbckbone) — they do **not** undermine the Gray↔critical-path map.
8. `switck/firmware` is essentially **Gray’s** historical Coldcard tree ending
   Jan 2021, not a post-bug secret fork.

### Contested / open

1. **Intentional plant / theft (C, S6)** — not established.
2. **Staff participation in drains (D, S5)** — no proceeds bridge.
3. **Prior notice of this exact flaw (G)** — loud third-party claims vs
   “unaware until today”; studentofthings chronology is about a **different
   reported bug** plus non-report of RNG after no credit.
4. **Day-0 `@DocHex` RFC6979 AI speculation** — wrong vulnerability class;
   interesting timing; not proof of guilt (E23).

### Least-strained reading

**S1–S3:** catastrophic integration negligence and OPSEC/culture failure by a
person who controlled the critical code path, amplified by dual identity and
ignored or mis-triaged signals — **without** proven long-game heist.

### Highest-value next work (unchanged from Fable)

1. **Weak keyspace reproduction + prior-victim address membership tests.**
2. Keep **archiving** citations; stand up court/drain/DocHex watches.
3. Obtain **primary** prior-notice artifacts if they surface.

---

## Related

- Evidence detail: [`2026-08-04-cto-thesis-evidence-detail.md`](2026-08-04-cto-thesis-evidence-detail.md)
- On-chain plan: [`2026-08-04-on-chain-keyspace-prior-drain-plan.md`](2026-08-04-on-chain-keyspace-prior-drain-plan.md)
- Archive: [`receipts/2026-08-04-cto-thesis-archive-index.md`](receipts/2026-08-04-cto-thesis-archive-index.md)
- Technical: [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md),
  [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
