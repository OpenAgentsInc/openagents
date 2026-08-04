# Coinkite CTO / “inside job” thesis — evidence collection

Status: **living evidence log** (social + independent technical re-checks).  
This is **not** a claim that theft-complicity is proven. It is a structured place
to park what is observed, what was re-verified, and what remains open.

Last updated: 2026-08-04 (second pass: local GPG verify + commit census +
firmware authorship).

## How to read this

- **Evidence** = something a third party can re-fetch (URL, commit SHA, GPG
  output shape).
- **Inference** = interpretation that still needs more support.
- **Open** = not yet checked or not checkable from public data here.

No layer below is a courtroom verdict. Collecting “CTO involvement” evidence
does not by itself establish “CTO stole coins.”

---

## Thesis under collection

Working statement from morning discourse (2026-08-04):

> The Coldcard entropy failure and ongoing drains involve more than simple
> oops — **at minimum the Coinkite CTO** (Peter Gray / X `@DocHex` / GitHub
> `doc-hex`) is central: the critical code was written or signed under a
> GitHub nym (`switck`), process was opaque, and company behavior after the
> drains is consistent with (or at least not falsifying) deeper involvement.

Split for collection:

| ID | Sub-claim | Status after this pass |
| --- | --- | --- |
| **A** | Gray’s personal OpenPGP key signed `switck`-attributed libngu commits | **Strong evidence (re-verified locally)** |
| **B** | Gray authored / integrated the security-critical seed path (libngu + firmware) | **Strong evidence (dual authorship + real-name firmware commit)** |
| **C** | Defect was **intentionally** planted as a long-term theft backdoor | **Open / not established** |
| **D** | Gray or Coinkite staff **participated in or directed** the 2026 drains | **Open / not established** |
| **E** | Remediation messaging is a cover for ongoing theft | **Ambiguous** (compatible with innocence and guilt) |
| **F** | 2021 “retirement attack” tweet = heist dry-run with nvk | **Speculation only** (author-labeled) |

---

## Evidence log (append-only style)

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

---

## Synthesis (evidence, not verdict)

### What the evidence **does** support collecting as high confidence

1. **Cryptographic identity bridge:** Peter D. Gray’s personal OpenPGP key
   produces **Good signatures** on the critical libngu commit and on **dozens**
   of other commits whose author field is the `switck` nym (E3).
2. **Dual identity on one repo:** the same person commits as **Switck** and as
   **Peter D. Gray `<peter@conalgo.com>`** on `switck/libngu` (E3–E5).
3. **Real-name firmware responsibility:** the Coldcard “First pass w/ libNgU”
   migration that wired seed generation into libNgU is authored by
   **Peter D. Gray** (E4b).
4. **Nym surface is narrow and Coldcard-adjacent** (E2).
5. **Process failure** around review and commit hygiene is independently
   documented (E7, prior postmortems).

These points make “CTO is central to how the defective code entered the
product” a **well-supported collection target**, not a vibes-only RT.

### What the evidence does **not** yet support

1. **Intentional backdoor for later theft (C).** Composition bugs, empty
   subjects, and dual identity are consistent with negligence, haste, and
   secrecy habits. They are not a completed intent proof.
2. **Participation in the 2026 drains (D).** No collector-key attribution to
   named staff is in this file.
3. **Remediation-as-confession (E).** “Generate new seed and move funds” is
   also what a non-malicious vendor must say under active exploitation.
4. **Narrative overreach** in viral posts that depend on an incorrect full
   causal story for the 2018 RNG macro (E7).

### Working stance for further collection

Keep gathering artifacts that could move C or D (or falsify them): prior
notice records, internal issue trackers if they become public, factory
firmware provenance, chain analysis of drains, and any authentic staff
statements that address the GPG/nym facts **directly**.

Until then, the honest summary is:

> **Strong public evidence that Coinkite CTO Peter Gray controlled the signing
> key and authorship path for the defective libngu/firmware seed stack,
> including work under the switck nym. Open whether that implies intentional
> theft design or participation in the 2026 drains.**

---

## Reproduce checklist

```sh
# A) GPG good-sig on defect commit
curl -sS https://api.github.com/users/doc-hex/gpg_keys | jq -r '.[0].raw_key' > /tmp/doc-hex.asc
export GNUPGHOME=$(mktemp -d) && gpg --import /tmp/doc-hex.asc
git clone https://github.com/switck/libngu.git && cd libngu
git verify-commit f19de0527a49e560203102288ae4bc9740a32d96

# B) Firmware migration author
git clone --filter=blob:none --no-checkout https://github.com/Coldcard/firmware.git
cd firmware && git fetch --depth=1 origin b18723dddb6d751c39978e4364b56b2414f68b47
git log -1 --format=fuller FETCH_HEAD

# C) Social map (requires X_BEARER_TOKEN)
cargo run -p x_api -- search 'from:_PyBlock_ (DocHex OR Switck)' --max-results 20
cargo run -p x_api -- post 2084660832596570417
```

---

## Related OpenAgents docs

- [`2026-08-04-x-posts-recent-summary.md`](2026-08-04-x-posts-recent-summary.md)
- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md)
- [`2026-08-01-bitcoin-plus-plus-oped-analysis.md`](2026-08-01-bitcoin-plus-plus-oped-analysis.md)
- [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
- X access tooling: [`../grok/2026-08-04-x-api-and-xai-x-search-access.md`](../grok/2026-08-04-x-api-and-xai-x-search-access.md)
