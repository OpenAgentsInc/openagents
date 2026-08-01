# A third independent account of the Coldcard RNG failure

Status: **analysis of published third-party material.** Not a disclosure, not a
new vulnerability claim, and not authority for any surface. Every defect
statement here is already public in the sources cited.

Subject: `Kelbie/coldcard-rng-postmortem`, read at commit `47d8f554`
(2026-08-01 06:20 +0100). Authored by Kevin Kelbie (`kevin@kelbie.me`),
~40 commits between 2026-07-31 17:39 and 2026-08-01 06:20. Local reference
clone: `~/work/projects/repos/coldcard-rng-postmortem`, synced read-only under
the `projects/` convention. **The repository carries no LICENSE file**, so it
is quotable as published material but not vendorable — which is the posture we
wanted anyway.

Reading order:

- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md) — the technical anatomy
  of the defect.
- [`../loupe/2026-08-01-would-loupe-have-caught-coldcard.md`](../loupe/2026-08-01-would-loupe-have-caught-coldcard.md)
  — our pre-registered prediction that a per-file scanner would miss it.
- [`../loupe/2026-07-31-omega-first-class-pentester-speculation.md`](../loupe/2026-07-31-omega-first-class-pentester-speculation.md)
  — §5, the evidence ladder applied below.

---

## 1. The headline correction: this is not a reproduction

The obvious guess about a third postmortem is that it re-derives the broken
PRNG and regenerates seeds. **It does not, and it does not claim to.**

Verified by exhaustive search of the repository, not inferred:

- No Yasmarang implementation, no BIP-39, no PBKDF2, no HMAC, no secp256k1, no
  key derivation, no state enumeration, no brute-force loop anywhere in
  `src/`, `scripts/`, or `tools/`.
- `package.json` dependencies are exactly `react`, `react-dom`, `tailwindcss`,
  `bun-plugin-tailwind`. There is no cryptographic dependency to hide behind.
- The one executable tool, `tools/coldcard-node-scan.py`, imports only
  `argparse`, `base64`, `http.client`, `json`, `os`, `socket`, `sys`, `time`,
  `collections`. It is a Bitcoin Core JSON-RPC client and nothing else.

The only mentions of seeds and mnemonics in the source are UI strings and one
comment explaining why a screenshot is *not* republished.

What the repository actually is: **an on-chain forensic instrument** — a
derivation pipeline that turns a small set of human testimony into the full
attacker address set, the victim ledger, and the money flow, plus a static site
that renders the result and archives the social record. It is the third
independent account not because it re-derives the *bug* but because it
re-derives the *event*.

That distinction is the whole value of the thing, and it is worth stating in
the terms the task framed: the repository is a **demonstration of impact**, not
an argument about feasibility. It never touches feasibility. Feasibility was
settled by someone else, and the repository is careful to say so — see §4.

---

## 2. What it proves, and how

The architecture is the argument. `src/data/evidence.ts` is declared the only
hand-maintained file, and it holds evidence rather than conclusions: **two
victim reports, five published addresses, one self-derived scan match.** From
those eight seeds, `scripts/lib/discover.ts` walks the chain:

```
victim report  →  the transaction they name  →  its destination is a collector
collector      →  everything paying into it  →  those payers are more victims
collector      →  where it spends onward     →  that destination is a vault
```

Everything downstream — 24 attacker addresses, 2,348 victim addresses, six
waves, three episodes, every headline number — is generated. `README.md:76-78`
states the design rule plainly: *"It holds evidence, not conclusions — there is
no list of attacker addresses anywhere in the project."*

Four things this actually establishes, each re-runnable by a third party:

**1. The derivation is falsifiable and self-invalidating.** The justification
sentence shown for each address is emitted by the same code that found the
address, so the two cannot drift apart. Wave grouping is a connected component
of the pooling graph; episode grouping is a one-hour quiet threshold that the
README defends as untuned (the longest gap inside the 30 July burst is 22
minutes, the next sweeping is over a day later, so anything between gives the
same answer). If a vault ever spends, `exfiltratedSats` goes non-zero and every
"nothing has moved" sentence rewrites itself.

**2. It independently reconciles two other parties' published figures.**
`attribution.reconciliation` compares against Block's and an independent
analyst's numbers at the precision each published them, and flags `DRIFT`
rather than quietly disagreeing. All three match: Block's two waves to the
satoshi (204 tx / 298 UTXOs / 89.62370091 BTC and 491 / 728 / 398.48587857),
and the widely-cited 500-transaction wave to two decimal places (594.48
published, 594.47722484 derived). This is the strongest evidence in the
repository: a third party rebuilding the first two accounts from raw chain data
and landing on the same satoshis.

**3. An original forensic contribution — the fee-estimator fingerprint.** Every
sweep pays an exact whole number of satoshis per vbyte of an *estimated* size
(`42 + 68|91|148` per input by script type), and that estimate overshoots the
real signed size every single time — meaning the fee was priced from a lookup
table before the transaction existed and never recomputed. It fits **2,424 of
2,426** sweeps. Combined with `nLockTime = 0` (where Core and Electrum write
the current height) and 1,692 of 3,446 signatures carrying a 33-byte R (which
Core, Electrum and Sparrow all pay a retry to avoid), this is a signature
ordinary wallets do not produce.

This is not decorative. It is what found **wave 4** — 45.9 BTC across 1,126
victim addresses on 31 July, *after* the advisories shipped. Block's scan ran
to block 960230; wave 4 opens at 960345, past the end of it, and prices its fee
at 8 and 50 sat/vB where the first night used 30. A scan looking for the right
shape at the wrong constant walks straight past it. The repository's own FAQ is
blunt about this: the second wave *"already came, and it was missed by looking
in the wrong blocks for the wrong number."*

**4. The detector ships, self-tests, and states its own failure mode.**
`tools/coldcard-node-scan.py` runs against your own unpruned Core node. It
begins by re-deriving the fee rate of eight documented sweeps and refuses to
continue if they do not return 2 and 30 sat/vB — *"If those do not come back at
2 and 30 sat/vB, the scan is broken and nothing else it prints is worth
reading."* It reports zero false positives across 7,553 transactions at 30
sat/vB, **and** it declares where it stops working: at 2 sat/vB the same
fingerprint collides with ordinary traffic, because 2 sat/vB was the going rate
and the size table is one anyone would copy.

I read this tool; I did not run it. It requires a full unpruned archival node
we do not have, and running third-party code was out of scope. Its behaviour is
described above from source.

### What it argues rather than proves

Held to the same standard, three things are assertion:

- **The root-cause anatomy.** `src/components/RootCause.tsx` restates the
  `#ifndef MICROPY_HW_ENABLE_RNG` defect correctly and cites the four culprit
  commits, but the repository never builds the firmware, never fetches the
  submodules, and never demonstrates the symbol resolution. It is a faithful
  secondary account of Block's and the vendor's findings.
- **The entropy ladder** (§4). Six numbers in a TSX constant. No code computes
  them.
- **Attacker identity and whether it is one party or several.** Explicitly held
  open as unresolved questions, with the evidence for each reading laid out.

---

## 3. Evidence quality, on our own ladder

Scored against `2026-07-31-omega-first-class-pentester-speculation.md` §5.
The ladder was written for vulnerability findings, so this is an analogy, but
the rungs transfer cleanly because they are really about who can re-run what.

| Component | Rung | Why |
| --- | --- | --- |
| Chain reconstruction (`fetch` → `chain.json`) | **T2-equivalent** | Executes, re-runnable against a public API or your own node, and reconciles against independent published figures with drift detection |
| `coldcard-node-scan.py` | **T2/T3-equivalent** | Runnable detector with a built-in self-test on known-answer data and a stated false-positive rate |
| The nine attribution tests | **T2-equivalent** | Computed from fetched transactions, each stating what the null hypothesis predicts before measuring |
| Root-cause anatomy | **T0/T1** | Correct, cited, but a restatement — not independently verified here |
| Entropy ladder | **T0** | Asserted constants, and the most quotable part of the page |
| Recovery demonstration | **not present** | Cited to a third party (§4), not reproduced |

The interesting structural point: the repository's evidence *strengthens* as it
moves away from the bug and toward the money. Its weakest claims are the ones
about cryptography and its strongest are the ones about chain data — which is
the correct division of labour for an on-chain analyst, and it mostly stays on
the right side of it.

Two disciplines deserve explicit credit because they are rare:

- **The single decisive test is testimony, not chain analysis.** The
  `testimony` test is the only one marked `establishes-theft`, and its own text
  says *"This is the only evidence separating theft from a rescue."* The
  repository states directly that a white-hat sweeping coins to safety is
  indistinguishable on-chain from a thief. Eight of nine tests are explicitly
  incapable of proving the thing the page is about, and it says so.
- **It publishes six standing limits**, including that 2,348 emptied addresses
  are not 2,348 people; that Block did not confirm the two waves it published;
  that wave 6 was not built by the same tool, so *"if both are theft, the bug
  had more than one exploiter"*; and that wave 5 is an unmeasurable floor
  because that operator pooled nothing, so the trick that turned one report
  into 500 victims does not work there.

It also declines to republish a researcher's screenshot because it contains the
real mnemonic and xprv of his own test device, describing what the image
asserts instead. That is the correct call and worth noting given how much of
the rest of the page is verbatim archival.

---

## 4. Where it lands on the search-space question

This is where the repository is sharpest, and it resolves an apparent
disagreement rather than adding a third position.

The published positions:

| Source | Position |
| --- | --- |
| Coinkite (technical backgrounder) | ~40 bits Mk3, ~72 bits Mk4/Q/Mk5 |
| Block (@max_guise) | Mk2/Mk3 *"deterministic, not random"* from UID, timer state and call history; Mk4/Q/Mk5 *"the reseed truncates it to 32 bits"* |
| Ledger CTO (@P3b7_) | 73 bits *"look good enough. Brute forcing the full space is not economically relevant"* — but *"there is a large room for optimization. And I don't even talk about the device IDs that are anything but random"* |

The repository refuses a single number and publishes a **range keyed to what
the attacker already knows** (`RootCause.tsx:14-51`):

| Bits | Case |
| --- | --- |
| 128 | Intended — BIP-39 12-word seed as designed |
| 73.3 | Mk4/Q/Mk5, UID and timers unknown — *"the upper bound for a unit the attacker knows nothing about"* |
| 40.7 | Mk3, UID and timers unknown — *"the upper bound: serial number never observed, boot timing unmeasured"* |
| 32 | Mk4/Q/Mk5 where the reseed succeeded — *"secure-element entropy hashed to 40 bytes, but only 4 reach reseed()"* |
| 16.3 | Mk3, UID known, timers unknown — *"≈80,000 reachable SysTick states; the case Greg Sanders actually reproduced"* |
| 0 | Mk3, UID and timers both known — fully deterministic |

**So it agrees with the vendor's arithmetic and with Block's reading at the
same time**, by making explicit what both leave implicit: 40 and 72 are upper
bounds conditioned on attacker ignorance, not entropy. 40.7 and 73.3 are within
rounding of Coinkite's ~40 and ~72 — the repository is not disputing the
vendor's numbers, it is *bounding* them. It then agrees with Block on substance
by showing how fast the bound collapses once the attacker learns the serial
number or the boot timing: to 32 bits for later devices whose reseed worked,
and to 16.3 bits for the Mk3 case actually demonstrated.

That framing also explains the Ledger CTO's caveat, which reads as a
non-sequitur next to a flat "73 bits." *Room for optimization* and *device IDs
that are anything but random* are precisely the moves from the 73.3 row to the
32 row. The repository's ladder is the missing middle of that argument.

**On our own doc's framing.** Our task brief characterised the split as
Coinkite's ~40/~72 *versus* Block's more nuanced reading. Having read the
sources, that is not quite the shape of it: the numbers are not in conflict,
the *conditioning* is. Coinkite quoted a bound; Block described the conditional
distribution; the repository is the first of the three to publish both together
so the relationship is visible. If we cite the disagreement in future, this is
the more accurate way to state it.

The load-bearing caveat: **the ladder is asserted, not computed.** No code in
the repository derives 16.3 or 73.3. It is the least-evidenced and most
quotable content on the page — exactly the inversion our own thesis warns
about. Anyone citing these numbers is citing a T0 claim, and should say so.

### The reproduction belongs to someone else, and the repository says so

The one end-to-end demonstration in the record is credited, not claimed. On
2026-07-30 at 22:37 UTC — thirteen minutes before the vendor advisory — Greg
Sanders (`@theinstagibbs`) posted a recovery run against his own disposable Mk3
on v4.1.3 reporting `xpub_match=true` and `recovery_verified=true`, recovering
the device's private root on an ordinary computer without the Coldcard ever
revealing its mnemonic, by modelling the fallback generator against an
80,000-state timer range plus the keypad-shuffle call trace.

That is a genuine T3/T4 artifact, and it is the evidence that converts the
whole story from argument to demonstration. It is not this repository's work,
and this repository does not pretend otherwise — the 16.3-bit row names him as
the case actually reproduced.

---

## 5. What would have caught it

The repository does **not** make a "what would have caught it" argument. It
makes no claim about detection tooling at all. What it does is archive the
evidence, and that evidence points somewhere different from our own conclusion.

Our answer, from the prediction doc, is technical: symbol-level build
verification (the vendor's `rng-code-check`), whole-program reachability from a
sink, attack-surface ranking, and a corpus of prior shapes.

The evidence assembled here is **procedural**, and it is new to us. From
`commits.json`, the four commits Block identifies as the path from "guard
written wrong" to "every seed predictable":

| Commit | What the data records |
| --- | --- |
| `libngu@f19de05` (2021-01-28) | The defect itself. **No PR** — pushed straight to a branch. Commit message is literally `"x"`. +881/−200 across 28 files |
| `firmware@b18723d` (2021-03-01) | *"First pass w/ libNgU"* — **+2766/−2722 across 120 files**, moving seed generation onto libNgU inside the same change that strips the GPL crypto libraries. **No PR** |
| `libngu@61ffc74` (2022-03-11) | The truncating reseed entry point. PR #18, merged, **zero reviewers** |
| `firmware@01cb43f` (2022-03-11) | Mixes secure-element entropy but caps at 2^32 rather than repairing the fallback. PR #90, merged, **zero reviewers** |

**All four landed with no recorded reviewer, and two had no pull request at
all.** The defect entered a hardware wallet's seed generation inside a
120-file commit whose subject line was "First pass."

This is a materially different answer from ours, and both are right. Ours says
*a scanner needs whole-program reachability to see this*; this says *no human
ever looked at the diff.* A review gate on changes touching entropy paths is
cheaper than everything on our list and would have put eyes on the `#ifndef` in
2021. Our doc does not mention review process anywhere. It should — a "what
would have caught it" list that omits "somebody reading the patch" is
incomplete, and this repository supplies the citation.

The FAQ adds one more: **no CVE has been assigned.** As of 31 July 2026 the NVD
has no record of the flaw, and search results point at an unrelated 2024 Trust
Wallet entropy bug because it is the same defect class.

---

## 6. What it teaches us

### 6.1 A natural experiment on AI code review — and it cuts both ways

The repository's timeline preserves two dated, citable facts that sit in direct
tension, and together they are the most useful thing in it for us.

**Weeks before the drain**, the vendor ran a frontier model over this code and
it found nothing. From the technical backgrounder, quoted verbatim in the
timeline: *"A few weeks ago, we used one of the best available AI models to
review our code for security issues, and it did not find this bug or anything
serious."*

**Hours after disclosure**, an outside researcher found it from the public
repository in an afternoon: *"I was able to use Opus 5 to sniff out the
ColdCard vulnerability after cloning the firmware repo."*

Same class of tool, same public code, opposite outcomes. The variable is
knowing that a specific catastrophic bug exists and roughly where.

Both directions matter to us:

- **The vendor's miss is independent, pre-registered-in-effect corroboration of
  our prediction.** It is the closest real-world analogue to a Loupe run against
  this target that exists, it was performed before anyone knew the answer, and
  it missed. We predicted a miss for structural reasons; an unrelated party ran
  something in that family and missed. That is weak-to-moderate confirmation
  obtained *before* we run our own experiment.
- **It is not a Loupe run, and must not be reported as one.** Unknown harness,
  unknown scope, unknown prompt, unknown whether the submodules were present —
  which is the single most decisive variable in our own experiment design. It
  cannot substitute for the two-arm run our prediction doc specifies.
- **The post-disclosure hit is a warning about our own scoring.** Our prediction
  doc lists *"hindsight inflates findability"* as a caveat. This is empirical
  evidence that the caveat is load-bearing rather than decorative: the same
  defect flipped from invisible to an-afternoon's-work purely on knowing it was
  there. Any future claim that a scanner "would have caught" a historical bug —
  ours or anyone's — has to survive this. It also means the honest framing of
  our own experiment is that we are testing a scanner *that knows nothing*, and
  we must not leak the answer into the scope, the prompt, or the file selection.

### 6.2 Derive, don't assert — and the counterexample in the same repository

The most transferable engineering idea here is a discipline we already reach
for and do not consistently enforce: **the claim and the justification for the
claim are emitted by the same code path, so they cannot drift.** One
hand-maintained evidence file; everything else generated, each address carrying
the derivation that produced it; reconciliation against outside figures that
flags `DRIFT` instead of quietly disagreeing.

That is directly applicable to product promises, receipts, and ProductSpec
evidence refs, where we currently write a claim in one place and its evidence in
another and rely on discipline to keep them aligned.

And the repository supplies its own counterexample, which is the better half of
the lesson. A GitHub Action refreshes the chain data on a schedule; the prose
does not refresh. In roughly 36 hours the README went stale against its own
generated data:

| README says | Data says |
| --- | --- |
| "three waves, 1,082 BTC" | 1,082.58680432 BTC is episode 1 only; the dataset holds **six waves, 1,129.13105121 BTC** |
| "1,202 sweeps out of 1,202 … at 30 sat/vB and then at 2" | **2,424 of 2,426**, across three sittings, at four constants {2, 8, 30, 50} |
| "untouched for 1d 16h" | `dormantSeconds` = **16h 55m** |
| "~72-bit Mk4/Q/Mk5" | the component says 73.3 |

None of these are dishonest — the derived layer is correct throughout and the
prose is simply behind it. That is exactly the point: **the generated claims
stayed true and the written claims rotted, in the same repository, in a day and
a half.** We make a great many written claims. This is a cheap, dated,
real-world argument for generating them instead, and for treating any
hand-written number in a doc as a liability with a half-life.

Anyone quoting this repository should quote `chain.json`, not the README.

### 6.3 For the fix-as-a-service thesis

Three specific updates to
`2026-07-31-fix-as-a-service-company-thesis.md`:

- **"Attested absence" has a worked example now.** The thesis argues nobody
  sells *bounded, evidenced negative results*. This repository ships a small one:
  a detector with a self-test, a stated false-positive rate, and an explicit
  declaration of the regime where it stops working (2 sat/vB). That is the shape
  the thesis is reaching for, executed by one person in a weekend, and it is
  worth studying as a template for what a negative-result artifact looks like.
- **The highest-consequence finding here was a scan-constant change, not a new
  bug.** Wave 4 was missed by a correct detector run at the wrong constant in
  the wrong block range. The thesis's *variant analysis* row (boundary table
  #13) is usually framed as hunting a known shape across other repositories;
  this is the same idea in the time dimension — re-running your own detector
  with the parameters relaxed. Cheap, and it found 45.9 BTC nobody else had
  written about.
- **The marketing warning survives contact.** The thesis and the prediction doc
  both caution that the highest-profile bugs are the ones a file-scoped scanner
  cannot see. The vendor's own failed AI review is now a citable instance. It
  should be cited *against* ourselves whenever we are tempted to claim coverage
  from a finding count.

### 6.4 For our own code

We have spent the week finding the same defect class in this repository: code
that exists, ships, passes tests, and is never reached — a kill switch whose
hooks were never wired, a token minter whose tests asserted the same wrong shape
it emitted.

Coldcard is the same family with the polarity flipped. The hardware TRNG existed
and was correct. The board config was correct. The guard was there and looked
like a safety check. Every file passed review on its own terms. The defect is
that **the correct implementation was never reached**, because two objects
exported `uint32_t rng_get(void)` and the linker picked the other one.

The generalisation worth carrying: *the artifact exists* and *the artifact is
reached* are separate properties, and our tooling almost always checks the
first. A test that imports a module proves the module compiles, not that
production ever calls it. Our green suite asserted a wrong shape consistently
and stayed green. Coldcard's build succeeded and shipped for five years. In both
cases the missing check is the same question — *does the path from the entry
point actually arrive here?* — and in both cases it is answerable with
reachability analysis rather than more tests.

That is the strongest argument yet for the L0/L1 whole-program capability, and
it now has two independent supports: our own defects, and a five-year, ~1,129
BTC one.

---

## 7. Caveats and honest limits

- **I read this repository; I did not execute it.** No fetch script, no build,
  no scanner run. All behavioural statements are from source. The scanner needs
  an unpruned archival node we do not have.
- **The numbers above are the repository's own**, extracted from `chain.json`
  and its source. I verified them for internal consistency — wave sums reconcile
  to the summary, victim gross minus sweep fees equals net swept, and the three
  published-figure reconciliations pass — but I did not independently verify any
  of them against the blockchain. A clean-room check would mean running the
  fetch against our own node.
- **Its data is a floor and says so.** Everything descends from eight seeds.
  Sweeps into addresses nobody has reported appear as nothing at all rather than
  as a gap. Waves 5 and 6 exist in the record only because two victims happened
  to reply to a thread.
- **The social archive is third-party content**, recovered through a
  syndication endpoint, with timestamps decoded from snowflake IDs. Quotes above
  are quotes of that archive, not of the platform.
- **No new vulnerability claim is made or implied here.** Every defect statement
  restates the vendor's and Block's published findings. The flaw is patched
  (Mk3 4.2.0, Mk4/Mk5 5.6.0, Q 1.5.0Q), and the operative advice in the source
  is unchanged: firmware does not repair a seed that already exists, so an
  affected user must migrate off the seed.
- **Attribution.** The reconstruction, the fee-estimator fingerprint, the
  scanner, and the entropy ladder are Kevin Kelbie's work. The two waves
  reconciled to the satoshi are Block's (Clay Garrett). The 500-transaction wave
  and the first public "entropy" diagnosis are Rob Hamilton's. The root-cause
  disclosure across all five device models is Block's (Max Guise). The RNG
  hypothesis and the correct prediction of a second wave are Kevin Loaec's. The
  only end-to-end recovery demonstration is Greg Sanders'. Nothing in this
  document is our own forensic work.
