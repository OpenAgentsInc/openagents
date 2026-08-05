# Coinkite CTO / "inside job" thesis — consolidated evidence and analysis

Status: **single consolidated document** (claims + full evidence log +
scenarios + collection plans). Not a legal finding and not a public product claim.

Last updated: 2026-08-05 ~06:05Z (overnight sweep E35–E39; DocHex still silent).

Related receipts (machine data, kept beside this file):

- [`receipts/2026-08-04-switck-x-archive.json`](receipts/2026-08-04-switck-x-archive.json) — full `@switck` timeline dump (52 posts)
- [`receipts/2026-08-04-darosior-bikes-hanlon-thread.json`](receipts/2026-08-04-darosior-bikes-hanlon-thread.json) — full E29 thread (92 posts: BikesandBitcoin Hanlon root + darosior QT conversation)
- [`receipts/2026-08-04-later-discourse-sweep.json`](receipts/2026-08-04-later-discourse-sweep.json) — later sweep ~16:54Z–21:15Z (117 posts after E29 cutoff; verifications)
- [`receipts/2026-08-05-overnight-discourse-sweep.json`](receipts/2026-08-05-overnight-discourse-sweep.json) — overnight sweep ~21:00Z Aug 4 – 06:05Z Aug 5 (~315 posts; tripwire dashboard)

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
| **G** | Vendor had prior victim reports / notice years before Jul 2026 | **Collecting** | Multi-source social claims (E12/E18); jamesob May 2025 report (E29); inverse_hanlon also claims Apr 2021 Telegram + “LNGU Clean up” Signal (E31 — **needs primary**) | **Primary ticket/email/Signal** *or* pre-2026 victim address ∈ weak keyspace (on-chain) |
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
they do not erase the opacity. darosior’s later take (E29) that a nym can mean
“more eyes” / seriousness theater is a clean non-malicious version of the same
opacity. Reputation and governance guilt run high; theft still needs a proceeds
bridge.

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
haste than a careful trap. darosior states the backdoor theory is “not
compelling” and that a real backdoor would be less conspicuous (E29). S6 should
stay on the board as the hypothesis social media wants, but it demands intent
evidence the current set does not supply.

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
signals — **without** proven long-game heist. When unsure about malice, weight
technical third parties who say **extreme negligence > backdoor** (e.g. darosior
E29).

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

### E29 — Hanlon / nym / backdoor thread: full inventory (2026-08-04)

Credible technical voices debate **intentional backdoor** vs **extreme negligence**
after `@jamesob` answers `@BikesandBitcoin`’s Hanlon-razor post. Anchor for this
row is darosior’s QT
[2084661793624924500](https://x.com/darosior/status/2084661793624924500); the
**full reply tree** is inventoried so no material post in that conversation is
left only in the API dump.

**Machine receipt (all 92 posts + metrics):**
[`receipts/2026-08-04-darosior-bikes-hanlon-thread.json`](receipts/2026-08-04-darosior-bikes-hanlon-thread.json)

#### Scope of inventory

| Scope | Conversation ID | Posts captured |
| --- | --- | --- |
| Root Hanlon post + all replies (incl. jamesob branch) | `2084630840982831568` | 88 |
| darosior QT as its own conversation (QT + 3 replies) | `2084661793624924500` | 4 |
| **Total unique posts** | | **92** |

**Quoted parent outside the conversation set** (root quotes this; not counted in
the 92): `@raw_avocado`
[2084626833182458302](https://x.com/raw_avocado/status/2084626833182458302) —
“please stop trying to make it sound that coinkite knew about this bug for
likes… 99.99% chance they did not know.”

**Probe window:** posts from **2026-08-04T13:21:37Z** through
**2026-08-04T16:54:25Z** (UTC). Author mix (top): BikesandBitcoin 8, jamesob 8,
4moonsettler 4, udiWertheimer 4, darosior 3, plus 50+ other accounts (full
counts in receipt).

**Label:** external discourse observation. Engagement counts are point-in-time
probes, not frozen truth.

#### Spine (material posts, full text where recovered)

These are the load-bearing posts for claims A/B, S1–S3 vs S6, and the May 2025
prior-notice claim. Shorter reactions live only in the full inventory table.

**1. Root — `@BikesandBitcoin` Hanlon frame**
[2084630840982831568](https://x.com/BikesandBitcoin/status/2084630840982831568)
(~38 likes; quotes raw_avocado):

> Never attribute to malice that which is adequately explained by stupidity.
>
> Alex is exactly right.
>
> Conspiracy theories will get you clicks, but you’re also blackpilling a
> generation of bitcoiners.

**2. Branch root — `@jamesob` nym + prior report**
[2084640418918957430](https://x.com/jamesob/status/2084640418918957430)
(~552–559 likes; highest engagement in tree):

> It's looking worse and worse. Nvk was a friend of mine, but for the CTO to
> commit the buggy code under a pseudonym has no reasonable explanation. He was
> already publicly working on security critical software, so a 'nym would not
> have bought him anything.
>
> I reported the possibility of the defect directly, by name, and it was
> shrugged off.
>
> High potential that something isn't right.

**Use:** steelman guilt (nym OPSEC + May-ish prior notice); does **not** alone
prove theft. Cross-check with E12/E18 on prior notice.

**3. jamesob confirms he reported before explosion**
[2084643276859039955](https://x.com/jamesob/status/2084643276859039955) — reply
to `@raw_avocado` “you told them…?”: **“Yes”** (~53 likes).

**4. Malice confirmation bar — Bikes ↔ jamesob**
- Bikes [2084646214373691456](https://x.com/BikesandBitcoin/status/2084646214373691456):
  “How would one go about confirming malice in this case?”
- jamesob [2084646588211667297](https://x.com/jamesob/status/2084646588211667297):
  “I think not possible short of more evidence than we have now” (~37 likes).

**Use:** even the loudest nym critic **refuses** a malice verdict from public
data alone — aligns with this document’s A/B strong, C/D open.

**5. “More than one dev” theater (non-theft nym theory)**
`@MrHodl` [2084646338780635269](https://x.com/MrHodl/status/2084646338780635269)
(~9 likes): more likely they wanted it to look like more than one Coinkite
dev; follow-up “which is honestly not that much better.”

**6. `@udiWertheimer` — secret-CTO more responsible than secret-nobody; dumb
malice path; “evil maid” self-story**
- [2084646756592082989](https://x.com/udiWertheimer/status/2084646756592082989):
  post-exploit every fishy point looks worse; but secret-CTO-as-anon is still
  more responsible than critical code from a true nobody; dumb reasons possible.
- jamesob [2084647461029572834](https://x.com/jamesob/status/2084647461029572834):
  “why pick and choose what to do under a nym?” whole firmware equally sensitive.
- udi [2084647846628794747](https://x.com/udiWertheimer/status/2084647846628794747):
  even if malicious, “a very dumb way to do it.”
- jamesob [2084648217069646012](https://x.com/jamesob/status/2084648217069646012):
  “RNG/ECDSA interaction mediated by a complete no-one library” is what flagged
  attention first (~12 likes).
- udi [2084650014207275198](https://x.com/udiWertheimer/status/2084650014207275198):
  speculative “deranged” privacy/evil-maid conference story as non-theft nym
  motive (explicitly labeled absurd-but-seen-before).

**7. Grok bot in-thread (identity fact, non-verdict)**
`@grok` [2084642145470763449](https://x.com/grok/status/2084642145470763449)
reply to “why pseudonym?”: nyms routine; GPG on libngu matches Peter Gray /
DocHex key under switck handle — same cryptographic claim as E3, still not a
theft finding.

**8. Retirement-attack official post re-cited as “admission”**
`@PrinceHeat44402` [2084648769459482693](https://x.com/PrinceHeat44402/status/2084648769459482693)
quotes the 2021 COLDCARDwallet “retirement attack” definition (E10) as looking
like admission of guilt if you understand the mistakes. **Dual-use** with E10
analysis — joke is real; heist inference speculative (claim F).

**9. Culture / shrug claims**
- `@alpacasw` [2084649788289531917](https://x.com/alpacasw/status/2084649788289531917):
  shrugging off then attacking reporters “par for the course.”
- `@zherbert` [2084649694358040792](https://x.com/zherbert/status/2084649694358040792):
  thanks jamesob for speaking up.
- `@raw_avocado` [2084645021811155089](https://x.com/raw_avocado/status/2084645021811155089):
  still “gross incompetence and vanity” possible (~25 likes).

**10. Flight-game-theory branch (not evidence of theft)**
`@4moonsettler` and replies debate “if intentional why not run?” vs plausible
deniability / stick-around (`@TheDesignFlaw`, `@autocatalytics`, later
`@PeachFrog99` influencer-complicity speculation). **Speculation only** — no
proceeds link.

**11. darosior #1 — extreme negligence > malice**
[2084659808519463034](https://x.com/darosior/status/2084659808519463034)
(~15 likes; reply to jamesob):

> He did commit the change to libngu under doc hex, so using a nym to commit to
> libngu itself doesn't seem to be hinting at a cover up?
>
> At that point i still believe **extreme negligence explains this better than
> malice.**

**12. darosior #2 (anchor QT) — backdoor not compelling**
[2084661793624924500](https://x.com/darosior/status/2084661793624924500)
(2026-08-04T15:24:37Z, ~22 likes; quotes #11):

> The nym usage could also be explained by simply trying to portray the project
> as more serious than it was (more eyes).
>
> And if you really wanted to introduce a backdoor, there would be less
> conspicuous ways of doing it.
>
> **The backdoor theory is not compelling at this point.**

**13. jamesob reply to darosior QT — “why push non-backdoor?”**
[2084666514439848413](https://x.com/jamesob/status/2084666514439848413)
(~33 likes):

> If that were the intent, the obvious approach would be to have anons submit
> tests, not completely rework random number generation.
>
> Having an anon mediate your RNG is a huge red flag and is what got me
> interested in the first place.
>
> Your theory doesn't make a lot of sense. It's possible, but why are you
> pushing it?

**14. darosior #3 — wildfire / conspiracy preference; red flags still real**
[2084668646924656893](https://x.com/darosior/status/2084668646924656893)
(~11 likes):

> Not pushing anything. To be honest i'm worried suggesting there was a backdoor
> will spread like wildfire, because 1) people are still emotional after all the
> losses 2) our community often prefers believing conspiracy over more boring
> explanations. That seemed unfair and figured i'd offer counterpoints.  -- Just
> kidding, i'm 'pushing' it because Mossad paid me to of course!
>
> Agree with the red flags, but it's not news to anybody who actually had a look
> at their project. (That it was *that* broken definitely is news to me though,
> of course.)

**15. Identity confirmation sub-thread**
- `@Bitcoin_phan` asks if switck was CTO’s nym → jamesob “Confirmed” → “true if
  you believe in cryptography”
  ([2084664353253474593](https://x.com/jamesob/status/2084664353253474593),
  [2084664614382444896](https://x.com/jamesob/status/2084664614382444896)).
- `@SimplestBTCBook` mishears as NVK; `@bleighky` corrects to Peter Gray.
- `@xstoicunicornx` / `@willreeves` link older public nym self-indication posts
  (corroborative color for dual identity, not new crypto).

**16. OPSEC irony (non-guilt steelman)**
`@NicerInPerson`
[2084678159018967069](https://x.com/NicerInPerson/status/2084678159018967069):
evil-genius plant under nym while signing with real identity is “dumb theory”;
follow-up [2084680027782643986](https://x.com/NicerInPerson/status/2084680027782643986)
offers liability-insulation as non-theft nym motive for a security lib.

**17. Hanlon defense close**
Bikes continues: more likely find stupid people than ill-intentioned; razor is a
**starting heuristic**, not an answer
([2084671579238154655](https://x.com/BikesandBitcoin/status/2084671579238154655),
[2084673484555243986](https://x.com/BikesandBitcoin/status/2084673484555243986)).
`@denverbitcoin`: negligence and malice “become indistinguishable” under total
loss ([2084671077746380842](https://x.com/denverbitcoin/status/2084671077746380842)).

**18. Low-signal QT replies**
`@JWWeatherman_` [2084674008025378905](https://x.com/JWWeatherman_/status/2084674008025378905)
only “😂” under the darosior QT — inventoried for completeness, no analytical weight.

#### Full chronological inventory (all 92 posts)

One line per post. Full text + metrics in the JSON receipt. Times UTC on
2026-08-04.

| # | Time | Author | Post | ♥ | Summary |
| --- | --- | --- | --- | ---: | --- |
| 1 | 13:21:37 | `@BikesandBitcoin` | [2084630840982831568](https://x.com/BikesandBitcoin/status/2084630840982831568) | 38 | Never attribute to malice that which is adequately explained by stupidity.   Alex is exactly right.  Conspiracy theories will get you clicks, but you’re also... |
| 2 | 13:27:17 | `@jacklesser_` | [2084632266072809853](https://x.com/jacklesser_/status/2084632266072809853) | 1 | @BikesandBitcoin Unfortunately many are already too blackpilled to ever come back.   But agree nonetheless. Onwards |
| 3 | 13:41:23 | `@JustaLillyBit` | [2084635815087419422](https://x.com/JustaLillyBit/status/2084635815087419422) | 0 | @BikesandBitcoin Do you think Satoshi might have been a conspiracy theorist? |
| 4 | 13:59:34 | `@BikesandBitcoin` | [2084640388703453213](https://x.com/BikesandBitcoin/status/2084640388703453213) | 0 | @JustaLillyBit No. If they were they never would have built anything. |
| 5 | 13:59:41 | `@jamesob` | [2084640418918957430](https://x.com/jamesob/status/2084640418918957430) | 552 | It's looking worse and worse. Nvk was a friend of mine, but for the CTO to commit the buggy code under a pseudonym has no reasonable explanation. He was alre... |
| 6 | 14:03:06 | `@noD7R` | [2084641279879569455](https://x.com/noD7R/status/2084641279879569455) | 2 | @jamesob @BikesandBitcoin some layers of plausible deniability hiding something shadier? |
| 7 | 14:05:07 | `@raw_avocado` | [2084641789080694827](https://x.com/raw_avocado/status/2084641789080694827) | 27 | @jamesob @BikesandBitcoin Wait, you told them there may be a bug, like before this exploded? |
| 8 | 14:05:18 | `@4moonsettler` | [2084641833565515912](https://x.com/4moonsettler/status/2084641833565515912) | 7 | @jamesob @BikesandBitcoin if it was intentional shouldn't they just pull the plug on everything and ran? face change, sex change, new id? |
| 9 | 14:05:44 | `@BitcoinBombz` | [2084641943321973193](https://x.com/BitcoinBombz/status/2084641943321973193) | 0 | @jamesob @BikesandBitcoin @grok why would the CTO commit this buggy code under a pseudonym? |
| 10 | 14:06:32 | `@grok` | [2084642145470763449](https://x.com/grok/status/2084642145470763449) | 0 | @BitcoinBombz @jamesob @BikesandBitcoin Pseudonyms are routine in Bitcoin/cypherpunk work for privacy or project separation. GPG signatures on the libngu com... |
| 11 | 14:11:02 | `@jamesob` | [2084643276859039955](https://x.com/jamesob/status/2084643276859039955) | 53 | @raw_avocado @BikesandBitcoin Yes |
| 12 | 14:14:58 | `@mcchiperson77` | [2084644264970006653](https://x.com/mcchiperson77/status/2084644264970006653) | 2 | @jamesob @raw_avocado @BikesandBitcoin I'm starting to piece things together as well.  I'm not technical like you all, but I think it's possible they knew th... |
| 13 | 14:16:02 | `@4moonsettler` | [2084644534764368275](https://x.com/4moonsettler/status/2084644534764368275) | 1 | @jamesob @BikesandBitcoin what's the game theory of sticking around like a deer caught in headlight?  if you prepared for a rug for years you have thought th... |
| 14 | 14:16:24 | `@bendbotbtc` | [2084644625701118347](https://x.com/bendbotbtc/status/2084644625701118347) | 2 | @jamesob @BikesandBitcoin Thank you for your candor.  It can't be easy. |
| 15 | 14:16:28 | `@osborne_sam` | [2084644643808092409](https://x.com/osborne_sam/status/2084644643808092409) | 1 | @jamesob @BikesandBitcoin I do not want for likes I yearn for truth the internet warps it source code is one avenue to discover truth archive everything |
| 16 | 14:17:58 | `@raw_avocado` | [2084645021811155089](https://x.com/raw_avocado/status/2084645021811155089) | 25 | @jamesob @BikesandBitcoin Just saw your post, that is INSANE!!!!  But to be fair, I still think this could be explained by gross incompetence and vanity, fwiw. |
| 17 | 14:18:33 | `@osborne_sam` | [2084645169459286267](https://x.com/osborne_sam/status/2084645169459286267) | 1 | @jamesob @BikesandBitcoin when did verify become yet a slogan |
| 18 | 14:22:43 | `@BikesandBitcoin` | [2084646214373691456](https://x.com/BikesandBitcoin/status/2084646214373691456) | 11 | @jamesob How would one go about confirming malice in this case? |
| 19 | 14:23:12 | `@MrHodl` | [2084646338780635269](https://x.com/MrHodl/status/2084646338780635269) | 9 | @4moonsettler @jamesob @BikesandBitcoin This right here is why I don't think that was the case. What's more likely is that they wanted to make it look like i... |
| 20 | 14:23:48 | `@decentmoney2009` | [2084646489390010746](https://x.com/decentmoney2009/status/2084646489390010746) | 0 | @raw_avocado @jamesob @BikesandBitcoin Why the unnecessary nym though? |
| 21 | 14:24:12 | `@jamesob` | [2084646588211667297](https://x.com/jamesob/status/2084646588211667297) | 37 | @BikesandBitcoin I think not possible short of more evidence than we have now |
| 22 | 14:24:43 | `@MrHodl` | [2084646718373597444](https://x.com/MrHodl/status/2084646718373597444) | 4 | @4moonsettler @jamesob @BikesandBitcoin (Which is honestly not that much better) / |
| 23 | 14:24:52 | `@udiWertheimer` | [2084646756592082989](https://x.com/udiWertheimer/status/2084646756592082989) | 8 | obviously after the exploit, every fishy data point just makes things look worse  however in a world where this didn't get exploited (or the vulnerability di... |
| 24 | 14:25:21 | `@BikesandBitcoin` | [2084646880852816257](https://x.com/BikesandBitcoin/status/2084646880852816257) | 9 | @jamesob An unbelievable situation all around. Thanks for taking the time to try to unpack it. |
| 25 | 14:27:40 | `@jamesob` | [2084647461029572834](https://x.com/jamesob/status/2084647461029572834) | 17 | @udiWertheimer @BikesandBitcoin But the fishy thing is: why pick and choose what to do under a nym? The whole firmware codebase is basically equally revealin... |
| 26 | 14:29:12 | `@udiWertheimer` | [2084647846628794747](https://x.com/udiWertheimer/status/2084647846628794747) | 7 | @jamesob @BikesandBitcoin i don’t know why. there could be an entire range of very dumb reasons  i’d argue that even if the reason was malicious this would b... |
| 27 | 14:30:40 | `@jamesob` | [2084648217069646012](https://x.com/jamesob/status/2084648217069646012) | 12 | @udiWertheimer @BikesandBitcoin It's also *obviously* a bad look for the company to have your RNG/ECDSA interaction mediated by a complete no-one library, wh... |
| 28 | 14:30:49 | `@autocatalytics` | [2084648254936072500](https://x.com/autocatalytics/status/2084648254936072500) | 2 | @4moonsettler @jamesob @BikesandBitcoin Maybe they don’t count on someone else finding it first. But I put low probability this was intentional personally bu... |
| 29 | 14:31:59 | `@4moonsettler` | [2084648548730061128](https://x.com/4moonsettler/status/2084648548730061128) | 2 | @autocatalytics @jamesob @BikesandBitcoin i have been forced to eat my disbelief a few times too. |
| 30 | 14:32:01 | `@lonebulwark` | [2084648556535631912](https://x.com/lonebulwark/status/2084648556535631912) | 0 | @jamesob @udiWertheimer @BikesandBitcoin what issue did you find and report? |
| 31 | 14:32:52 | `@PrinceHeat44402` | [2084648769459482693](https://x.com/PrinceHeat44402/status/2084648769459482693) | 5 | @jamesob @BikesandBitcoin If you can read the code and understand the mistakes, this tweet looks like an admission of guilt.   Shocked it's still up.   https... |
| 32 | 14:33:36 | `@TheDesignFlaw` | [2084648955674018174](https://x.com/TheDesignFlaw/status/2084648955674018174) | 3 | @MrHodl @4moonsettler @jamesob @BikesandBitcoin I think this is most likely incompetence too, but not because they haven't run. All you need to not have to r... |
| 33 | 14:34:11 | `@Conor21m` | [2084649102881481088](https://x.com/Conor21m/status/2084649102881481088) | 1 | @jamesob @BikesandBitcoin you'd imagine they'd have patched this firmware on newer products if they knew the bug existed. |
| 34 | 14:34:28 | `@udiWertheimer` | [2084649171861004419](https://x.com/udiWertheimer/status/2084649171861004419) | 7 | @jamesob @BikesandBitcoin yes. you’re absolutely right. it’s diabolically stupid |
| 35 | 14:36:32 | `@zherbert` | [2084649694358040792](https://x.com/zherbert/status/2084649694358040792) | 19 | @jamesob @BikesandBitcoin Thank you for speaking up James |
| 36 | 14:36:55 | `@alpacasw` | [2084649788289531917](https://x.com/alpacasw/status/2084649788289531917) | 9 | @jamesob @BikesandBitcoin Shrugging off things was par for the course for that team though. Well, shrugging off then attacking those who pointed it out. |
| 37 | 14:37:24 | `@4moonsettler` | [2084649912654864395](https://x.com/4moonsettler/status/2084649912654864395) | 1 | @TheDesignFlaw @MrHodl @jamesob @BikesandBitcoin "All you need to not have to run is plausible deniability."  it doesn't work out well, if you run it forward... |
| 38 | 14:37:44 | `@masunobom` | [2084649994578022463](https://x.com/masunobom/status/2084649994578022463) | 1 | @BikesandBitcoin That’s Reddit programming https://t.co/YcGt1WG9U3 |
| 39 | 14:37:48 | `@udiWertheimer` | [2084650014207275198](https://x.com/udiWertheimer/status/2084650014207275198) | 7 | i don't know peter at all, i don't believe i ever met him or spoke to him, but having had social experiences with some other permanently online folks who con... |
| 40 | 14:39:16 | `@ChuckSRQ` | [2084650381179490485](https://x.com/ChuckSRQ/status/2084650381179490485) | 0 | @BikesandBitcoin https://t.co/uErdCg2U61 |
| 41 | 14:39:37 | `@BikesandBitcoin` | [2084650470254227912](https://x.com/BikesandBitcoin/status/2084650470254227912) | 0 | @masunobom You think BIP 110 is real. Opinion disregarded. |
| 42 | 14:40:09 | `@cici7869` | [2084650603041398875](https://x.com/cici7869/status/2084650603041398875) | 0 | @jamesob @BikesandBitcoin Honestly, I really want to appreciate @Crypto_Sm1th who brought a lot of light to my life in a short period of time in crypto |
| 43 | 14:42:24 | `@masunobom` | [2084651167905120694](https://x.com/masunobom/status/2084651167905120694) | 0 | @BikesandBitcoin 👍 |
| 44 | 14:42:29 | `@umbrellaXBT` | [2084651189077934320](https://x.com/umbrellaXBT/status/2084651189077934320) | 0 | @BikesandBitcoin @jamesob Here's my thought, their plan was to build a multi billion dollar giant company, they failed so they implemented a shit code at som... |
| 45 | 14:43:30 | `@TheDesignFlaw` | [2084651445597491526](https://x.com/TheDesignFlaw/status/2084651445597491526) | 0 | @4moonsettler @MrHodl @jamesob @BikesandBitcoin I mean, let's just hypothesize for a moment that this trans pseudoanon dev is the Ray Epps of cold storage...... |
| 46 | 14:47:46 | `@TheDesignFlaw` | [2084652520446521696](https://x.com/TheDesignFlaw/status/2084652520446521696) | 0 | @4moonsettler @MrHodl @jamesob @BikesandBitcoin Not to mention the coinjoin exposure. The history of those that transacted with these exposed seeds. The tax ... |
| 47 | 14:49:40 | `@codywhitt16` | [2084652998567870576](https://x.com/codywhitt16/status/2084652998567870576) | 2 | @jamesob @BikesandBitcoin In any other line of work, an analogous error would draw immediate suspicion, maybe worse. |
| 48 | 14:55:48 | `@369magicmanx` | [2084654540142338428](https://x.com/369magicmanx/status/2084654540142338428) | 0 | @BikesandBitcoin @jamesob By confirming that random number generation was disabled and replaced by deterministic number generation.  That doesnt happen by ac... |
| 49 | 14:56:22 | `@TheBTCGame` | [2084654686125301848](https://x.com/TheBTCGame/status/2084654686125301848) | 1 | @decentmoney2009 @raw_avocado @jamesob @BikesandBitcoin Decentralization theater? |
| 50 | 15:06:11 | `@jaybny` | [2084657156406522284](https://x.com/jaybny/status/2084657156406522284) | 1 | @jamesob @BikesandBitcoin something to look into for sure. great work so far btw |
| 51 | 15:11:01 | `@GMONEYPEPE` | [2084658371890942096](https://x.com/GMONEYPEPE/status/2084658371890942096) | 2 | @jamesob @BikesandBitcoin Mossad bud |
| 52 | 15:16:44 | `@darosior` | [2084659808519463034](https://x.com/darosior/status/2084659808519463034) | 15 | @jamesob @BikesandBitcoin He did commit the change to libngu under doc hex, so using a nym to commit to libngu itself doesn't seem to be hinting at a cover u... |
| 53 | 15:19:24 | `@the_nassar` | [2084660481088721324](https://x.com/the_nassar/status/2084660481088721324) | 2 | @jamesob @bdionbtc @BikesandBitcoin To be fair, this proves negligence, stupidity, and ego from their part more than anything else.  Anyway we, as Bitcoiners... |
| 54 | 15:23:56 | `@jordanBTCplz` | [2084661620685382136](https://x.com/jordanBTCplz/status/2084661620685382136) | 0 | @jamesob @BikesandBitcoin Even if they knew about this, what would they have done? I’m not implying they handled it well, but realistically there was no way ... |
| 55 | 15:24:37 | `@darosior` | [2084661793624924500](https://x.com/darosior/status/2084661793624924500) | 22 | The nym usage could also be explained by simply trying to portray the project as more serious than it was (more eyes).  And if you really wanted to introduce... |
| 56 | 15:25:58 | `@charlosrossi` | [2084662133447438475](https://x.com/charlosrossi/status/2084662133447438475) | 0 | @MrHodl @4moonsettler @jamesob @BikesandBitcoin Incompetence &gt; Malice |
| 57 | 15:30:31 | `@Bitcoin_phan` | [2084663280350474420](https://x.com/Bitcoin_phan/status/2084663280350474420) | 1 | @jamesob @BikesandBitcoin You think Switck was the CTO's nym? |
| 58 | 15:34:47 | `@jamesob` | [2084664353253474593](https://x.com/jamesob/status/2084664353253474593) | 15 | @Bitcoin_phan @BikesandBitcoin Confirmed |
| 59 | 15:35:37 | `@Bitcoin_phan` | [2084664563761307852](https://x.com/Bitcoin_phan/status/2084664563761307852) | 0 | @jamesob @BikesandBitcoin Confirmed that you think that or confirmed that that is true?! |
| 60 | 15:35:49 | `@jamesob` | [2084664614382444896](https://x.com/jamesob/status/2084664614382444896) | 5 | @Bitcoin_phan @BikesandBitcoin True if you believe in cryptography |
| 61 | 15:38:48 | `@Bitcoin_phan` | [2084665364944703871](https://x.com/Bitcoin_phan/status/2084665364944703871) | 2 | @jamesob @BikesandBitcoin I said it already, but I'll say it again... Yikes. https://t.co/dxDKBHTRi9 |
| 62 | 15:40:56 | `@mykopikid` | [2084665900712820792](https://x.com/mykopikid/status/2084665900712820792) | 0 | @jamesob @BikesandBitcoin Seems like the ethos of "Don't Trust, Verify" has been eroded over time from the bitcoin community.  Is "Adversarial" thinking be e... |
| 63 | 15:43:22 | `@jamesob` | [2084666514439848413](https://x.com/jamesob/status/2084666514439848413) | 33 | If that were the intent, the obvious approach would be to have anons submit tests, not completely rework random number generation.  Having an anon mediate yo... |
| 64 | 15:44:29 | `@belair6909` | [2084666793646268499](https://x.com/belair6909/status/2084666793646268499) | 0 | @Bitcoin_phan @jamesob @BikesandBitcoin Ockham´s razor? |
| 65 | 15:48:16 | `@kimstrodamus` | [2084667746248474742](https://x.com/kimstrodamus/status/2084667746248474742) | 0 | @BikesandBitcoin I never understood this saying. Why shouldn’t we attribute malice to careless incompetence like this? |
| 66 | 15:51:51 | `@darosior` | [2084668646924656893](https://x.com/darosior/status/2084668646924656893) | 11 | Not pushing anything. To be honest i'm worried suggesting there was a backdoor will spread like wildfire, because 1) people are still emotional after all the... |
| 67 | 15:59:16 | `@GlennPudick` | [2084670512421302654](https://x.com/GlennPudick/status/2084670512421302654) | 0 | @BikesandBitcoin @jamesob Last years Bitcoin dump. Who sold the top? |
| 68 | 16:01:30 | `@denverbitcoin` | [2084671077746380842](https://x.com/denverbitcoin/status/2084671077746380842) | 2 | @BikesandBitcoin Negligence and malice become indistinguishable when outcome is the realization of total loss. |
| 69 | 16:03:30 | `@BikesandBitcoin` | [2084671579238154655](https://x.com/BikesandBitcoin/status/2084671579238154655) | 0 | @kimstrodamus because you're far more likely to find stupid people than ill-intentioned people. |
| 70 | 16:03:38 | `@BikesandBitcoin` | [2084671612331176428](https://x.com/BikesandBitcoin/status/2084671612331176428) | 0 | @belair6909 @Bitcoin_phan @jamesob Hanlon's |
| 71 | 16:04:35 | `@inverse_hanlon` | [2084671852979601811](https://x.com/inverse_hanlon/status/2084671852979601811) | 1 | @BikesandBitcoin @jamesob https://t.co/EDNTZ39d82 you don't get it |
| 72 | 16:06:04 | `@RRulehard` | [2084672223827173561](https://x.com/RRulehard/status/2084672223827173561) | 0 | @jamesob @BikesandBitcoin https://t.co/Y2OFQxC2vU |
| 73 | 16:08:10 | `@kimstrodamus` | [2084672752813060337](https://x.com/kimstrodamus/status/2084672752813060337) | 0 | @BikesandBitcoin That doesn’t really answer why we shouldn’t attribute malice to morons. We can’t just keep living as hostages by the bottom dregs of society. |
| 74 | 16:11:04 | `@BikesandBitcoin` | [2084673484555243986](https://x.com/BikesandBitcoin/status/2084673484555243986) | 0 | @kimstrodamus The point of a razor is to provide a heuristic,  a starting assumption to work from, not an answer. |
| 75 | 16:12:04 | `@SimplestBTCBook` | [2084673736524136858](https://x.com/SimplestBTCBook/status/2084673736524136858) | 1 | @jamesob @bleighky @BikesandBitcoin Wait are you saying @switck was NVK? https://t.co/iW0Z8zhxcM https://t.co/GZqM1PhIEI |
| 76 | 16:13:09 | `@JWWeatherman_` | [2084674008025378905](https://x.com/JWWeatherman_/status/2084674008025378905) | 0 | @darosior 😂 |
| 77 | 16:17:33 | `@GeoffreyGardine` | [2084675114248102297](https://x.com/GeoffreyGardine/status/2084675114248102297) | 0 | @jamesob @BikesandBitcoin if the anon code submission was normal   what other anon submissions are there ? |
| 78 | 16:19:44 | `@xstoicunicornx` | [2084675663982739955](https://x.com/xstoicunicornx/status/2084675663982739955) | 0 | @jamesob @BikesandBitcoin seems like he publicly indicated that he is using a nym from a while back  https://t.co/I4Hb7W7wtg |
| 79 | 16:20:30 | `@bleighky` | [2084675857105199602](https://x.com/bleighky/status/2084675857105199602) | 1 | @SimplestBTCBook @jamesob @BikesandBitcoin @switck Or Peter Gray https://t.co/vFcI1YXeho |
| 80 | 16:20:46 | `@cornfordogs` | [2084675924100784335](https://x.com/cornfordogs/status/2084675924100784335) | 0 | @jamesob @americanhodl8 @BikesandBitcoin So so so sad if true  And let’s just say this community believes in vigilante justice |
| 81 | 16:21:42 | `@belair6909` | [2084676157677420653](https://x.com/belair6909/status/2084676157677420653) | 0 | @BikesandBitcoin @Bitcoin_phan @jamesob I hope.. it is a convoluted way of proceeding |
| 82 | 16:29:39 | `@NicerInPerson` | [2084678159018967069](https://x.com/NicerInPerson/status/2084678159018967069) | 0 | @udiWertheimer @jamesob @BikesandBitcoin It’s a dumb theory :  &gt; Be super evil genius smuggling bugs intentionally into your company’s product under a pse... |
| 83 | 16:32:29 | `@BridgeBTC` | [2084678875058192776](https://x.com/BridgeBTC/status/2084678875058192776) | 2 | @4moonsettler @jamesob @BikesandBitcoin It's early yet |
| 84 | 16:37:04 | `@NicerInPerson` | [2084680027782643986](https://x.com/NicerInPerson/status/2084680027782643986) | 0 | @udiWertheimer @jamesob @BikesandBitcoin There are other explanations for publishing a security lib under a pseudonym (assuming that’s even what happened).  ... |
| 85 | 16:37:40 | `@willreeves` | [2084680179369296223](https://x.com/willreeves/status/2084680179369296223) | 1 | @jamesob @BikesandBitcoin https://t.co/YTYcGhWt8A https://t.co/uSqdMaYgmt |
| 86 | 16:37:59 | `@SimplestBTCBook` | [2084680255349112963](https://x.com/SimplestBTCBook/status/2084680255349112963) | 0 | @bleighky @jamesob @BikesandBitcoin @switck Ok yea hearing that now |
| 87 | 16:43:31 | `@GrauweDakGans` | [2084681651511087345](https://x.com/GrauweDakGans/status/2084681651511087345) | 0 | @raw_avocado @jamesob @BikesandBitcoin Let me guess, you took the jab as well? |
| 88 | 16:44:20 | `@PeachFrog99` | [2084681854041424153](https://x.com/PeachFrog99/status/2084681854041424153) | 0 | @PrinceHeat44402 @jamesob @BikesandBitcoin so imagine that they were pumping this hardware wallet with all the influencers despite being a small low revenue ... |
| 89 | 16:45:43 | `@deepchess_` | [2084682204311941288](https://x.com/deepchess_/status/2084682204311941288) | 0 | @jamesob @BikesandBitcoin Shady as **** isn't it? |
| 90 | 16:47:49 | `@PeachFrog99` | [2084682731800174786](https://x.com/PeachFrog99/status/2084682731800174786) | 0 | @4moonsettler @jamesob @BikesandBitcoin it would be better to stick around for plausible deniability. are odell, natalie, marty others going to be running as... |
| 91 | 16:50:36 | `@PeachFrog99` | [2084683432487985172](https://x.com/PeachFrog99/status/2084683432487985172) | 0 | @4moonsettler @jamesob @BikesandBitcoin i don't know if it's just the conspiracy tendencies of twitter, but i'm starting to get a bad feeling. there is more ... |
| 92 | 16:54:25 | `@btc_technica` | [2084684394191581297](https://x.com/btc_technica/status/2084684394191581297) | 0 | @raw_avocado @jamesob @BikesandBitcoin Which post was this? |

#### Bearing on this log

| Point from thread | Bearing |
| --- | --- |
| Nym as “more eyes” / seriousness theater (darosior #2) | Alternative to “nym = cover-up for theft” (S3 without S6) |
| Real backdoors would be stealthier (darosior #2) | Pressures S6 (intentional plant) |
| Real-name DocHex commits on libngu alongside nym (darosior #1) | Cuts pure burner-cover story; aligns E3–E5 |
| Explicit “not compelling” + wildfire concern (darosior #2–#3) | Strong technical **non-guilt-of-theft** steelman |
| jamesob: no reasonable nym explanation; prior report shrugged | Strong process / opacity indictment; May notice claim |
| jamesob: malice not confirmable from current public evidence | Matches C/D open in claim table |
| jamesob: anon mediating RNG is the red flag that drew audit | Explains *why* nym discourse matters without proving heist |
| udi: secret CTO better than true nobody; dumb malice path | Softens pure “burner = assassin” reading |
| MrHodl: multi-dev optics | Non-theft nym motive (still OPSEC failure) |
| NicerInPerson: sign real key on nym commits is bad plant OPSEC | Aligns with dual-authorship evidence vs careful plant |
| Bikes Hanlon + “heuristic not answer” | Method for S1–S3 preference without closing S5–S6 |
| PrinceHeat + 2021 retirement-attack quote | Reinforces F dual-use, not new fact |
| raw_avocado quoted root: “99.99% they did not know” | Social non-guilt prior; independent of G ticket hunt |
| Flight / influencer / Fed conspiracy side-branches | Discourse noise; no staff↔proceeds artifact |

**Label:** external expert + community observation set. Does not erase A/B
identity facts or process guilt. Directly supports least-strained **S1–S3 over
S6** when the reader is **not sure** about malice, while preserving jamesob’s
red flags and prior-report claim as open process evidence (G / S2).

---


### E29Δ — Same thread + broader discourse after E29 cutoff (2026-08-04 ~16:54Z–21:15Z)

**Probe:** ~4 hours after the E29 inventory end (last prior row ~16:54Z). Full post
list in
[`receipts/2026-08-04-later-discourse-sweep.json`](receipts/2026-08-04-later-discourse-sweep.json)
(**117** posts with `created_at ≥ 16:54Z` across root/QT conversations plus
switck/DocHex/identity queries; **116** not already in the E29 receipt).

**DocHex:** still **no** new personal posts in the window (silence continues).

#### Material additions on the E29 / darosior branch

| Time UTC | Author | Link | Point |
| --- | --- | --- | --- |
| 17:55 | `@OneSirMeow` | [2084699873085931911](https://x.com/OneSirMeow/status/2084699873085931911) | Multi-GH-account “paranoid privacy” nym theory; still “bad execution” |
| 17:59 | `@darosior` | [2084700696369135972](https://x.com/darosior/status/2084700696369135972) | Privacy management that **signs both identities with the same key**? |
| 18:00 | `@OneSirMeow` | [2084700972396253634](https://x.com/OneSirMeow/status/2084700972396253634) | “Yes, bad and clueless execution.” |
| 18:20 | `@adam3us` | [2084706074095910954](https://x.com/adam3us/status/2084706074095910954) | Don’t over-read PGP; senior-dev merge/update workflow can explain signing (E32) |
| 17:50 | `@FlyTheElephant1` | [2084698458640802059](https://x.com/FlyTheElephant1/status/2084698458640802059) | Pushback: why assume negligence if entropy collapse benefits a party with UID list? |

**Use:** darosior continues to steelman non-malice while **tightening** the dual-key
absurdity; Adam Back adds a senior-workflow non-guilt reading that **does not**
erase dual *authorship* under real email (E3–E5) — only weakens pure “GPG = intent”
tea-leaf reading.

Other root-thread replies after cutoff are mostly reaction / conspiracy color
(state attack, “assume malice,” influencer speculation). Inventoried in receipt;
no new cryptographic or ticket primary evidence.

#### High-signal posts outside the Hanlon root (same window)

| Time UTC | Author | Link | Why it matters |
| --- | --- | --- | --- |
| 17:13 | `@inverse_hanlon` | [2084689208627925384](https://x.com/inverse_hanlon/status/2084689208627925384) | Long guilt-leaning timeline essay (E31) |
| 17:28 | `@hodlonaut` | [2084692918506295583](https://x.com/hodlonaut/status/2084692918506295583) | High-engagement identity+libngu confirmation ask (~278 likes) (E34) |
| 17:24 | `@anticonduct` | [2084691958841372699](https://x.com/anticonduct/status/2084691958841372699) | Independent re-verify of GPG subkey link |
| 17:56 | `@darosior` | [2084699942531014843](https://x.com/darosior/status/2084699942531014843) | In-person Coldcard vuln discussion at PubKey |
| 19:48 | `@fionacmurphy` | [2084728222701535248](https://x.com/fionacmurphy/status/2084728222701535248) | Rename + registrar coincidence claims (E33) |
| 20:02 | `@COLDCARDwallet` | [2084731768632991801](https://x.com/COLDCARDwallet/status/2084731768632991801) | Official “public record” article + historical-disclosures (E30) |
| 20:38 | `@asanoha_gold` | [2084740726458556872](https://x.com/asanoha_gold/status/2084740726458556872) | Screenshots of alleged switck posts (dice imagery) as “spine chilling” — social reading of E25-class material |
| 17:05 | `@jamesob` | [2084687252924318090](https://x.com/jamesob/status/2084687252924318090) | Personal: strong passphrase; “mistakenly trusted that they knew” |
| 18:21 | `@jamesob` | [2084706411791954058](https://x.com/jamesob/status/2084706411791954058) | Simulator path uses host `os.random()` (not the device defect) |

**Noise filtered:** generic “$114M drained” reposts, BIP-110 delay solidarity
threads, antisemitic / Epstein / pure-Fed fanfic without new artifacts.

---

### E30 — Official COLDCARD “Adding to the Public Record” + historical-disclosures (2026-08-04)

**Primary X post:**
[2084731768632991801](https://x.com/COLDCARDwallet/status/2084731768632991801)
(~58 likes / ~26k views at probe) — long-form X article.

**Companion page (live):**
[https://coinkite.com/historical-disclosures](https://coinkite.com/historical-disclosures)
— “COLDCARD Security Disclosure History”; probe HTML claims **23**
security-relevant events, **12** with public evidence of coordinated disclosure,
coverage 2019–2026 through 2026-08-03. Explicitly “a chronology, not a count of
independent vulnerabilities or a product score.”

**Article substance (paraphrase + direct claims):**

1. Anger/losses acknowledged; migrate if seed on affected firmware without ≥50
   private dice rolls and without strong unique BIP-39 passphrase.
2. Bug framed as living at a **boundary between two unrelated submodules**, not
   in parent code / Bitcoin-crypto logic most reviews target; flag check “looked
   correct.”
3. **AI-assisted review** of critical codebases, including weeks before exploit,
   **did not catch** the vulnerability; post-incident tests against frontier
   models (**Kimi K3, Claude Fable, Codex 5.6**) also missed it.
4. Ask other teams to test AI review against **build and submodule boundaries**.
5. Historical-disclosures published as definitive public record for researchers.

**Bearing:**

| Claim | Bearing |
| --- | --- |
| Submodule-boundary integration bug | Reinforces S1 composition narrative (aligns E17) |
| AI review miss (self + named models) | Non-guilt / industry-warning steelman; **not** proof no human knew |
| Historical-disclosures page | Process transparency move; does **not** by itself settle prior-notice (G) |
| Still no CTO nym/GPG personal response | Silence continues (H/comms pattern) |

**Label:** official product communication. Preserve as evidence of vendor
framing, not as independent verification of unawareness.

---

### E31 — `@inverse_hanlon` longform guilt-leaning timeline (2026-08-04)

**Primary:** [2084689208627925384](https://x.com/inverse_hanlon/status/2084689208627925384)
(~31 likes; long article body via note/expand). Title energy: inverse Hanlon —
when many “incompetences” stack, incentive is simpler.

**Assembled claims in the essay (verification status in this log):**

| Essay claim | Status in our ledger |
| --- | --- |
| NVK Dec 2020 “retirement attack” / dice reply to Flaxman | Matches E10-class official discourse; exact Flaxman thread should stay pinned to primary URLs |
| Mar 2021 “First pass w/ libNgU” routes seed to software PRNG | **Strong** (E4–E5, E17) — mechanism wording in essay slightly loose (serial/clock vs Yasmarang constants) |
| switck = Gray via GPG census | **Strong** (E3 / jamesob / zherbert) |
| Apr 2021 Telegram warning on libNgU swap | **Social claim** — needs primary Telegram export / archive (new to G lane) |
| May 2025 jamesob report + “we’d already know” | **Social primary from jamesob** (E29 spine); essay elaborates |
| Signal group “LNGU Clean up” (n, Doc, andres…) May 2025; 4-week disappearing msgs; no ship | **Unverified social claim** — high flip value if confirmed (G / S2 / S4) |
| Official “retirement attack” definition Oct 2021 | **Documented** (E10) |
| Dice opt-in as architecture of a retirement attack | **Interpretation** — dual-use with real user-agency design |
| Purchase-data retention contradiction (90 days vs 2019 buyers) | **Not verified here** — track separately |
| Hotfix TRNG latch / #692→#693 | Aligns E21; “more time for attackers” is inference |
| Wave-1 prep argument for insider/precompute | Aligns open claim **D**; still needs staff↔proceeds |

**Use:** best-packaged **public guilt narrative** in this window. Treat as a
**steelman index of allegations**, not as primary evidence for the unverified
rows. Highest new research target from the essay: **primary for “LNGU Clean up”
Signal + Apr 2021 Telegram**.

---

### E32 — `@adam3us` on GPG tea-leaf reading (2026-08-04)

**Primary:** [2084706074095910954](https://x.com/adam3us/status/2084706074095910954)
(reply under darosior / jamesob; ~3 likes at probe):

> agree, let's not get too speculative on tea-leaf reading who pgp signed what.
> simple work-flow explanations abound for @dochex to be signing merge of a 3rd
> party project as their senior dev, or pulling in updates etc.

**Bearing:** non-guilt steelman from a high-reputation cryptographer. **Scope
check against our git facts:**

- Workflow signing of merges/updates can explain *some* Good signatures.
- It does **not** by itself explain **dual authorship** (switck *and*
  `peter@conalgo.com` / DocHex-named commits on the same critical path) or the
  firmware “First pass w/ libNgU” under real email (E4b).
- Still useful: separates **“GPG proves Gray’s key touched the object”** (strong)
  from **“GPG proves heist intent”** (not established).

---

### E33 — Domain registrar match vs GitHub-rename claim (2026-08-04)

**Social source:** `@fionacmurphy`
[2084728222701535248](https://x.com/fionacmurphy/status/2084728222701535248)
on zherbert’s identity thread:

1. “A month before introducing the bug the CTO renamed GitHub from
   `peter-conalgo` to `doc-hex`.”
2. `conalgo.com` and `switck.com` share a small Ontario-based registrar.

**Independent checks (2026-08-04 probe; in later-sweep receipt):**

| Check | Result |
| --- | --- |
| RDAP `conalgo.com` | Registrar **easyDNS Technologies Inc.**; registered 2000-08-18 |
| RDAP `switck.com` | Registrar **easyDNS Technologies Inc.**; registered **2021-01-05** |
| GitHub `doc-hex` | Created **2012-02-28** (long-standing) |
| GitHub `switck` | Created **2020-08-07** |
| GitHub `peter-conalgo` | Created **2021-04-22** — **after** Mar 2021 libNgU ship |

**Verdict:**

- **Registrar coincidence: confirmed** (both easyDNS). Supports dual-identity /
  same-operator **hygiene** reading; not proof of theft. Aligns with E25-era
  switck web presence.
- **“Renamed peter-conalgo → doc-hex a month before the bug”: not supported** by
  public user-creation dates (`doc-hex` predates by years; `peter-conalgo` post-
  dates the defect). Do **not** promote this claim without commit-graph rename
  evidence stronger than user-created timestamps.
- Note: firmware commits use email `peter@conalgo.com` (E4b) — domain link is
  real; username-rename story as told is the weak part.

---

### E34 — hodlonaut identity amplification + independent GPG re-check

**`@hodlonaut`** [2084692918506295583](https://x.com/hodlonaut/status/2084692918506295583)
(~278 likes / ~16k views): asks confirmation that switck image = CTO Peter Gray
and that “library he made himself” is the one that displaced hardware RNG.
Replies include retirement-attack readings and pointers back to zherbert phone-
digit + GPG census (E29 steelman table / prior E rows).

**`@anticonduct`** [2084691958841372699](https://x.com/anticonduct/status/2084691958841372699):
claims independent verification that switck signing key material matches
DocHex encryption subkey — same person.

**Bearing:** spreads **A/B identity** further into mainstream Bitcoin Twitter;
does not add staff↔proceeds. Useful as discourse-temperature / amplification
evidence, not new crypto beyond corroboration of E3.

---


### E35 — jamesob live tripwire dashboard (`cktripwire.com`) (2026-08-04 night)

**Highest-signal new artifact of the overnight window.**

**Primary:** [2084769501661331589](https://x.com/jamesob/status/2084769501661331589)
(~971 likes / ~121k views at probe) + follow-up
[2084769504299487543](https://x.com/jamesob/status/2084769504299487543)
→ **https://cktripwire.com**

**Method (public claim):**

- Deploy tripwire UTXOs on **broken default seeds** with graded added entropy:
  dice-roll ladders (5, 10, 15, …) and passphrase-word ladders (1, 2, 3, …).
- Goal: approximate the **active confiscation frontier** (how far attackers have
  brute-forced partial user mitigations).
- **Control** UTXO with **no** added entropy was **swept within about one hour**.
- Calibrated “good bad entropy” emulator from **real Mk3 measurements**; control
  confiscation used as proof the emulator matches attacker tooling.
- Credits `@Rob1Ham` `@otaliptus` `@PortlandHODL` and other redteamers.

**Bearing:**

| Point | Bearing |
| --- | --- |
| Control swept in ~1h | Attack is **still live**; pure device-default keyspace is trivial for current operators |
| Graded dice/passphrase tripwires | Maps **how much extra entropy** is currently economic for attackers — feeds on-chain plan / claim D timing |
| Independent redteam dashboard | Third-party measurement lane, not Coinkite messaging |
| Does **not** identify operators | Still no staff↔proceeds; strengthens **ongoing external (or any) exploitation** observation |

**Use:** add `cktripwire.com` as a standing observation source for Lane 1
(on-chain). Re-check frontier movement over time; do not treat a single control
sweep as proof of insider knowledge.

`@darosior` RTs the dashboard post (amplification only).

---

### E36 — `@zherbert` resurfaces DocHex 2020 “rogue QR lib” thought experiment

**Primary:** [2084830882968330259](https://x.com/zherbert/status/2084830882968330259)
(“No comment”, ~31 likes) **quotes** `@DocHex`
[1288894727866245120](https://x.com/DocHex/status/1288894727866245120)
(2020-07-30):

> QR Code library is actually a great example! My rogue QR lib would (sometimes)
> grind a little and present a bogus payment address that matches first and last
> few chars of what's expected. Very few victims would notice that.

**Same night cluster:**

| Post | Point |
| --- | --- |
| [2084843982052340101](https://x.com/zherbert/status/2084843982052340101) | “Unplugged the RNG” (reply context) |
| [2084800982643302678](https://x.com/zherbert/status/2084800982643302678) | Matrix / “clues about Switck, Cyber, and the white rabbit” (~64 likes) |
| [2084836189681397893](https://x.com/zherbert/status/2084836189681397893) | Nym motive maybe “for fun”; points at switck “itch scratched” on merge |
| [2084838524113551567](https://x.com/zherbert/status/2084838524113551567) | Quotes switck “itch scratched” (2021-01-14) |
| [2084751726704226455](https://x.com/zherbert/status/2084751726704226455) | Confirms to hodlonaut on identity ask |

**Bearing:** **cultural / adversarial-thinking color**, not a cryptographic link
to 2026 drains. Steelman guilt reading: CTO previously daydreamed stealthy
payment-address tricks. Steelman non-guilt: normal security-engineer threat
modeling years before the entropy bug. Label **dual-use discourse**; do not
promote to intent proof without more.

---

### E37 — Official product line: personal silence; migrate; AI review reiteration

**`@COLDCARDwallet` overnight pattern:** mostly support/migration replies under
the public-record article (E30). Material policy line:

[2084799249582014629](https://x.com/COLDCARDwallet/status/2084799249582014629)
(~7 likes):

> The team is heads down helping ppl migrate, there is no value in their
> personal comments at this moment. NVK issued a statement on his account.

[2084800653143003206](https://x.com/COLDCARDwallet/status/2084800653143003206):
reiterates latest AI models + third-party AI review; points back to E30 article;
notes Kimi K3 “without safety” released days before the attack.

**`@DocHex`:** still **zero** original posts in the 21:00Z–06:05Z window
(search `from:DocHex` empty). CTO personal silence continues while product
account runs migration ops.

**`@nvk`:** RTs of news/how-to content; no new technical rebuttal of nym/GPG
claims in this window’s sample.

**Bearing:** consistent with damage-control ops + intentional non-engagement on
identity discourse. Dual-use (focus on victims vs avoid answering A/B facts).

---

### E38 — Non-guilt / de-escalation color (overnight)

| Voice | Link | Point |
| --- | --- | --- |
| `@stack2thefuture` | [2084760618322526298](https://x.com/stack2thefuture/status/2084760618322526298) | Toronto Star **Nov 2013** shows Peter Gray as public Coinkite figure early; may have grown private — not proof of flight plan. [Star article](https://www.thestar.com/news/gta/bitcoin-entrepreneurs-want-to-put-virtual-coins-in-your-wallet/article_ec1cf783-5cf5-585e-a6d3-71615e1102f4.html) |
| `@IfindCoretards` | [2084750261055037705](https://x.com/IfindCoretards/status/2084750261055037705) | Not inside job; nym+migration “bizarre” / “catastrophically sloppy”; **not malicious, just unserious** |
| `@PraveenPerera` (on zherbert thread) | [2084835187062341682](https://x.com/PraveenPerera/status/2084835187062341682) | If premeditated malice, why so obvious? |

Aligns S1–S3 preference; does not erase G/process indictment.

---

### E39 — Pushback on “submodule boundary / AI miss” framing + phishing note

| Voice | Link | Point |
| --- | --- | --- |
| `@janrothen` | [2084770248327123333](https://x.com/janrothen/status/2084770248327123333) | Aircraft metaphor: interface/seam is **first** audit target, not edge case |
| `@AndrewBTC` | [2084769067211391023](https://x.com/AndrewBTC/status/2084769067211391023) | “Test submodule boundaries” advice from Coinkite reads as 🚩; stop lecturing industry, help victims |
| `@mattcrv` | [2084773088009294057](https://x.com/mattcrv/status/2084773088009294057) | Coinkite not in position to recommend practices to other teams |

**Phishing:** impersonator handle `@Coldcardwalleet` posting “send support a DM”
under real COLDCARD threads (multiple ~05:00Z Aug 5). Ops note only — users
should use documented support email, not random DMs.

**Backdoor-as-code discourse** (`@FlyTheElephant1` etc. on XOR / low-variance
streams) continues without new primary tickets or proceeds links — already
covered by E17/E31 interpretation lanes; no status flip.

**Noise filtered:** identity re-spreads, bigotry against switck presentation,
“fleeing to Israel” fanfic, pure RT volume.

---

# Part III — Steelman readings

## Steelman: readings that pull toward **guilt / bad faith**

These are **strongest fair constructions** of the “something darker than oops”
side. They are **not verdicts**. Steelman means: give the case its best form,
then still separate fact from leap.

### Steelman posts / voices (guilt-leaning)

| Voice | Link | Steelman point |
| --- | --- | --- |
| `@jamesob` | [2084640418918957430](https://x.com/jamesob/status/2084640418918957430) | Public nym for security-critical code “has no reasonable explanation”; prior defect report “shrugged off” (E29 spine #2) |
| `@jamesob` | [2084666514439848413](https://x.com/jamesob/status/2084666514439848413) | Anon mediating RNG is huge red flag; seriousness-theater nym theory does not explain reworking RNG (E29 #13) |
| `@jamesob` | [2084620229389197453](https://x.com/jamesob/status/2084620229389197453) | switck ≈ DocHex; same person dismissed prior report |
| `@PrinceHeat44402` | [2084648769459482693](https://x.com/PrinceHeat44402/status/2084648769459482693) | 2021 retirement-attack official post re-read as “admission” if you understand the mistakes (E10 dual-use; E29 #8) |
| `@inverse_hanlon` | [2084689208627925384](https://x.com/inverse_hanlon/status/2084689208627925384) | Full inverse-Hanlon timeline essay: dice architecture, dual warnings, alleged “LNGU Clean up” Signal, AI-blame vs prior report (E31) |
| `@hodlonaut` | [2084692918506295583](https://x.com/hodlonaut/status/2084692918506295583) | High-amplification switck=Gray + libngu ask (~278 likes) (E34) |
| `@FlyTheElephant1` | [2084698458640802059](https://x.com/FlyTheElephant1/status/2084698458640802059) | Why assume negligence if weak entropy benefits a party with UID list? (E29Δ) |
| `@zherbert` | [2084830882968330259](https://x.com/zherbert/status/2084830882968330259) | Resurfaces DocHex 2020 “rogue QR lib” payment-address trick thought experiment (E36; dual-use) |
| `@jamesob` (tripwire) | [2084769501661331589](https://x.com/jamesob/status/2084769501661331589) | Live attack mapping: control seed swept in ~1h (E35) — not guilt of staff, but live exploitation truth |
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
| `@darosior` (nym / backdoor) | [2084661793624924500](https://x.com/darosior/status/2084661793624924500) | Nym can mean “portray more eyes”; real backdoors would be less conspicuous; **“The backdoor theory is not compelling at this point”** (E29 #12) |
| `@darosior` (quoted prior) | [2084659808519463034](https://x.com/darosior/status/2084659808519463034) | Gray also committed under DocHex to libngu; nym alone “doesn’t seem to be hinting at a cover up”; **extreme negligence > malice** (E29 #11) |
| `@darosior` (reply to jamesob) | [2084668646924656893](https://x.com/darosior/status/2084668646924656893) | Not “pushing” non-backdoor; fears wildfire conspiracy; agrees red flags but opacity was already visible to anyone who looked (E29 #14) |
| `@jamesob` (malice bar) | [2084646588211667297](https://x.com/jamesob/status/2084646588211667297) | Confirming malice “not possible short of more evidence than we have now” (E29 #4) |
| `@udiWertheimer` | [2084646756592082989](https://x.com/udiWertheimer/status/2084646756592082989) / [2084647846628794747](https://x.com/udiWertheimer/status/2084647846628794747) | Secret CTO better than true nobody; malicious nym plant would still be a “very dumb way” (E29 #6) |
| `@MrHodl` | [2084646338780635269](https://x.com/MrHodl/status/2084646338780635269) | Nym more likely multi-dev theater than heist flight plan (E29 #5) |
| `@NicerInPerson` | [2084678159018967069](https://x.com/NicerInPerson/status/2084678159018967069) | Plant under nym while signing real identity is self-defeating OPSEC (E29 #16) |
| `@BikesandBitcoin` | [2084630840982831568](https://x.com/BikesandBitcoin/status/2084630840982831568) | Hanlon razor root of the thread; razor is heuristic not answer (E29 #1, #17) |
| `@raw_avocado` (quoted root) | [2084626833182458302](https://x.com/raw_avocado/status/2084626833182458302) | “99.99% chance they did not know”; stop inflating for likes |
| `@adam3us` | [2084706074095910954](https://x.com/adam3us/status/2084706074095910954) | Don’t tea-leaf PGP; senior merge/update workflow can explain signing (E32; limited vs dual authorship) |
| `@darosior` (same-key dig) | [2084700696369135972](https://x.com/darosior/status/2084700696369135972) | Still non-malice, but multi-account privacy that reuses one signing key is absurd (E29Δ) |
| Official COLDCARD article | [2084731768632991801](https://x.com/COLDCARDwallet/status/2084731768632991801) | Submodule-boundary bug; AI reviews missed; historical-disclosures page (E30) |
| `@stack2thefuture` | [2084760618322526298](https://x.com/stack2thefuture/status/2084760618322526298) | Gray was public in 2013 Toronto Star; privacy growth ≠ flight plan (E38) |
| `@IfindCoretards` | [2084750261055037705](https://x.com/IfindCoretards/status/2084750261055037705) | “Not malicious, just unserious” (E38) |
| Official product silence line | [2084799249582014629](https://x.com/COLDCARDwallet/status/2084799249582014629) | Team heads-down on migration; personal comments deferred (E37) |
| `@zherbert` (even while identity-hunting) | [2084638910970220754](https://x.com/zherbert/status/2084638910970220754) | “Still think it is likely just **incompetence / negligence**” unless identity raises flags |
| `@CyberTruckRonin` | [2084661432960880793](https://x.com/CyberTruckRonin/status/2084661432960880793) | Identity facts can be true without “evil maid” / intentional plant framing |
| Dice / multisig survivors | many; e.g. diceroll reports | Mitigation paths worked as designed for users who BYOE |
| Inventory destroy + hotfixes | official posts | Consistent with damage control, not only with cover-up |

### Best-form non-guilt narrative (speculative assembly)

1. **Composition bug is a classic integration failure:** same symbol names,
   `#ifndef` vs value, submodule PRNG — exactly the sort of thing that survives
   partial review (E17 “binary had TRNG, wrong `rng_get` linked”).
2. **Nym use can be privacy / old habit / side project / “more eyes” optics**
   without theft intent (darosior E29 #11–#14); still bad OPSEC, but not a completed
   crime theory. Gray also commits under **real name** on the same repos
   (hurts “pure burner” / cover-up-only story).
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
- [ ] Watch **https://cktripwire.com** frontier movement (E35; jamesob tripwires)

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
| Primary for inverse_hanlon Apr 2021 Telegram warning | **Open** (E31) |
| Primary for “LNGU Clean up” Signal group claim | **Open** (E31 / G) |
| easyDNS registrar match conalgo.com ↔ switck.com | **Done** E33 |
| GitHub rename peter-conalgo→doc-hex pre-bug story | **Refuted** as stated (E33) |
| Official historical-disclosures page snapshot | **Done** E30 (live page + X article) |
| jamesob cktripwire.com live frontier dashboard | **Standing watch** E35 |
| DocHex personal public response on nym/GPG | **Still none** through 2026-08-05 06:05Z (E37) |
| Phishing impersonators on COLDCARD threads | **Noted** E39 (`@Coldcardwalleet`) |

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
