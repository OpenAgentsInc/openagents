# Coinkite CTO / "inside job" thesis — consolidated evidence and analysis

Status: **single consolidated document** (claims + full evidence log +
scenarios + collection plans). Not a legal finding and not a public product claim.

Last updated: 2026-08-04 (split reversed — all thesis material recombined here).

Related receipts (machine data, kept beside this file):

- [`receipts/2026-08-04-switck-x-archive.json`](receipts/2026-08-04-switck-x-archive.json) — full `@switck` timeline dump (52 posts)

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
Part II–III of this document.

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
9. **`@switck` X archive (52 posts):** nym born at DEFCON 2019; on **2020-10-16**
   thanks `@DocHex` for a merge and says a new bitcoin library “Could be useful
   on @COLDCARDwallet someday”
   ([1317230987294740480](https://x.com/switck/status/1317230987294740480)) —
   primary for the viral flag. Under A/B this is dual-account performance aimed
   at Coldcard.
10. **Galaxy** published seven concrete Waves 1–2 monitor addresses and evolving
    loss totals (E27 receipt).

### Contested / open

1. **Intentional plant / theft (C, S6)** — not established.
2. **Staff participation in drains (D, S5)** — no proceeds bridge.
3. **Prior notice of this exact flaw (G)** — loud third-party claims vs
   “unaware until today”; studentofthings chronology is about a **different
   reported bug** plus non-report of RNG after no credit (E24d/E28).
4. **Day-0 `@DocHex` RFC6979 AI speculation** — wrong vulnerability class;
   interesting timing; not proof of guilt (E23).

### Least-strained reading

**S1–S3:** catastrophic integration negligence and OPSEC/culture failure by a
person who controlled the critical code path, amplified by dual identity
(including the 2020 “someday” Coldcard tweet) and ignored or mis-triaged
signals — **without** proven long-game heist.

### Highest-value next work

1. **Weak keyspace reproduction + prior-victim address membership tests**
   (public PoC candidate + Galaxy collector watch list).
2. Watch for deletion of the 2020-10-16 switck post; fill any missing 2 posts.
3. Primary tickets / discovery for prior-notice and Block provider name.

---

---

# Part II — Evidence log

### E1 — Morning social amplification (2026-08-04)

Source: X API recent search + post lookup. Owner account `@AtlantisPleb`
heavily RTed / quoted the CTO-identity cluster.

| Kind | UTC | Link | Note |
| --- | --- | --- | --- |
| RT | 15:52 | [AtlantisPleb/2084668869881323543](https://x.com/AtlantisPleb/status/2084668869881323543) | RT `@_PyBlock_` follow-graph: Peter / Switck / DocHex |
| Source | 15:20 | [_PyBlock_/2084660826271531386](https://x.com/_PyBlock_/status/2084660826271531386) | Same collage |
| Source | 14:17 | [_PyBlock_/2084644806890782737](https://x.com/_PyBlock_/status/2084644806890782737) | “Switck aka Peter” + image |
| Quote | 15:20 | [AtlantisPleb/2084660832596570417](https://x.com/AtlantisPleb/status/2084660832596570417) | Complicity hypothesis if team knew; points at CTO |
| Quote | 15:28 | [AtlantisPleb/2084662822512873785](https://x.com/AtlantisPleb/status/2084662822512873785) | “Could be useful on Coldcard someday” 🚩 |
| Quote | 13:33 | [AtlantisPleb/2084633753817592252](https://x.com/AtlantisPleb/status/2084633753817592252) | `<speculation>` 2021 retirement-attack tweet |
| Reply | 15:44 | [AtlantisPleb/2084666853419360648](https://x.com/AtlantisPleb/status/2084666853419360648) | “Or preparing a theft” |
| RT | 15:18 | [AtlantisPleb/2084660264499020015](https://x.com/AtlantisPleb/status/2084660264499020015) | RT `@zherbert` identity post |
| Source | 14:29 | [zherbert/2084647957526167853](https://x.com/zherbert/status/2084647957526167853) | Phone-last-digits + cites GPG work |
| Source | 12:39 | [jamesob/2084620229389197453](https://x.com/jamesob/status/2084620229389197453) | switck ≈ Peter Gray |
| Source | 12:56 | [jamesob/2084624605969350915](https://x.com/jamesob/status/2084624605969350915) | May 2025 libngu/RNG report; “we'd already know” |
| Source | 13:59 | [jamesob/2084640418918957430](https://x.com/jamesob/status/2084640418918957430) | Nym “no reasonable explanation”; “something isn't right” |
| Source | 13:32 | [hodlonaut/2084633601845363006](https://x.com/hodlonaut/status/2084633601845363006) | Bug seemingly written by CTO; post-attack messaging critique |
| Source | Aug 3 | [hodlonaut/2084217381165940812](https://x.com/hodlonaut/status/2084217381165940812) | Conditional long-known-bug + job-vacancy theory |
| Official | 11:07 | [COLDCARDwallet/2084596971268956161](https://x.com/COLDCARDwallet/status/2084596971268956161) | Migrate / new seed / threat ongoing |
| Official | 13:29 | [COLDCARDwallet/2084632863756955661](https://x.com/COLDCARDwallet/status/2084632863756955661) | Not permanently bricked; power-cycle; [PR #693](https://github.com/Coldcard/firmware/pull/693) |

**Label:** external observation. Useful as a citation map of what the timeline
was arguing, not as primary technical proof.

### E2 — Account directory (public profiles)

| Account | Platform | Observed fields (2026-08-04) |
| --- | --- | --- |
| `@DocHex` | X | Bio: “Co-Founder Coinkite. Programmer.” Created 2010-10-21. ~3.1k followers. |
| `@switck` | X | Bio: “Cypher all the things. She/he/his/hers.” Created 2019-08-04. ~114 followers. ~54 posts. Location “Cyberspace”. |
| `doc-hex` | GitHub | Company Coinkite Inc.; blog coinkite.com; location Toronto; created 2012-02-28; 41 public repos. |
| `switck` | GitHub | Bio `mx@switck.com`; company `NaN`; location Cyberspace; twitter `switck`; created **2020-08-07**; 5 public repos; 9 followers. |
| `@nvk` | X | High-follower Coinkite-associated account; recent window mostly RTs of official/migration content (no original switck-identity rebuttal found in sampled recent search). |
| `@COLDCARDwallet` | X | Official product account; migration + inventory-destruction messaging. |

`switck` GitHub repos (all public):

| Repo | Created | Notes |
| --- | --- | --- |
| [switck/pandora-shar](https://github.com/switck/pandora-shar) | 2020-08-07 | Same day as account |
| [switck/libngu](https://github.com/switck/libngu) | 2020-10-15 | Non-fork; ★5; critical crypto |
| [switck/firmware](https://github.com/switck/firmware) | 2020-10-15 | Fork of Coldcard firmware |
| [switck/micropython](https://github.com/switck/micropython) | 2020-10-15 | Fork |
| [switck/bech32](https://github.com/switck/bech32) | 2021-02-03 | Fork |

**Inference (weak):** the nym repo graph looks purpose-built around Coldcard’s
stack (firmware + micropython + libngu), not a broad independent FOSS career.

### E3 — OpenPGP: local `git verify-commit` against `doc-hex` public key

**Method (reproducible):**

```sh
# Import Peter Gray personal key published on GitHub user doc-hex
curl -sS https://api.github.com/users/doc-hex/gpg_keys | jq -r '.[0].raw_key' > /tmp/doc-hex.asc
export GNUPGHOME=$(mktemp -d)
gpg --import /tmp/doc-hex.asc

git clone https://github.com/switck/libngu.git
cd libngu
git verify-commit f19de0527a49e560203102288ae4bc9740a32d96
```

**Observed (this machine, 2026-08-04):**

```text
gpg: Signature made Thu Jan 28 13:16:43 2021 CST
gpg:                using RSA key D9766C79E77B0198D66975BDF0E6CC6AFC16CF7B
gpg: Good signature from "Peter D. Gray (Personal) <peter@conalgo.com>" [expired]
gpg:                 aka "Peter Gray <peter@ripeapps.com>" [expired]
gpg: Note: This key has expired!
Primary: A004C9BCE217ABE9341CD81AA2DCD558C2BE5D7C
Subkey:  D9766C79E77B0198D66975BDF0E6CC6AFC16CF7B
```

Key notes:

- GitHub API lists the same signing subkey id **`F0E6CC6AFC16CF7B`** (last 64
  bits of the fingerprint above).
- Key is marked **expired as of 2023-05-28** when verified in 2026; that does
  **not** invalidate a 2021 signature made while the key was valid.
- UIDs on the key: **Peter D. Gray (Personal) `<peter@conalgo.com>`** (verified
  on GitHub) and `peter@ripeapps.com`.

**Census of all 123 commits on `switck/libngu` (local verify loop):**

| Result | Count |
| --- | ---: |
| **Good signature** under Gray’s imported key | **77** |
| PGP block present but not good under this key | 10 |
| No PGP signature | 36 |
| **Total commits** | **123** |

Good signatures by **author string** on the commit:

| Author field | Good sigs |
| --- | ---: |
| `Switck <69336248+switck@users.noreply.github.com>` | **58** |
| `Peter D. Gray <peter@conalgo.com>` | **19** |

**Implication for A:** this is stronger than “key id string match on one
commit.” Dozens of nym-authored commits, including the critical RNG adapter
commit, produce **Good signature** as Peter D. Gray’s personal key. Nineteen
more commits are authored **in Gray’s real name on the same nym-owned repo**
and also good-sig under the same key.

### E4 — Critical commits (technical + authorship)

#### E4a — libngu RNG adapter (defect component)

- SHA: [`f19de0527a49e560203102288ae4bc9740a32d96`](https://github.com/switck/libngu/commit/f19de0527a49e560203102288ae4bc9740a32d96)
- Date: 2021-01-28
- Author/committer fields: **Switck** / `69336248+switck@users.noreply.github.com`
- Message: `x`
- GPG: **Good signature**, Peter D. Gray personal key (E3)
- Role: existence-only `#ifndef MICROPY_HW_ENABLE_RNG` guard + `rng_get()` path
  (see [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md),
  [`2026-08-01-bitcoin-plus-plus-oped-analysis.md`](2026-08-01-bitcoin-plus-plus-oped-analysis.md))

#### E4b — Coldcard firmware imports libNgU / moves seed generation

- SHA: [`b18723dddb6d751c39978e4364b56b2414f68b47`](https://github.com/Coldcard/firmware/commit/b18723dddb6d751c39978e4364b56b2414f68b47)
- Message: **`First pass w/ libNgU`**
- Author **and** committer (local `git log` on fetch):  
  **`Peter D. Gray <peter@conalgo.com>`**
- Role: large migration that points wallet seed creation at libNgU’s
  `random.bytes()` path (see existing technical docs)

**Implication for B:** the nym is not required to “hide” the firmware
integration. The seed-path migration is under Gray’s **real name and
conalgo.com email**. Combined with E3, Gray is on **both sides** of the
composition that produced weak seeds: libngu adapter (signed as nym) and
firmware import (real name).

#### E4c — Later libngu reseed under real name

- SHA: [`61ffc74af171197ef5d5d79b78b59ab750ef35c7`](https://github.com/switck/libngu/commit/61ffc74af171197ef5d5d79b78b59ab750ef35c7)
- Date: 2022-03-11
- Author: **Peter D. Gray `<peter@conalgo.com>`**
- Message: `Add reseed function to ngu.random module`
- Process note (Kelbie archive): PR #18, zero reviewers — see
  [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)

### E5 — Author census on `switck/libngu` (API commit list, 123 commits)

| Author email | Commits |
| --- | ---: |
| `69336248+switck@users.noreply.github.com` | 66 |
| `scgbckbone@proton.me` | 36 |
| **`peter@conalgo.com`** | **19** |
| `avirgovi@cisco.com` | 2 |

Committer field: 92× switck noreply, **19× peter@conalgo.com**, 9× GitHub
noreply, etc.

**Implication:** the nym-owned repository is not “a stranger’s project that
Coinkite later vendored with no staff fingerprints.” Gray’s real email is a
first-class author on that repo for a non-trivial commit share, and his key
signs most of the early nym history.

### E6 — `@DocHex` recent X behavior (sampled window)

Recent-search `from:DocHex` (≤7 days, 2026-08-04 probe): **almost entirely
reposts** of official COLDCARD / third-party migration and clarification posts.
No original long-form rebuttal of the switck/GPG identity thread appeared in
that sample.

Official product account (`@COLDCARDwallet`) explicitly told critics that
personal opinions of named staff have “little value” and that the team is
presenting a “single united front”
([2084632203019833820](https://x.com/COLDCARDwallet/status/2084632203019833820),
mentions `@nvk` `@DocHex`).

**Label:** behavioral observation only. Silence is not guilt; it is still
evidence of **comms strategy**.

### E7 — Process / review failure (prior docs, still relevant)

Already in-repo; re-listed so the thesis file is self-contained as an index:

- Empty commit subjects (`x`, `runs`) on security-critical diffs.
- No PR / zero reviewers on key path commits (Kelbie `commits.json` table).
- 2018 board macro `MICROPY_HW_ENABLE_RNG (0)` **predates** 2021 — undercuts
  “he zeroed the macro in 2021 only to silence the compiler” as a complete
  history ([bitcoin++ op-ed review](2026-08-01-bitcoin-plus-plus-oped-analysis.md)).

### E8 — What we still do **not** have (open collection)

| Gap | Why it matters |
| --- | --- |
| On-chain link from Gray/Coinkite to drain/collector keys | Required for D (theft participation) |
| Independent verification of jamesob’s May 2025 private report text / Coinkite reply | Supports process failure / prior notice; not yet a public artifact here |
| Full offline audit of the 10 “signed but not good under this key” commits | Who else signed? Key rotation? |
| switck X account archive (54 posts) full pull | Nym persona consistency / links |
| Device serial / factory programming chain of custody | Inventory destruction claims |
| Legal process, LE, or exchange freezes tied to named parties | Outside this repo’s authority |
| Primary manufacturer ticket / email for 2022–2024 victim reports | Needed to harden G |

### E9 — Recent discourse census (X recent search, 2026-08-04)

Fifteen parallel recent-search queries (identity, intent, RNG, remediation, loss
figures, named investigators) returned **~590 unique posts** in the API window.

Rough keyword co-occurrence counts (non-exclusive; post text only):

| Theme keywords | Posts hitting theme |
| --- | ---: |
| libngu / yasmarang / entropy / RNG / TRNG / random.bytes | 168 |
| GPG / switck / DocHex / nym / Peter Gray | 107 |
| BTC / $ / million / drained / stolen | 100 |
| May 2025 / report / warned / disclosure | 48 |
| inside job / on purpose / complicit / honeypot / exit scam | 46 |
| migrate / new seed / inventory / brick | 26 |

**High-engagement non-identity posts (context, not authorship proof):**

| Author | Link | Signal |
| --- | --- | --- |
| `@jackmallers` | [2083224256976953706](https://x.com/jackmallers/status/2083224256976953706) | Widespread “call everyone” alert (~5.5k likes) |
| `@TheBTCTherapist` | [2083916626051715528](https://x.com/TheBTCTherapist/status/2083916626051715528) | Viral “4 years ago drained + blocked” claim (h/t `@Zenul_Abidin`) |
| `@clay_garrett` (Block) | [2083247006139503065](https://x.com/clay_garrett/status/2083247006139503065) | Sweep operator used paid blockchain-services account during drains |
| `@glxyresearch` | [2083623500183421043](https://x.com/glxyresearch/status/2083623500183421043) | Loss tracking waves (~1,367 BTC / 4,585 addresses at that update) |
| `@BTCsessions` | [2084024733511921691](https://x.com/BTCsessions/status/2084024733511921691) | Confirmed Mk3 + 2-word passphrase loss (passphrase not absolute) |
| `@wowens` | [2084041966212591963](https://x.com/wowens/status/2084041966212591963) | Honey-trap UTXO; RBF beat attacker fee |
| `@Rob1Ham` | [2084140242915782743](https://x.com/Rob1Ham/status/2084140242915782743) | Ecosystem red-team wave after Coldcard |
| `@OpenSats` | [2084017521376866402](https://x.com/OpenSats/status/2084017521376866402) | nvk steps down from OpenSats board (governance adjacency) |
| `@MedusaOnchain` | [2084492687533838626](https://x.com/MedusaOnchain/status/2084492687533838626) | Viral “inside job” + 2021 “confession” framing |
| `@parachutesBTC` | [2083702964565475414](https://x.com/parachutesBTC/status/2083702964565475414) | Long-dormant-knowledge-then-strike theory (explicitly theory) |

### E10 — Official 2021 “retirement attack” definition (primary text)

Often reshared as a “confession.” Primary post:

- [`@COLDCARDwallet` 2021-10-10](https://x.com/COLDCARDwallet/status/1447213375398846473)
  (reply defining the term):  
  *“It’s when the project makers could have a ‘bug’ in the entropy generation
  for later retrieval.”*

**Evidence status:** the **definition is authentic official-account text**.  
**Inference status:** treating this as proof that Coinkite *implemented* such a
bug in 2021 remains **speculation** unless tied to contemporaneous intent
evidence. It does show the vendor publicly discussed the *threat model* of
vendor-planted entropy bugs years before the 2026 drains (fuel for F / social
narrative).

AtlantisPleb’s `<speculation>` quote about CTO floating the idea to nvk
([2084633753817592252](https://x.com/AtlantisPleb/status/2084633753817592252))
is still labeled speculation by its author.

### E11 — Official advisory timeline vs model blast radius (H)

Day-0 public product post (2026-07-30):

- [`@COLDCARDwallet/2082961993070247948`](https://x.com/COLDCARDwallet/status/2082961993070247948)
- Linked advisory:
  [blog.coinkite.com/coldcard-mk3-seed-generation-warning](https://blog.coinkite.com/coldcard-mk3-seed-generation-warning/)

**Early claim (tweet):** Mk3 after 4.0.1 at risk; **“Mk4, Q and Mk5 are not
affected based on our early analysis.”**

**Updated advisory body (retrieved 2026-08-04; page says updated Aug 1, 2026):**

- Mk2/Mk3 4.0.1–4.1.9 at risk without ≥50 private dice rolls / strong passphrase
  exceptions as written.
- **Mk4 / Mk5 / Q also affected** for seeds generated before fixed firmware;
  advisory states **~72 bits of entropy rather than 128** on those models
  (serious, “not as severe” as Mk3).
- Fixed version matrix published (4.2.0 / 5.6.0 / 1.5.0Q / Edge 6.6.0X /
  6.6.0QX).
- Firmware update does **not** repair old seeds; migrate.
- TAPSIGNER / OPENDIME / SATSCARD called out as different codebases.

**Shipment / inventory (E-adjacent):**

- [`2083977229084578060`](https://x.com/COLDCARDwallet/status/2083977229084578060)
  — halted shipments; destroyed remaining affected units on hand; emailed
  already-shipped customers.

**Label:** H is **documented process failure in early public scoping**, not
proof of theft. It *is* material to victim guidance quality and trust in
day-0 vendor statements.

### E12 — Prior-notice / “they knew” leads (G) — third-party claims

These are **collection leads**, not independently re-verified tickets:

| Source | Link | Claim |
| --- | --- | --- |
| `@BlockUnmasked` | [2083210091671568874](https://x.com/BlockUnmasked/status/2083210091671568874) + [article](https://www.blockchainunmasked.com/post/coldcard-seed-flaw-38m) | 2024 victims missing BTC from Coldcards; weak entropy; reports to manufacturer and agencies |
| `@TheBTCTherapist` / `@Zenul_Abidin` | [2083916626051715528](https://x.com/TheBTCTherapist/status/2083916626051715528) | ~4 years ago user drained after RNG-without-dice; “blocked by Coldcard” |
| `@JWWeatherman_` | [2083871791013888477](https://x.com/JWWeatherman_/status/2083871791013888477) | Points at **2022** historical material (media attachment) |
| `@jamesob` | [2084624605969350915](https://x.com/jamesob/status/2084624605969350915) | May 2025 private concern about libngu/RNG; response “we'd already know” |
| `@americanhodl8` | [2084388719788982730](https://x.com/americanhodl8/status/2084388719788982730) | Quotes BlockUnmasked as “completely damning **if true**” |

`@Zenul_Abidin` also states in-thread that issues were “discovered, but Coldcard
wasn't interested in acting”
([2084586773703782442](https://x.com/Zenul_Abidin/status/2084586773703782442)).

**Still needed for G:** primary emails/tickets, Coinkite ticket IDs, or LE
filing numbers. Do not upgrade G to “proven” on social claims alone.

### E13 — Attacker operational pattern (context for D)

- `@clay_garrett` / Block engineering: sweep operator used a **paid account at a
  well-known blockchain-services provider** to query source addresses during
  sweeps ([2083247006139503065](https://x.com/clay_garrett/status/2083247006139503065)).
- Multiple public posts describe **organized waves**, mempool RBF races, bait
  UTXOs, and evolving loss totals (`@glxyresearch`, `@wowens`, etc.).

**Label:** supports sophisticated third-party exploitation. **Does not** identify
Coinkite staff as the operator. Relevant to “who could have known / waited”
theories but not identity proof.

### E14 — Comms / governance adjacency

- `@Pledditor` day-5 critique: little personal communication from `@nvk` /
  `@DocHex`
  ([2084624867350004111](https://x.com/Pledditor/status/2084624867350004111)).
- Official reply: personal opinions “little value”; “single united front”
  ([2084632203019833820](https://x.com/COLDCARDwallet/status/2084632203019833820)).
- `@OpenSats`: nvk steps down from board
  ([2084017521376866402](https://x.com/OpenSats/status/2084017521376866402)) —
  **timing correlation only**; no causal claim recorded here.
- `@DocHex` recent-search sample remains almost all RTs; `from:switck` and
  `to:DocHex` identity-challenge queries returned **no** nym self-defense
  posts in window.

### E15 — `switck/pandora-shar` also Gray-signed

Repo: [switck/pandora-shar](https://github.com/switck/pandora-shar) — “Encrypted
self-extracting Unix shell scripts”; dedication to people who `curl | bash`.

- Single commit `cfbd7d7` (2020-08-07), author Switck noreply.
- Local `git verify-commit`: **Good signature** from **Peter D. Gray** personal
  key (same fingerprint as E3).

**Label:** reinforces that the switck GitHub identity’s early crypto/tooling
work is cryptographically bound to Gray’s key **beyond libngu alone**. Not a
theft proof; expands the nym↔key map.

### E16 — Passphrase exception is not absolute (victim guidance)

`@BTCsessions` reported a **confirmed Mk3 + 2-word passphrase** drain
([2084024733511921691](https://x.com/BTCsessions/status/2084024733511921691)).
Official advisory still treats a **strong unique** passphrase as reducing
immediate exposure but recommends migration. Short passphrases are in the
attack surface.

### E17 — Official technical backgrounder (primary, 2026-07-30 / updated Aug 1)

URL: [blog.coinkite.com/entropy-technical-backgrounder](https://blog.coinkite.com/entropy-technical-backgrounder/)

Retrieved full HTML text 2026-08-04. Material quotes / claims:

| Claim in post | Relevance |
| --- | --- |
| “**We were unaware of the bug until today.**” | Official unawareness claim — **tension with G** prior-notice social claims |
| “A few weeks ago, we used one of the best available **AI models** to review our code… **it did not find this bug**” | Process failure under AI review; also frames attacker discovery as AI-assisted |
| “The COLDCARD source… open… we have to **assume** that someone used AI to review previous versions… and stumbled upon this issue” | Vendor attack-story hypothesis, not independent proof of method |
| Migration 2021: `ckcc.rng_bytes()` → `ngu.random.bytes()`; `rng_get()` resolved to **MicroPython software fallback** | Matches in-repo technical analyses |
| First person: “a PRNG that **I didn’t know** was actually in the source code base (… Micropython)” | **I** voice — internal engineer owning narrative (consistent with CTO-as-author map, not proof alone) |
| First person: “the carefully crafted TRNG code **I wrote** was being used, but just by chance, and only for less important things” | Same |
| First person: “**I explicitly set** `MICROPY_HW_ENABLE_RNG` to zero, thinking we didn’t need either version, but that’s not what it does” | Direct ownership of the zeroed macro decision (contrast with pure “accidental 2018 carry-forward only” popular story — macro value choice is admitted) |
| Mk2/Mk3 search space estimate **~40 bits**; Mk4/Q/Mk5 **~72 bits** under stated assumptions | Quantifies blast radius |
| Existing review confirmed TRNG **present in binary** but did not verify **which `rng_get()` seed generation actually reached** | Explains how review missed it; aligns with whole-program reachability lesson in Loupe docs |

**Label:** highest-value **vendor primary** for technical mechanism + unawareness claim + first-person engineering ownership. Does **not** name `switck` or address GPG/nym identity.

**Tension to track:** E17 “unaware until today” vs E12 / E18 prior-notice claims. Both can be true only if “unaware” means “did not accept this root cause,” not “never heard of anomalous drains.”

### E18 — More prior-notice / ignored-report claims (still third-party)

| Source | Link | Claim |
| --- | --- | --- |
| `@Zenul_Abidin` | [2083756420843839872](https://x.com/Zenul_Abidin/status/2083756420843839872) (~2.9k likes) | “SCANDAL: Coldcard knew… for years”; **2022** brand-new user drained; victim “blocked for reporting”; media attachments |
| `@studentofthings` | [2084007449267188163](https://x.com/studentofthings/status/2084007449267188163) (~3.6k likes in search ranking) | “Just confirmed, I found the Coldcard RNG bug **11 months ago** and never reported it because they didn’t acknowledge me on the **first one** I reported” (+ image) |
| `@COLDCARDwallet` on studentofthings | [2084280694864224641](https://x.com/COLDCARDwallet/status/2084280694864224641) | Official: “your original report is **not related** to the current issues. A patch was made, and credit was issued.” |
| `@jamesob` | [2084603251706503369](https://x.com/jamesob/status/2084603251706503369) | May 2025: “found the potential/likelihood… Reported it, was **shrugged off**… trusted Coinkite when they said it wasn’t misconfigured” |
| `@PanHodl` | [2084117965717323961](https://x.com/PanHodl/status/2084117965717323961) | Claims Telegram pointed at Coldcard random function **in 2021** (screenshot) |

**Collection status:** volume and engagement of “prior knowledge” narrative is high. **Primary manufacturer tickets still absent.** Official line on at least one reporter is “different issue, already patched.” That could be true, partial, or incorrect — needs document comparison.

### E19 — Block / Bitkey investigation: paid analytics provider (expanded)

Thread from `@clay_garrett` (Block / Bitkey eng leadership):

1. [2083247006139503065](https://x.com/clay_garrett/status/2083247006139503065) — sweep operator used **paid account** at well-known blockchain-services provider to query source addresses during sweeps.
2. [2083247007808774228](https://x.com/clay_garrett/status/2083247007808774228) — Block contacted provider; **internal logs matched** number/timing/sequence of requests “with extraordinary specificity”; provider supplied standard services; **no evidence provider knowingly facilitated** theft.

Related: `@max_guise` / Block early investigation thread
[2083007180874379515](https://x.com/max_guise/status/2083007180874379515) (quoted in clay’s posts).

**Label:** strong **third-party operational** evidence about attacker tooling. Provider name not public in the text we captured. **Does not** identify Coinkite staff as the account holder.

### E20 — Loss totals (moving public estimates)

Public figures shifted during the week (not reconciled here):

- Early: ~594 BTC / ~$38M / ~500 wallets (many posts; BlockUnmasked article).
- Galaxy updates: e.g. ~1,367 BTC / ~4,585 addresses
  ([2083623500183421043](https://x.com/glxyresearch/status/2083623500183421043)).
- Later viral: **>$100M**
  ([2084413716494266594](https://x.com/WatcherGuru/status/2084413716494266594) style JUST IN posts);
  ~2,055 BTC / ~$130M / 7,700+ addresses
  ([2084465607932854717](https://x.com/lookonchain/status/2084465607932854717) citing Galaxy).

**Label:** treat as **external estimates**. Prefer Galaxy / Kelbie-style ledgers for forensic work; do not freeze a single number in product claims.

### E21 — Hotfix side effect: TRNG latch / “brick” scare

- Community reported rushed hotfix risk of permanent RNG fault before PIN
  ([2084245649944387633](https://x.com/i/web/status/2084245649944387633) citing
  [PR #692](https://github.com/Coldcard/firmware/pull/692)).
- Official later: not permanent; power-cycle; fail closed; [PR #693](https://github.com/Coldcard/firmware/pull/693)
  (E1 / E11).

**Label:** remediation quality issue; separate from 2021 seed defect root cause.

### E22 — Vendor first-person vs dual-identity evidence (inference map)

Combining E3–E5 (GPG + dual authorship) with E17 first-person blog:

| Fact class | Support |
| --- | --- |
| Gray’s key signed switck libngu work | Cryptographic (E3) |
| Gray authored firmware libNgU seed migration under real email | Git (E4b) |
| Vendor blog’s “I” admits zeroing `MICROPY_HW_ENABLE_RNG` and not knowing PRNG was in tree | Official prose (E17) |
| Blog addresses switck nym / dual identity | **Not present** in retrieved text |
| Blog claims unawareness until disclosure day | Official prose (E17) — contested by E12/E18 |

**Inference (not proven):** the first-person engineering voice is **consistent with**
CTO-as-integrator; identity of the blog “I” is not cryptographically signed in
the HTML. Treat as **probable staff voice**, likely Gray given ownership of TRNG
code and migration, unless Coinkite attributes authorship otherwise.

### E23 — CTO `@DocHex` post (~4 days before this note): AI / RFC6979 speculation

**Primary post**

| Field | Value |
| --- | --- |
| Author | `@DocHex` (bio: Co-Founder Coinkite. Programmer.) |
| Time | **2026-07-30T20:24:37.000Z** |
| ID | [2082925350732870062](https://x.com/DocHex/status/2082925350732870062) |
| Text | “I wonder if a smart AI has found a hole in **RFC6979**? That would be bad.” |
| Link in post | [RFC 6979](https://datatracker.ietf.org/doc/html/rfc6979) (deterministic ECDSA nonces) |
| Metrics (probe) | ~13 likes, ~8 replies, ~3 RTs |

**Timeline context (same calendar day as disclosure window)**

| UTC (approx) | Event |
| --- | --- |
| ~2026-07-30 01:36 | Mass drain wave often cited as incident start (prior docs / media) |
| 2026-07-30 (blog) | Coinkite security advisory + technical backgrounder published |
| 2026-07-30 22:50 | `@COLDCARDwallet` Mk3 advisory tweet (early “Mk4/Q/Mk5 not affected”) |
| **2026-07-30 20:24** | **This `@DocHex` RFC6979 / AI speculation** |
| 2026-07-30 21:06 | `@MartyBent` RTs the DocHex post |
| 2026-07-30 21:56 | `@darosior` reply (below) |
| 2026-07-31+ | Expanding model scope, hotfixes, identity discourse |

So the post is **not** “weeks before anything happened.” It lands **during day-0 of the public crisis**, from the CTO’s personal account, while product accounts are issuing advisories.

**What RFC6979 is (and is not)**

- RFC6979 specifies **deterministic ECDSA/DSA nonces** from private key + message (hedges classic nonce reuse).
- A real break in RFC6979 would be a **global ECDSA catastrophe**, not a Coldcard-only seed issue.
- Later community clarification (e.g. `@MrHodl` citing `@Rob1Ham`,
  [2084470259357024379](https://x.com/MrHodl/status/2084470259357024379)): the known Coldcard
  defect is **seed entropy**, and **does not** by itself break normal ECDSA spending that uses
  RFC6979 nonces. DocHex’s hypothesis and the actual bug class **do not match**.

**Immediate technical reply in-thread**

`@darosior` ([2082948440720359924](https://x.com/darosior/status/2082948440720359924), ~27 likes):

> “Smells more like an **entropy fuckup**, and a dumb LLM orchestration of the exploit
> (stop scanning at a specific index, etc)”

That reply is directionally aligned with what became the public root-cause story
(weak seed entropy + automated search), while DocHex pointed at **nonce standards**.

**Later replies on the same conversation** (days after; tone is accountability / rage,
not technical refinement): e.g. calls for prison, “didn’t you ruin everyone’s life,”
“feature or a bug?” linking hodlonaut CTO-authorship posts, “care to comment?” with
screenshots ([conversation_id search](https://x.com/DocHex/status/2082925350732870062)).
No `@DocHex` follow-up correcting RFC6979 → entropy found in the recent-search window.

**How to use this as evidence (careful)**

| Reading | Support |
| --- | --- |
| CTO was **actively thinking about AI finding crypto bugs** on incident day | Direct text |
| CTO’s **first public personal hypothesis was wrong class** (RFC6979 vs seed RNG) | Text + darosior + later RFC6979 clarifications |
| Post is **proof of inside job / advance knowledge of entropy bug** | **Not established** — could be general AI-crypto anxiety or bad early theory |
| Post is **misdirection** away from seed RNG | Speculative; no evidence of deliberate diversion beyond wrong guess |
| Silence after correct replies | Behavioral observation only |

**Adjacent DocHex behavior in the same ~7-day window:** almost all other posts are
**RTs** of official COLDCARD / migration / clarification content (E6, E14). This
RFC6979 tweet is one of the only **original** security-adjacent posts from `@DocHex`
in the sample (the other non-RT is a hardware reply about soldered memory, Jul 29).

---

## E24 — Fable pass forensic closes (2026-08-04)

### E24a — The 10 "PGP present but not Good under Gray key" commits

Full `switck/libngu` census remains: **123** commits; **77** Good signature as
Peter D. Gray; **36** unsigned; **10** signed but not verifiable as Gray.

| Count | Signing key (RSA) | Interpretation |
| ---: | --- | --- |
| 8 | `4AEE18F83AFDEB23` | **GitHub web-flow** commit signing key (merge commits + initial commit via UI). Not a third human identity on the critical path. |
| 1 | `B5690EEEBB952194` | **scgbckbone** (2026-03-12 PR merge validate schnorr) |
| 1 | `35B41FB07EFCCF36A1EA89D3A940F7604E73741B` | **scgbckbone** (2022-09-20 compact seed) |

Critical defect commit `f19de052` remains **Good signature / Peter D. Gray**.

**Status change:** E8 "audit the 10" row **closed**. No mystery third-party
signer on the 2021 RNG adapter.

### E24b — Timezone support for dual identity

Author-date TZ offsets on `switck/libngu`:

| Author email | Offsets observed |
| --- | --- |
| `Switck <...noreply...>` | `-04:00`, `-05:00` (US Eastern) |
| `Peter D. Gray <peter@conalgo.com>` | `-04:00`, `-05:00` (US Eastern) |
| `scgbckbone@proton.me` | `+01:00`, `+02:00` (EU) |

**Status:** independent **metadata consistency** with Toronto/Eastern Gray;
not cryptographic proof alone, but aligns with GPG map.

### E24c — `switck/firmware` fork audit

- Remote: https://github.com/switck/firmware
- Last activity observed: **2021-01-12** (merge PR #58)
- Author mass: **~1013 commits** `Peter D. Gray <peter@conalgo.com>` — Gray-dominated Coldcard firmware history mirror
- Chronology: ends **before** the Mar 2021 "First pass w/ libNgU" migration on upstream Coldcard

**Status:** fork does **not** show a post-migration parallel RNG plant under
the nym. It **strengthens** "switck GitHub surface is Gray's workspace."

### E24d — studentofthings / official "credit issued"

Reporter chronology (recent posts):

- Disclosed a **different critical crypto bug**; Coinkite patched quickly.
- **No public credit / CVE** reduced willingness to continue free research.
- AI ranked the **RNG** issue High not Critical; they did **not** complete /
  submit full RNG analysis after the credit failure.
- Asks official account to "Show me the credit"
  (https://x.com/studentofthings/status/2084295719809142893).
- Official: original report "not related to the current issues... patch... credit
  issued" (https://x.com/COLDCARDwallet/status/2084280694864224641).

**Status:** does **not** prove Coinkite knew the **2026 drain root cause** in
2025. It **does** support a **broken research/credit culture** claim.

### E24e — switck X archive

- Account `@switck` still live: created 2019-08-04; **54** lifetime posts.
- `from:switck` recent search (7 days): **0 posts** — full 54-post archive
  still needs non-recent timeline access.

### E24f — Archive pass

Local HTTP 200 snapshots + sha256 digests for key URLs recorded in
Part V of this document.



---

## E25 — Full `@switck` X timeline archive (API, 2026-08-04)

**Receipt:** [`receipts/2026-08-04-switck-x-archive.json`](receipts/2026-08-04-switck-x-archive.json)

- Endpoint: `GET /2/users/1157857508775608320/tweets` (max 100)
- Returned **52** posts (account metrics said **54** lifetime — 2 missing or
  deleted / pagination edge)
- Account created **2019-08-04**; last post in archive **2022-03-20**
- Bio: "Cypher all the things. She/he/his/hers." Location "Cyberspace"

### Material posts (identity / Coldcard adjacency)

| Time (UTC) | ID | Text (summary) |
| --- | --- | --- |
| 2019-08-04 | [1157858788185202688](https://x.com/switck/status/1157858788185202688) | "#defcon seems like a good time to **start a new identity**. Follow me!" |
| 2020-10-16 | [1317230987294740480](https://x.com/switck/status/1317230987294740480) | "**Thanks for merge @DocHex** ... I'm making yet another bitcoin library. **Could be useful on @COLDCARDwallet someday.**" |
| 2020-10-21 | [1318909664281636867](https://x.com/switck/status/1318909664281636867) | "i made a useful thing" (link) |
| 2021-01-26 | [1354124727699255301](https://x.com/switck/status/1354124727699255301) | "writing **crypto code** that passes test vectors on first try is a superpower" |
| 2021-04-30 | [1388281293536854016](https://x.com/switck/status/1388281293536854016) | RT official **COLDCARD Firmware v4.1.0** (in-window of vulnerable generation era) |

The 2020-10-16 post is the primary source behind the viral "Could be useful on
Coldcard someday" flag (AtlantisPleb quote chain). Under claim **A/B** (switck
signing key = Gray), this is **self-referential roleplay or dual-account
performance** thanking `@DocHex` for a merge while building lib-adjacent work
explicitly aimed at Coldcard. Under a pure third-party-nym theory, it is a
contractor thanking the CTO—still a tight Coinkite adjacency.

**DEFCON 2019 birth of the nym** supports intentional alternate identity, not
by itself theft intent.

### E26 — Public Mk3 RNG PoC repository (keyspace arm)

GitHub search hit (2026-08-04):  
[HenryqueBrito/coldcard-mk3-rng-poc](https://github.com/HenryqueBrito/coldcard-mk3-rng-poc)
— described as educational PoC of Coldcard Mk3 RNG failure (Yasmarang).

**Status:** candidate input for on-chain Arm 1 (review license, reproduce,
compare addresses). Not yet executed/verified in this repo.

### E27 — Galaxy public collector addresses + loss framing

Receipt: Part V (Galaxy addresses)

Seven bech32 addresses published as Waves 1–2 monitoring set with ~1,158 BTC
at post time; later threads raise high-confidence totals toward **~1,596 BTC**
and unconfirmed paths toward ~2k BTC. Galaxy states data is from chain patterns
plus victim reports and that they have **not** brute-forced entropy to confirm
every victim address.

**Use:** watch list + ledger seed. **Not** staff attribution.

### E28 — studentofthings clarification (process culture)

Additional posts reinforce E24d:

- [2084312571457249599](https://x.com/studentofthings/status/2084312571457249599):
  RNG "discovered but not analyzed to see how bad"; "wasn't negligence or
  deceit, just effort that got no attention."
- [2084037746075611343](https://x.com/studentofthings/status/2084037746075611343):
  different critical bug patched without public credit; stopped submitting
  remaining bugs including unfinished RNG analysis.

**Status:** weakens "they were told this exact drain bug and ignored it" for
*this* researcher; strengthens "broken acknowledgment culture delayed more
eyes on the code."

---

# Part III — Steelman readings

## Steelman: readings that pull toward **guilt / bad faith**

These are **strongest fair constructions** of the “something darker than oops”
side. They are **not verdicts**. Steelman means: give the case its best form,
then still separate fact from leap.

### Steelman posts / voices (guilt-leaning)

| Voice | Link | Steelman point |
| --- | --- | --- |
| `@jamesob` | [2084640418918957430](https://x.com/jamesob/status/2084640418918957430) | Public nym for security-critical code “has no reasonable explanation”; May 2025 concern shrugged off |
| `@jamesob` | [2084620229389197453](https://x.com/jamesob/status/2084620229389197453) | switck ≈ DocHex; same person dismissed prior report |
| `@zherbert` | [2084647957526167853](https://x.com/zherbert/status/2084647957526167853) | Multi-channel identity verification (GPG + phone digits claim) |
| `@AtlantisPleb` | [2084660832596570417](https://x.com/AtlantisPleb/status/2084660832596570417) | Migration advice is “logical if complicit”; who else knew? |
| `@hodlonaut` | [2084633601845363006](https://x.com/hodlonaut/status/2084633601845363006) | Authorship + post-attack messaging inconsistency |
| `@Zenul_Abidin` | [2083756420843839872](https://x.com/Zenul_Abidin/status/2083756420843839872) | Years of sparse drains + blocked reporter → long knowledge |
| `@BlockUnmasked` | [2083210091671568874](https://x.com/BlockUnmasked/status/2083210091671568874) | 2024 entropy theory reported to manufacturer + agencies |
| Official 2021 def. | [1447213375398846473](https://x.com/COLDCARDwallet/status/1447213375398846473) | Vendor defined “retirement attack” as planted entropy bug |
| Grok / public | [2084661432960880793](https://x.com/CyberTruckRonin/status/2084661432960880793) style | Facts of identity + bug are real; framing must stay precise |

### Best-form guilt narrative (speculative assembly)

1. **Control of the defect surface:** Gray’s key signs switck libngu work; Gray
   authors firmware “First pass w/ libNgU” under real email (E3–E5). Not a
   random junior PR.
2. **Opacity:** security-critical work under a sparse nym while also committing
   as real name on the same repo looks like intentional identity separation
   (jamesob’s “no reasonable explanation”).
3. **Possible prior signals ignored:** May 2025 report; 2022–2024 victim claims;
   official “unaware until today” (E17) then looks self-serving if tickets exist.
4. **Cultural tell:** official account joking about retirement attacks (E10).
5. **Incident-day cognition:** RFC6979 AI post (E23) shows CTO thinking about
   AI-discovered crypto breaks the same day — steelman guilt reading: he knew
   AI-scale search was the threat model and floated a **broader** scary theory
   while the **local** bug was seed RNG.
6. **Comms pattern:** personal silence on nym/GPG; “united front”; early wrong
   model scope (H).

**Hardest leap still required for theft guilt:** joining the above to **control
of drain keys / proceeds**. Without that, the steelman peaks at **reckless
opacity + ignored warnings + catastrophic negligence**, not proven heist.

---

## Steelman: readings that pull toward **non-guilt / tragic negligence**

### Steelman posts / voices (non-guilt or “competence failure”)

| Voice | Link | Steelman point |
| --- | --- | --- |
| Official tech blog | [entropy-technical-backgrounder](https://blog.coinkite.com/entropy-technical-backgrounder/) | Complex integration bug; “I didn’t know” PRNG in tree; AI review missed it; open source assumed attacker AI path |
| `@afilini` | [2083477163957694524](https://x.com/afilini/status/2083477163957694524) | MicroPython PRNG is normal upstream fallback; bug is **accidentally using it**, not inventing evil PRNG in Coldcard |
| `@afilini` | [2083483878887399594](https://x.com/afilini/status/2083483878887399594) | Auditing fallback would be ignored if you **wrongly assumed** you weren’t on that path |
| `@darosior` (on E23) | [2082948440720359924](https://x.com/darosior/status/2082948440720359924) | Entropy + LLM orchestration — external attacker pattern, not RFC6979 |
| `@zherbert` (even while identity-hunting) | [2084638910970220754](https://x.com/zherbert/status/2084638910970220754) | “Still think it is likely just **incompetence / negligence**” unless identity raises flags |
| `@CyberTruckRonin` | [2084661432960880793](https://x.com/CyberTruckRonin/status/2084661432960880793) | Identity facts can be true without “evil maid” / intentional plant framing |
| Dice / multisig survivors | many; e.g. diceroll reports | Mitigation paths worked as designed for users who BYOE |
| Inventory destroy + hotfixes | official posts | Consistent with damage control, not only with cover-up |

### Best-form non-guilt narrative (speculative assembly)

1. **Composition bug is a classic integration failure:** same symbol names,
   `#ifndef` vs value, submodule PRNG — exactly the sort of thing that survives
   partial review (E17 “binary had TRNG, wrong `rng_get` linked”).
2. **Nym use can be privacy / old habit / side project** without theft intent;
   still bad OPSEC, but not a completed crime theory. Gray also commits under
   **real name** on the same repos (hurts “pure burner” story).
3. **“Unaware until today”** can mean unaware of **this root cause**, even if
   sparse unexplained drains existed and were mis-triaged (malware / user error
   assumptions).
4. **RFC6979 post (E23):** a senior engineer under stress free-associates to
   the scariest AI-crypto failure mode he knows (deterministic nonces). Wrong,
   but human. darosior corrects within ~90 minutes.
5. **Migration advice** is mandatory under active exploitation whether or not
   staff are saints.
6. **Open source + AI:** vendor’s own theory that attackers used AI on public
   firmware is consistent with third-party discovery, not insider monopoly.

**Hardest gap for pure innocence of process:** years of open defect, empty
commit subjects, shrugged May 2025 report claim, and dual identity still demand
a **governance / culture** indictment even if theft is false.

---

---

# Part IV — Collection and on-chain plans

## Ongoing collection plan

Companion to
[`2026-08-04-cto-inside-job-thesis-analysis.md`](2026-08-04-cto-inside-job-thesis-analysis.md).

Ordered by **ability to change claim status**, not by discourse volume.

---

## Lane 1 — On-chain (priority)

See
Part IV of this document.

- [ ] Mk3 weak keyspace reproduction + address intersect
- [ ] Prior-victim address membership tests (when addresses available)
- [ ] Galaxy/Kelbie ledger JSON receipt + collector clustering

---

## Lane 2 — Cheap forensic closes

| Item | Status |
| --- | --- |
| Audit 10 non-Gray PGP commits | **Done E24a** |
| TZ consistency switck vs Gray | **Done E24b** |
| switck/firmware fork vs upstream chronology | **Done E24c** |
| studentofthings credit / official reply | **Nuanced E24d** |
| Full switck X 54-post archive | **Done enough** — 52/54 in `receipts/2026-08-04-switck-x-archive.json` |
| “Thanks @DocHex… Coldcard someday” primary | **Captured** E25 |
| Galaxy collector address receipt | **Done** E27 |
| Public Mk3 RNG PoC review/reproduce | **Open** (repo found) |
| Retry Wayback saves for failed URLs | **Open** |
| Import scgbckbone public keys and confirm Good on their 2 commits | Optional polish |

---

## Lane 3 — Preservation

| Item | Status |
| --- | --- |
| Local HTML digests for 17 key URLs | **Done** — archive index |
| Git-store multi-MB HTML | **Skipped** (size); digests only |
| Wayback retry queue | Blog/GitHub partial; X often 523 — retry |
| PACER / Ontario court watch | **Not stood up** — note only |
| Recurring X watch (DocHex nym rebuttal, drain movement) | **Not stood up** — manual |

---

## Lane 4 — Doc structure

| Item | Status |
| --- | --- |
| Split claim ledger vs evidence detail | **Done** |
| Flip-artifact column on A–I | **Done** |
| On-chain plan satellite | **Done** |
| Keep detail file append-only | **Policy** |

---

## Explicit non-goals

- Unknown-key hunting against random third parties.
- Public accusation products without Assurance/owner gates.
- Treating social loss totals as fixed economic truth.

## On-chain keyspace and prior-drain plan

Status: **collection / engineering plan.** Does not authorize unknown-key search
against live wallets beyond public chain data and already-published victim
ledgers. Follow
[`2026-08-01-omega-coldcard-forensic-practice-runbook.md`](2026-08-01-omega-coldcard-forensic-practice-runbook.md)
stopping rules.

Related claim ledger:
[`2026-08-04-cto-inside-job-thesis-analysis.md`](2026-08-04-cto-inside-job-thesis-analysis.md)
(claims **G**, **D**, scenarios **S5/S6**).

---

## Why this is the highest flip-value work

Public identity work (A/B) is already strong. **Theft / prior-knowledge claims
stall** without:

1. Independent verification of the **weak keyspace → real addresses** map.
2. Tests of whether **pre-2026 drain victims** sit inside that space (**G**).
3. A durable **drain ledger** for any future attribution to plug into (**D**).

---

## Arm 1 — Reproduce weak keyspace (Mk3 ~40-bit class)

### Inputs (public)

- Mechanism docs: [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md),
  Coinkite
  [technical backgrounder](https://blog.coinkite.com/entropy-technical-backgrounder/)
  (~40-bit Mk3 / ~72-bit Mk4–Q under stated assumptions).
- MicroPython STM32 `rng.c` Yasmarang fallback path (upstream May 2018;
  relevant only once linked into seed gen Mar 2021).
- Kelbie / Galaxy public address sets when available:
  [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md).

### Steps

1. Pin exact firmware versions under test (Mk3 4.0.1–4.1.9 primary).
2. Implement or import a **read-only** candidate seed generator matching the
   documented fallback path (no device bricking; no customer seed handling).
3. Enumerate or sample the reduced space with explicit resource bounds and a
   written stop condition (runbook).
4. Derive P2WPKH/P2PKH addresses for candidates; intersect with:
   - published drain address lists;
   - optional self-reported victim addresses (only with consent / public posts).
5. Record: version pins, code digest, hit rate, false-positive controls.

### Success criterion

At least one **independently derived** address that matches a published drain
or victim address, with reproducible steps. That upgrades vendor blast-radius
prose to **fixture-grade** evidence.

### Non-goals

- Searching for unknown third-party wallets to empty them.
- Claiming total economic loss from partial keyspace coverage.

---

## Arm 2 — Prior-drain claims (flip G)

### Inputs

Social claims pointing at 2022–2024 drains (detail E12/E18). Any public address
from those claims.

### Steps

1. Build a table: claim source → date → address (if any) → model/firmware if
   stated.
2. Run membership test against Arm 1 keyspace (or against a bloom/filter of
   generated addresses).
3. Outcomes:
   - **Hit:** G upgrades toward “on-chain corroborated prior exploitation.”
     Pressures E17 “unaware until today.”
   - **Miss:** claim may still be true (different bug, phishing, wrong address)
     — record as non-confirming.
   - **No address:** claim stays social-only.

### Success criterion

Any single pre-2026 address with public provenance that hits the weak space.

---

## Arm 3 — Drain ledger ingest (dataset for D)

1. Import Galaxy Research threads / Kelbie generated ledgers as versioned JSON
   under `docs/coldcard/receipts/` (public data only).
2. Cluster collectors; label consolidation waves; note exchange deposit
   candidates without doxxing.
3. Re-check periodically for movement (watch lane).

This does **not** identify staff; it builds the substrate for LE/exchange
attribution later.

---

## Implementation ownership

| Lane | Owner surface |
| --- | --- |
| Keyspace tool + tests | Omega forensics / practice runbook code paths |
| Ledger JSON receipts | `docs/coldcard/receipts/` |
| Claim status updates | claim ledger A–I table |

---

## Current status (2026-08-04, continued)

| Arm | Status |
| --- | --- |
| Arm 1 keyspace reproduction | **Planned** — public educational PoC candidate noted: [HenryqueBrito/coldcard-mk3-rng-poc](https://github.com/HenryqueBrito/coldcard-mk3-rng-poc) (Yasmarang Mk3). Review license + reproduce before trust. |
| Arm 2 prior-victim membership | **Blocked on victim addresses** + Arm 1 |
| Arm 3 ledger ingest | **Partial** — public Galaxy collector set recorded in Part V (Galaxy addresses); Kelbie still separate |

---

# Part V — Archive digests and public address receipts

## Archive index

Status: **local HTML snapshots + digests** taken during the Fable-directed pass.
Large blobs are **not** checked into git (multi-MB GitHub/X pages). Digests prove
what was retrieved. Prefer Wayback for durable public mirrors when save succeeds.

| Source (logical URL) | HTTP | bytes | sha256 |
| --- | ---: | ---: | --- |
| `https://blog.coinkite.com/coldcard-mk3-seed-generation-warning/` | 200 | 30209 | `eef06bc778f09ffb…` |
| `https://blog.coinkite.com/entropy-technical-backgrounder/` | 200 | 32378 | `b856a519abf33668…` |
| `https://www.blockchainunmasked.com/post/coldcard-seed-flaw-38m` | 200 | 1391664 | `4f1d683e26ece0c5…` |
| `https://x.com/DocHex/status/2082925350732870062` | 200 | 166330 | `dd36a20639fd3526…` |
| `https://x.com/COLDCARDwallet/status/1447213375398846473` | 200 | 198688 | `d1ca3f6575e936ee…` |
| `https://x.com/jamesob/status/2084620229389197453` | 200 | 217834 | `840ec85b3504ac17…` |
| `https://x.com/jamesob/status/2084624605969350915` | 200 | 199585 | `53325572fc7bca4f…` |
| `https://x.com/zherbert/status/2084647957526167853` | 200 | 206207 | `1f457c4a60d334fd…` |
| `https://x.com/clay_garrett/status/2083247006139503065` | 200 | 179672 | `d48b74e7934b9121…` |
| `https://x.com/BlockUnmasked/status/2083210091671568874` | 200 | 195061 | `34c7a2736b01cca5…` |
| `https://x.com/Zenul_Abidin/status/2083756420843839872` | 200 | 154211 | `a39bf7a37fd201a3…` |
| `https://x.com/studentofthings/status/2084007449267188163` | 200 | 137222 | `b853776ba8c43dd2…` |
| `https://x.com/AtlantisPleb/status/2084660832596570417` | 200 | 89625 | `04f73efc0ff665c0…` |
| `https://x.com/darosior/status/2082948440720359924` | 200 | 139353 | `65905236239f5a90…` |
| `https://github.com/switck/libngu/commit/f19de0527a49e560203102288ae4bc9740a32d96` | 200 | 636157 | `43bab6f3ac0116c9…` |
| `https://github.com/Coldcard/firmware/commit/b18723dddb6d751c39978e4364b56b2414f68b47` | 200 | 590297 | `8aad4ca45fb95500…` |
| `https://insider.btcpp.dev/p/when-randombytes-runs-but-doesnt` | 200 | 226842 | `973da5846dcc052d…` |

## Full local snapshot digests

```json
[
  {
    "file": "https---blog-coinkite-com-coldcard-mk3-seed-generation-warning-.html",
    "bytes": 30209,
    "sha256": "eef06bc778f09ffbe688700d604e80c7134520fb7685fbcf8af06e62db0b3c1e",
    "http_code": "200"
  },
  {
    "file": "https---blog-coinkite-com-entropy-technical-backgrounder-.html",
    "bytes": 32378,
    "sha256": "b856a519abf33668516b9f4270a243afa7aa1bde12647da0749c69efcee74432",
    "http_code": "200"
  },
  {
    "file": "https---github-com-Coldcard-firmware-commit-b18723dddb6d751c39978e4364b56b2414f6.html",
    "bytes": 590297,
    "sha256": "8aad4ca45fb9550016d76ebbc4f9ea3c7d0b7e88eda316e77aef52a13943a73a",
    "http_code": "200"
  },
  {
    "file": "https---github-com-switck-libngu-commit-f19de0527a49e560203102288ae4bc9740a32d96.html",
    "bytes": 636157,
    "sha256": "43bab6f3ac0116c943e3d76fde905e8c61458a926e9074dc0afc7e55f7e42bf3",
    "http_code": "200"
  },
  {
    "file": "https---insider-btcpp-dev-p-when-randombytes-runs-but-doesnt.html",
    "bytes": 226842,
    "sha256": "973da5846dcc052d728075104e1208401bd9c6dd5a6cf8fa3705d992d123a65a",
    "http_code": "200"
  },
  {
    "file": "https---www-blockchainunmasked-com-post-coldcard-seed-flaw-38m.html",
    "bytes": 1391664,
    "sha256": "4f1d683e26ece0c5618bef958bcc1606d8e9f642dddb10fe574bdace24754e4c",
    "http_code": "200"
  },
  {
    "file": "https---x-com-AtlantisPleb-status-2084660832596570417.html",
    "bytes": 89625,
    "sha256": "04f73efc0ff665c0a29493ca0362f3c1636e32ca2fbd2981d80ba4907d0da9e2",
    "http_code": "200"
  },
  {
    "file": "https---x-com-BlockUnmasked-status-2083210091671568874.html",
    "bytes": 195061,
    "sha256": "34c7a2736b01cca570e67913ec35ac4d29caa6d420c6992ba37b1ab92dd7c670",
    "http_code": "200"
  },
  {
    "file": "https---x-com-COLDCARDwallet-status-1447213375398846473.html",
    "bytes": 198688,
    "sha256": "d1ca3f6575e936ee153a231cf50c114481c11e0cd70ea1a26d90ae24826d7283",
    "http_code": "200"
  },
  {
    "file": "https---x-com-DocHex-status-2082925350732870062.html",
    "bytes": 166330,
    "sha256": "dd36a20639fd3526bc9b348acfe80b722601691dc6d24350acb94ff39de7cb42",
    "http_code": "200"
  },
  {
    "file": "https---x-com-Zenul-Abidin-status-2083756420843839872.html",
    "bytes": 154211,
    "sha256": "a39bf7a37fd201a38ffafc51f91951f50e599bc38599b796447b36fe9c3bd8da",
    "http_code": "200"
  },
  {
    "file": "https---x-com-clay-garrett-status-2083247006139503065.html",
    "bytes": 179672,
    "sha256": "d48b74e7934b9121ac2be7539ea493d21a73eafee8aba55b942f6194b2ede01c",
    "http_code": "200"
  },
  {
    "file": "https---x-com-darosior-status-2082948440720359924.html",
    "bytes": 139353,
    "sha256": "65905236239f5a90458a23bb5f1631277fb4572c614e0aaa1241a860e95429fa",
    "http_code": "200"
  },
  {
    "file": "https---x-com-jamesob-status-2084620229389197453.html",
    "bytes": 217834,
    "sha256": "840ec85b3504ac1790d020c63873ee5105e23300822c5d4f633e6dae9f8be356",
    "http_code": "200"
  },
  {
    "file": "https---x-com-jamesob-status-2084624605969350915.html",
    "bytes": 199585,
    "sha256": "53325572fc7bca4f232acfcba0aa7cbc7940cf73230198a5e20dfc27259b65b2",
    "http_code": "200"
  },
  {
    "file": "https---x-com-studentofthings-status-2084007449267188163.html",
    "bytes": 137222,
    "sha256": "b853776ba8c43dd2558659f9695358d75aca80ae77e20cf7bba6618bd0a8d998",
    "http_code": "200"
  },
  {
    "file": "https---x-com-zherbert-status-2084647957526167853.html",
    "bytes": 206207,
    "sha256": "1f457c4a60d334fdbb102dab95c70b3da1dc151384fb86ac4516c89d7c30c57d",
    "http_code": "200"
  }
]
```

Snapshot time (operator machine): 2026-08-04 Fable pass.

## Galaxy public attacker addresses

**Label:** external observation. Addresses copied from a public `@glxyresearch`
post for monitoring/dataset purposes. Not independent verification that every
UTXO is attacker-controlled. Not a loss total authority.

**Source post:** [2083623556542349552](https://x.com/glxyresearch/status/2083623556542349552)  
**Time:** 2026-08-01T18:39:02.000Z  
**Claim in post:** seven addresses from Waves 1 and 2 collectively hold
**1,158.8148 BTC** (at time of post); also monitoring 293 P2WSH vaults from
Wave 3.

## Addresses (as published)

```
bc1qq85v2c926eg6pgxhwp6q7lf6cnsz80qs3fcu9r
bc1qx76cae2706qd5q576feh7xq8rfcsjpf2htfhe3
bc1q8jy96fe5lf8vfugydnte3cguk92gpev7kwtp3q
bc1qtfrwa4j6rmj9rsgspv6a0yjumkg39js2numu75
bc1qnk4zh9qcnap2mycp56qjrgza3cc8ylrh8fecp0
bc1qmd5m5ktv7m5ffujxv4248fxv36myvdx79n8jp6
bc1qsjrf5ze5tmulz7y2x4pc7qaex2a35sanp3rqlx
```

## Related Galaxy claims (same investigation thread family)

| Post | Claim (summary) |
| --- | --- |
| [2084411904924045370](https://x.com/glxyresearch/status/2084411904924045370) | High-confidence **1,596 BTC** / ~7300 addresses / 3 waves + 14 smaller; unconfirmed path toward ~$130m / ~2k BTC |
| [2084411915745595527](https://x.com/glxyresearch/status/2084411915745595527) | Wave 4 suspected, not in top-line without victim confirmation; would bring ~2055 BTC |
| [2084411918861652194](https://x.com/glxyresearch/status/2084411918861652194) | Shared addresses with US federal LE, exchanges, compliance firms; ~90% of stolen coins unmoved at that update |
| [2083623541921001756](https://x.com/glxyresearch/status/2083623541921001756) | Vulnerable firmware ship ~2021-03-17 / block **674951**; Waves 1–3 coins created after that block |

## Use

Feed collector clustering and watch scripts. Do **not** treat as complete
attacker set. Re-fetch live UTXO set before any operational decision.

---

## Related OpenAgents docs

- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md)
- [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
- [`2026-08-01-bitcoin-plus-plus-oped-analysis.md`](2026-08-01-bitcoin-plus-plus-oped-analysis.md)
- [`2026-08-04-x-posts-recent-summary.md`](2026-08-04-x-posts-recent-summary.md)
- [`receipts/2026-08-04-switck-x-archive.json`](receipts/2026-08-04-switck-x-archive.json)
