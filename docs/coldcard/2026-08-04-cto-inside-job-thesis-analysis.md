# Coinkite CTO / “inside job” thesis — social claims vs what holds

Status: **external-observation map + independent technical cross-check.**  
Not a legal finding, not a public accusation product claim, and not accepted
forensic proof of theft participation. Written 2026-08-04 after a live X API
pass and re-check of public Git / GPG material.

## Thesis under review

**Claim (morning discourse, 2026-08-04):** the Coldcard entropy failure and the
ongoing fund drains are not mere negligence — they involve an **inside job**,
**at minimum the Coinkite CTO** (Peter Gray, X `@DocHex`), including the idea
that the defect was authored under a GitHub nym (`switck`) that is actually
Gray, and that company remediation behavior is consistent with complicity.

This document splits that into **testable sub-claims**, links the main posts,
and marks each **substantiated / partially substantiated / not substantiated /
refuted as stated**.

---

## Morning social record (links)

### Amplification from `@AtlantisPleb` (owner account; morning of 2026-08-04)

| Kind | UTC | Link | Content (summary) |
| --- | --- | --- | --- |
| Repost | 15:52 | [status/2084668869881323543](https://x.com/AtlantisPleb/status/2084668869881323543) | RT `@_PyBlock_`: “Who is Peter/@Switck/@DocHex following?” |
| Quote | 15:28 | [status/2084662822512873785](https://x.com/AtlantisPleb/status/2084662822512873785) | Flags “Could be useful on Coldcard someday” with red-flag emoji |
| Repost | 15:28 | [status/2084662675057877310](https://x.com/AtlantisPleb/status/2084662675057877310) | RT `@_PyBlock_`: “Switck aka Peter” |
| Repost | 15:27 | [status/2084662595177337212](https://x.com/AtlantisPleb/status/2084662595177337212) | RT `@hodlonaut` on CTO authorship + messaging after attack |
| Quote | 15:20 | [status/2084660832596570417](https://x.com/AtlantisPleb/status/2084660832596570417) | “not absurd … if you / your team are **complicit in the ongoing theft**”; “Evidence is pointing to the Coinkite CTO”; “who else on the team knew” |
| Repost | 15:18 | [status/2084660264499020015](https://x.com/AtlantisPleb/status/2084660264499020015) | RT `@zherbert` DocHex ≡ switck verification |
| Reply | 15:44 | [status/2084666853419360648](https://x.com/AtlantisPleb/status/2084666853419360648) | “Or preparing a theft” |
| Quote | 13:33 | [status/2084633753817592252](https://x.com/AtlantisPleb/status/2084633753817592252) | **Self-tagged** `<speculation>` about 2021 “retirement attack” tweet / heist probe |
| Repost | 14:33 | [status/2084648950196277698](https://x.com/AtlantisPleb/status/2084648950196277698) | RT `@jamesob` May 2025 audit / libngu concerns |
| Repost | 14:32 | [status/2084648659891749182](https://x.com/AtlantisPleb/status/2084648659891749182) | RT `@jamesob`: CTO committing under a pseudonym “has no reasonable explanation” |

### Source posts (not only RTs)

| Author | UTC | Link | Role in thesis |
| --- | --- | --- | --- |
| `@_PyBlock_` | 15:20 | [status/2084660826271531386](https://x.com/_PyBlock_/status/2084660826271531386) | Follow-graph collage for Peter / Switck / DocHex |
| `@_PyBlock_` | 14:17 | [status/2084644806890782737](https://x.com/_PyBlock_/status/2084644806890782737) | “Switck aka Peter” (image) |
| `@zherbert` | 14:29 | [status/2084647957526167853](https://x.com/zherbert/status/2084647957526167853) | Phone-last-digits claim + cites jamesob GPG work; “conclusive” identity |
| `@jamesob` | 12:39 | [status/2084620229389197453](https://x.com/jamesob/status/2084620229389197453) | Strong possibility switck = Peter Gray / `@DocHex` |
| `@jamesob` | 12:56 | [status/2084624605969350915](https://x.com/jamesob/status/2084624605969350915) | May 2025 report to Coinkite about RNG / libngu; response “we'd already know” |
| `@jamesob` | 13:59 | [status/2084640418918957430](https://x.com/jamesob/status/2084640418918957430) | Pseudonym unexplained; “High potential that something isn't right” |
| `@jamesob` | 13:36 | [status/2084634518883872942](https://x.com/jamesob/status/2084634518883872942) | “based on gpg sigs” |
| `@hodlonaut` | 13:32 | [status/2084633601845363006](https://x.com/hodlonaut/status/2084633601845363006) | “Bug seemingly written by” CTO; criticizes post-attack messaging |
| `@hodlonaut` | 13:39 | [status/2084635363192996287](https://x.com/hodlonaut/status/2084635363192996287) | Satirical CTO / Yasmarang line |
| `@hodlonaut` | Aug 2 | [status/2083888379465273754](https://x.com/hodlonaut/status/2083888379465273754) | Amplifies bitcoin++ “disabled HW RNG to silence compiler” narrative |
| `@hodlonaut` | Aug 3 | [status/2084217381165940812](https://x.com/hodlonaut/status/2084217381165940812) | Conditional theory: long-known bug + job vacancy timing |
| `@COLDCARDwallet` | 11:07 | [status/2084596971268956161](https://x.com/COLDCARDwallet/status/2084596971268956161) | Official: migrate / new seed / move funds; threat ongoing |
| `@COLDCARDwallet` | 13:29 | [status/2084632863756955661](https://x.com/COLDCARDwallet/status/2084632863756955661) | Official: firmware does not permanently brick; power-cycle TRNG fault |

---

## Sub-claims

### A. Peter Gray (`@DocHex` / GitHub `doc-hex`) authored the critical libngu RNG code under the GitHub nym `switck`

**Verdict: substantially substantiated as identity of the signing key, with residual process caveats.**

Independent checks (this session):

1. Public defect commit in libngu:
   [`switck/libngu@f19de052`](https://github.com/switck/libngu/commit/f19de0527a49e560203102288ae4bc9740a32d96)
   (2021-01-28, message `x`, author name “Switck”).
2. That commit carries an OpenPGP signature. GitHub UI reports
   `verification.verified = false` / `unknown_key` **on the switck account**
   (expected if the key is not registered to `switck`).
3. Decode the signature’s embedded fingerprint →  
   `D9766C79E77B0198D66975BDF0E6CC6AFC16CF7B`.  
   Long key id (last 64 bits) → **`F0E6CC6AFC16CF7B`**.
4. GitHub user [`doc-hex`](https://github.com/doc-hex) exposes GPG keys via
   API. Primary key uid material identifies **Peter D. Gray**
   (`peter@conalgo.com`). **Signing subkey `key_id` is exactly
   `F0E6CC6AFC16CF7B`.**

So: the critical 2021 libngu commit is signed with a subkey that is **byte-identical**
to the signing subkey published on the `doc-hex` GitHub account under Peter Gray’s
name. That is strong public-key evidence that **Gray’s key material signed the
switck-attributed defect commit**, which is what the morning GPG discourse is
pointing at.

Caveats (do not skip):

- This shows **key control / signing**, not a court-grade proof of sole human
  operator every day of the nym’s life.
- Phone-last-two-digits overlap cited by `@zherbert` is **weak alone** (he
  himself quotes ~1% chance for two digits); treat as secondary color only.
- Follow-graph collages (`@_PyBlock_`) are **not identity proofs**.

### B. The CTO therefore “wrote the Coldcard bug”

**Verdict: partially substantiated, if A holds — for libngu side of the
composition; not the entire multi-repo story alone.**

Repository technical chronology already in-tree:

- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md) — seed path moved to
  `random.bytes()` → libngu → wrong `rng_get` selection / weak PRNG.
- [`2026-08-01-bitcoin-plus-plus-oped-analysis.md`](2026-08-01-bitcoin-plus-plus-oped-analysis.md)
  — `f19de052` existence-only `#ifndef` guard; Coldcard `b18723dd` migration;
  Mk4 port later.
- [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
  — no PR / message `x` / zero reviewers on related commits (process failure).

If A is accepted, Gray signed the libngu change that made the bad RNG adapter
real. Coldcard firmware still had to **import** that library and **point seed
creation** at it (`b18723dd` etc.). Authorship of the full incident is
**cross-repo composition**, not a single line of “CTO flipped one switch.”

### C. The defect was planted **on purpose** as a backdoor for later theft

**Verdict: not substantiated. Several popular “intentional disable” stories are
over-specific or contradicted by commit history.**

What **does** support seriousness / process failure:

- Security-critical commits with empty subjects (`x`, `runs`).
- Missing review (Kelbie / Block-derived commit table).
- Years of exposure before mass exploitation.
- May 2025 external concern (`@jamesob`) reportedly dismissed.

What **does not** prove intentional theft design:

1. **2018 board macro already set `MICROPY_HW_ENABLE_RNG` to 0** with an
   explanatory comment about Coldcard’s own RNG path
   ([bitcoin++ op-ed review](2026-08-01-bitcoin-plus-plus-oped-analysis.md)).
   The viral “he zeroed the macro in 2021 to silence a compiler error” story
   (widely RTed, including by `@hodlonaut` quoting Dusty Daemon) is **not a
   complete causal history** for that macro; our source review already says so.
2. The failure mode is a **composition bug** (existence-test `#ifndef`, missing
   `rng_get` export, MicroPython fallback, seed API migration). That pattern is
   classic **integration negligence**. Intentional backdoors more often look
   like deliberate PRNG seeds, hidden constants only the author knows, or dual
   code paths gated on secrets — none of which is established here as the
   primary mechanism.
3. No public, independently verified link in this repository (or in the morning
   posts we sampled) from **Gray’s identity → control of drain / collector
   addresses**. On-chain forensics in
   [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
   track attacker graphs without naming Coinkite staff as owners.

`@zherbert` himself, while pushing identity verification, still wrote that he
thinks it is **likely incompetence / negligence** unless identity proof raises
further red flags ([status/2084638910970220754](https://x.com/zherbert/status/2084638910970220754)).
That is the correct split: **identity ≠ intent**.

### D. Company remediation (“upgrade, new seed, move funds”) is evidence of
**complicity in ongoing theft**

**Verdict: not substantiated as stated; ambiguous at best.**

- The same advice is the **standard** remediation for weak-seed wallets: stop
  using the broken seed, generate a new one, move value. Official account:
  [status/2084596971268956161](https://x.com/COLDCARDwallet/status/2084596971268956161).
- `@AtlantisPleb` quote
  [status/2084660832596570417](https://x.com/AtlantisPleb/status/2084660832596570417)
  argues that advice is “logical if complicit.” That is a **motive hypothesis**,
  not a discriminator: a non-complicit vendor under active exploitation gives
  the same instruction.
- Critiques of messaging quality, humility, deleted posts, or brick-claim
  corrections ([status/2084632863756955661](https://x.com/COLDCARDwallet/status/2084632863756955661))
  can support **crisis-comms failure** without proving participation in theft.

### E. Long-known bug + wait-then-drain + job vacancy timing

**Verdict: speculative.** `@hodlonaut`
[status/2084217381165940812](https://x.com/hodlonaut/status/2084217381165940812)
explicitly double-`IF`s the theory. Timing coincidences need independent
attacker-knowledge evidence (when the PRNG space was first enumerated at
scale, who held the tooling, etc.). Not established in the linked morning
posts.

### F. 2021 “retirement attack” tweet as CTO floating a heist to nvk

**Verdict: pure speculation** — author’s own
[status/2084633753817592252](https://x.com/AtlantisPleb/status/2084633753817592252)
is wrapped in `<speculation>`. Do not promote as evidence.

---

## Synthesis (what to keep vs drop)

| Layer | Keep? | Note |
| --- | --- | --- |
| Gray’s GPG subkey signed `switck` libngu defect commit | **Yes** | Re-checkable; see §A |
| libngu + Coldcard integration caused weak seeds | **Yes** | Existing technical docs |
| Catastrophic process / review failure | **Yes** | Existing Kelbie / op-ed reviews |
| May 2025 concern allegedly shrugged off | **Social claim** | Strong narrative; needs Coinkite-side records for full confirmation |
| CTO nym use “unexplained” / looks bad | **Judgment** | Fair as reputation critique if A holds; still not theft proof |
| Intentional backdoor planted for later theft | **No proof** | Do not treat as established |
| “Inside job at minimum the CTO” as **theft complicity** | **Not established** | Identity + negligence ⇏ participation in drains |
| Migration advice as proof of complicity | **No** | Compatible with both guilt and innocence |

**Short form:** Morning discourse correctly elevates a **serious identity and
governance problem** (CTO-linked key signing the nym repo that shipped the RNG
adapter; years of silent exposure; weak review). It **overreaches** when it
collapses that into **proven intentional inside theft** or treats standard
migration guidance as a confession.

---

## Reproduce the GPG subkey check

```sh
# 1) Pull commit signature metadata (API)
curl -sS \
  "https://api.github.com/repos/switck/libngu/commits/f19de0527a49e560203102288ae4bc9740a32d96" \
  | jq '.commit.verification'

# 2) Decode fingerprint from the signature block (FiEE… OpenPGP notation)
#    Expected long key id: F0E6CC6AFC16CF7B

# 3) Compare to doc-hex GitHub GPG keys
curl -sS "https://api.github.com/users/doc-hex/gpg_keys" \
  | jq '.[].subkeys[].key_id'
# Expect F0E6CC6AFC16CF7B among signing subkeys
```

Full offline verify would import the `doc-hex` public key and run `gpg --verify`
on the commit object; the key-id match above is the fast public cross-check used
for this note.

---

## Operating rules for OpenAgents surfaces

1. Label social posts **external observation**.
2. Do **not** turn “CTO key signed switck commits” into “CTO stole the coins”
   without on-chain attribution receipts.
3. Prefer linking technical postmortems and identity-check methods over rage-RT
   chains when product UI summarizes the incident.
4. Continue forensic work under the evidence ladder in
   [`2026-08-01-omega-coldcard-forensic-practice-runbook.md`](2026-08-01-omega-coldcard-forensic-practice-runbook.md).

## Related

- [`2026-08-04-x-posts-recent-summary.md`](2026-08-04-x-posts-recent-summary.md)
- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md)
- [`2026-08-01-bitcoin-plus-plus-oped-analysis.md`](2026-08-01-bitcoin-plus-plus-oped-analysis.md)
- [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
