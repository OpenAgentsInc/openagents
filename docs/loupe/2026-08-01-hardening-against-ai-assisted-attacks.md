# What would have caught it, what else to hunt, and what we should build

Status: **analysis and speculation.** Authorizes nothing. No public claims about
any third party beyond what the cited sources already state publicly.
`AUTHORITY.md` and `INVARIANTS.md` keep precedence.

This answers five questions, in order:

1. Did Loupe find the Coldcard vulnerability in advance?
2. *Could* it have?
3. **What would have?**
4. How do we run that against other projects now?
5. What else should we be hunting for — and what changes now that attackers have
   cheap agents?

Then: what we should build, and what we should not.

Grounding: [`../coldcard/chatgpt-pro-analysis.md`](../coldcard/chatgpt-pro-analysis.md)
(the defect), [`../coldcard/2026-08-01-kelbie-independent-postmortem-analysis.md`](../coldcard/2026-08-01-kelbie-independent-postmortem-analysis.md)
(third-party postmortem), [`2026-08-01-would-loupe-have-caught-coldcard.md`](2026-08-01-would-loupe-have-caught-coldcard.md)
(our prediction), [`loupe-in-plain-words.md`](loupe-in-plain-words.md).

---

## 1. Did Loupe find it? No — and "opt-in" is only half the answer

The common explanation is that Loupe is opt-in and Coldcard did not run it. True,
and insufficient. It lets the tool off too easily and it teaches the wrong
lesson, which would be *"get more projects to opt in."*

The full answer is that **running it would probably not have helped**, for
reasons in §2. Both facts matter: adoption was zero *and* the design could not
see this class. Fixing only the first fixes nothing.

---

## 2. Could it have? Almost certainly not

Four structural reasons, argued fully in the prediction document:

1. **One session per file; the defect spans three codebases.** Each of the three
   files is locally correct.
2. **Two of the three are absent** from a default clone — unfetched submodules,
   verified at zero entries.
3. **Loupe reads source; the bug lived in linking.** Two objects exported the
   same symbol; the linker chose; no diagnostic.
4. **Its best rule suppresses it.** No finding without a test that fails on
   HEAD — and the required proof here is a build-system assertion, not a unit
   test.

### The independent evidence nobody planted

Two facts from the postmortem, and together they are the most useful thing in
this entire document.

**The vendor stated publicly that they used one of the best available AI models
to review this code, and it did not find the bug.** That was *before* the theft.
It is an uncontaminated, effectively pre-registered negative result from a party
with every incentive to find it, using a stronger setup than a per-file scanner.

**A researcher then found it with a frontier model in an afternoon — after
disclosure.** Knowing a bug exists, and roughly where, is a different task.

Read together: **the difficulty was never comprehension. It was attention.** No
model was asked the right question about the right code before the money moved.
That single distinction is the design brief for everything below.

---

## 3. What would have caught it

Ranked by cost, cheapest first. The ordering is itself the finding: the most
effective controls here are not AI.

### 3.1 Requiring a second reviewer on entropy-critical paths — nearly free

The postmortem establishes that **all four culprit commits landed with zero
reviewers**, two with no pull request at all, and the defect entered inside a
120-file commit titled *"First pass."*

No scanner is needed to notice that a change to seed generation had no second
pair of eyes. A `CODEOWNERS` entry over `rng.*`, `seed.*`, key derivation, and
board config, plus a branch rule requiring one approving review on those paths,
would have put a human in front of the exact diff.

**Our own "what would have caught this" list was entirely technical and missed
this.** That is a real gap in our thinking, and it is a warning about the failure
mode of security-product people: reaching for a detector when a policy is
cheaper and more reliable.

### 3.2 A symbol-provenance assertion at build time — cheap, decisive

This is what the vendor shipped after the fact:

```make
rng-code-check:
	upstream_symbols="$$($(NM) --defined-only $(BUILD_DIR)/rng.o)"
	if test -n "$$upstream_symbols"; then ERROR; exit 1; fi
```

Run `nm` on the compiled objects; fail the build unless the security-critical
symbol comes from the intended object and the fallback contributes nothing.

Generalized, the rule is: **for each security-critical symbol, assert which
object defines it.** It is a few lines, it runs in milliseconds, it needs no
model, and it converts a silent linker decision into a build failure. Every
project with a vendored or submoduled crypto/entropy dependency should have one.

### 3.3 Reachability from sink to certified source — the real detector

Ask one question, mechanically: **does key material derive from a certified
entropy source, along every path?**

That is the L0/L1 idea in the [speculation document](2026-07-31-omega-first-class-pentester-speculation.md).
It requires a call graph over the *built* program, not a file. It is the only
technique on this list that generalizes to defects nobody has seen yet, because
it encodes the property you want rather than the mistake you are looking for.

Concretely: mark `ngu.random.bytes` and friends as sinks, mark the hardware TRNG
as the certified source, and require every path to terminate there. The Coldcard
path terminated in Yasmarang — a deterministic PRNG — and the check fails.

### 3.4 Differential and statistical testing of generated secrets — moderate

Generate 10⁶ seeds on-device; check for structure. This is weaker than it
sounds: the output was double-SHA-256'd, so it passes every standard randomness
battery. **Hashing a small candidate set gives a small candidate set that looks
random.** Statistical testing of the *output* was always going to fail here.

What would work is testing the *input*: entropy accounting, not output
randomness. How many bits entered before the hash? That is a provenance
question again, which is why §3.3 dominates.

### 3.5 Reproducible builds plus artifact diffing — moderate, high value elsewhere

If the shipped binary is reproducible, you can diff what changed between
releases at the object level. The v4.0.0 build silently gained MicroPython's
`rng.o` symbols. **A build-artifact diff would have shown a new symbol appearing
in a security-critical object.** No source review shows that.

---

## 4. How we would run this on other projects now

Concrete, in the order we would actually do it.

**Step 1 — the provenance check, as a standalone tool.** Generalize
`rng-code-check` into something that runs against any C/C++/Rust project with a
config file naming symbols and their required providers. Ship it, open-source,
and offer to wire it into projects that want it. It is small, it is
mechanically checkable, and it makes the first contact with a maintainer a gift
rather than a bug report.

**Step 2 — the negative-space scan.** *"Which security-critical functions are
defined but never reached from a production entry point?"* We already built this
for TypeScript this week (`scripts/uncalled-production-symbol-guard.mjs`) after
finding **ten** instances in our own repo — including a spend ceiling that read
zero for weeks and a refusal reason with no code path.

Coldcard's reviewers looked at the binary, found the carefully written hardware
RNG implementation, and concluded it was present. **It was present. It was not
called.** That is the same defect class, and it generalizes far beyond entropy.

**Step 3 — attack-surface ranking (L0), measured.** Rank by *"attacker-controlled
bytes reaching key material."* Then measure cost-per-confirmed-finding against an
alphabetical baseline on one real target. If ranking does not beat alphabetical,
that is a one-week negative result, and it kills a large piece of our thesis
cheaply. That is the point of running it.

**Step 4 — variant hunting.** Once a shape is confirmed, hunt it across the
ecosystem. "Entropy truncated or substituted before expansion" either exists or
does not in every wallet that forked or reimplemented that code. Marginal cost of
checking project N+1 for a *known* shape is near zero.

**Step 5 — coordination.** Encrypted maintainer contact, embargo timers,
hash-committed findings so two teams discover they are on the same bug without
revealing it. This is the part Episode 263 actually asked for and the part no
scanner provides.

---

## 5. What else to hunt — the classes that matter

Ordered by expected loss, not by frequency.

### The prioritization insight: does the attacker get a free oracle?

The Coldcard attack was economically viable for one reason beyond the bug: **the
blockchain is a public validation oracle.** The attacker never had to wonder
whether a candidate seed was right — derive the address, check it against public
data, and a match is proof. No interaction with the victim, no rate limit, no
detection.

**That property, not severity, is what makes a bug catastrophic at machine
scale.** It should drive scanning priority directly:

| Oracle available | Consequence |
| --- | --- |
| Public, free, unlimited (blockchain, published keys, signatures) | Offline brute-force viable. **Top priority.** |
| Requires interaction with the victim | Rate-limited, detectable. Lower. |
| Requires physical access | Lowest, whatever the CVSS says |

Any defect that narrows a keyspace *and* is checkable against public data
belongs at the top of every queue. That is a small, well-defined family:
entropy, nonces, key derivation, and address/xpub generation.

### The classes

**1. Entropy provenance.** The Coldcard class. Every path from key material back
to a certified source. Includes the non-obvious consumers — the postmortem notes
the same weak RNG fed paper-wallet keys, Seed XOR masks, cloning keys, and
generated passwords.

**2. Nonce generation and reuse.** Historically the most expensive bug class in
Bitcoin. Deterministic-nonce deviations, biased k, reuse across signatures. It
has the free-oracle property in its purest form: signatures are public, and
recovery from nonce bias is textbook.

**3. Silent security downgrades.** The meta-class Coldcard belongs to. Anywhere a
secure path can degrade to an insecure one **without failing loudly**: crypto
library fallbacks, optional constant-time implementations, verification that
returns "unknown" and is treated as "ok". *Rule: a security control that can
silently become a no-op is worse than one that is absent, because it is
load-bearing in everyone's mental model.*

**4. Uncalled security code.** Defined, reviewed, never reached. Our ten
instances; Coldcard's TRNG implementation. Mechanically detectable and almost
nobody looks.

**5. Build-versus-source divergence.** What is compiled is not what was reviewed:
conditional compilation, feature flags, vendored copies drifting from upstream,
linker choices. Requires building; that is exactly why it is under-scanned.

**6. Parser memory safety on attacker-controlled bytes.** PSBT, descriptors,
network messages, QR payloads. Well-served by conventional fuzzing, which is why
it is the *least* differentiated thing we could build.

**7. Signature and validation skips.** Malleability, DER laxity, low-s, missing
checks on recovered pubkeys.

**8. Update and downgrade paths.** Firmware signature verification, rollback
protection. A wallet that can be silently downgraded to a vulnerable release
inherits every historical bug.

**9. Side channels in signing.** Timing, power, cache. Hard to detect statically,
expensive to prove, real for hardware wallets.

**10. Dependency substitution.** The Coldcard mechanism generalized: does the
artifact you shipped contain the code you think it does? Applies to every
language with a package manager.

---

## 6. What changes now that attackers have cheap agents

Four shifts, and they are not symmetric with defense.

**Obscurity stops working.** A bug that survived because reading the code was
expensive no longer survives. The Coldcard defect sat in public for years. There
is a **backlog of latent bugs in widely-forked code** that was only ever
protected by nobody bothering, and that protection is now gone.

**Historical-bug mining becomes cheap.** Every past CVE is a template. Variant
analysis across forks is the single highest-return attacker activity, and it is
symmetric — we should be doing it first and faster.

**The economics invert on obscure code.** Attackers can afford to read *all* of
it. Defenders still triage by importance. The gap between "code that matters" and
"code anyone looks at" is the attack surface.

**Attackers do not need proof.** They need a candidate and an oracle. We hold
ourselves to executed proof before contacting a maintainer — rightly — but should
be clear-eyed that **the adversary's bar is lower than ours by design.** They
only have to be right once, and the blockchain tells them when they are.

---

## 7. What we should build

Ranked by leverage per unit of effort.

**1. Symbol-provenance checker.** §3.2, generalized. Days of work. Immediately
useful to real projects. The best possible first contact with a maintainer
community, because it is a gift with no bug report attached.

**2. Reachability oracle: sink → certified source.** §3.3. The one detector that
generalizes to unknown defects. Hardest item here and the most defensible.

**3. Uncalled-security-code detector, ported beyond TypeScript.** We have the
TS version and the scar tissue. C/C++/Rust needs build integration, which we need
anyway for §2.

**4. Attack-surface ranker (L0), with measurement.** Build it to be *falsified*.
If it does not beat alphabetical on cost-per-finding, we learn that in a week.

**5. The shape corpus.** Every confirmed defect becomes a detector, a fuzz seed,
and a benchmark item. This is the compounding asset — combinatorial in shapes ×
targets, unscrapeable because it is defined by execution.

**6. Coverage attestation.** Signed, reproducible records of where we looked, how
hard, and what we did *not* find. Nobody sells this; assurance actually needs it;
it monetizes the runs that find nothing.

**7. Coordination fabric.** Hash-committed findings, encrypted maintainer
contact, embargo state. The Episode 263 ask.

### What we should not build

- **Another per-file LLM scanner.** Loupe exists, it is good, its discipline is
  worth adopting wholesale, and duplicating it adds nothing.
- **A generic fuzzer.** Solved, and better, by people who have done it for years.
- **A findings dashboard.** The industry's problem is not display.
- **Anything that files unverified findings at maintainers.** The fastest way to
  destroy the only durable moat here.

---

## 8. Honest limits

- **This is all hindsight.** We are designing detectors for a bug whose shape we
  know. The real test is a defect nobody has written up. Our own pre-registered
  experiment exists precisely because a model that has read about Coldcard is
  contaminated evidence.
- **§3.1 outranks everything we would build.** The cheapest fix was a review
  requirement. A security business has structural incentive to under-recommend
  process and over-recommend product, and we should say the cheap thing first
  even when it is not the thing we sell.
- **We have not proven any of this works.** Our own scan produced 132 unverified
  findings and zero confirmed ones, because the verification stage is broken. We
  should be extremely slow to make capability claims until we have executed
  proofs of our own.
- **Legal ground is unsettled** for unsolicited scanning of third-party code, and
  varies by jurisdiction and licence. Opt-in and foundation-sponsored work first.
- **This document is speculation**, and every item in §7 needs its own admitted
  plan and authority reconciliation before it exists as anything but prose.

---

## 9. The compressed answer

**What would have caught it:** a required reviewer on the diff; then a build-time
assertion about which object supplied `rng_get`; then a reachability check from
key material to a certified entropy source. Two of those three are not AI, and
the cheapest one is a branch-protection rule.

**What to hunt now:** defects that narrow a keyspace *and* can be checked against
public data — entropy, nonces, derivation. Free-oracle bugs are the ones machines
make catastrophic.

**What to build:** provenance and reachability checks that fail the build, a
corpus of confirmed shapes that compounds, and the honesty to publish what we did
not find.

**The lesson under all of it:** every party who looked at this code, human and
machine, verified that the secure implementation *existed*. Nobody verified that
it was *reached*. That gap — between present and called — is the same one we
found ten times in our own repository this week, and it is the most under-served
question in software security.
