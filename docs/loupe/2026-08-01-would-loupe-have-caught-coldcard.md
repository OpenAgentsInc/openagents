# Would Loupe have caught the Coldcard entropy bug?

Status: **prediction, not result.** This is an assessment written *before*
running Loupe against the Coldcard firmware, so that the prediction is on the
record and can be scored honestly afterwards. Writing the expected answer down
first is the only way the experiment means anything.

Reading order:

- [`loupe-in-plain-words.md`](loupe-in-plain-words.md) — what Loupe does and
  what it tells the model.
- [`../coldcard/chatgpt-pro-analysis.md`](../coldcard/chatgpt-pro-analysis.md)
  — the technical anatomy of the failure itself.
- [`2026-07-31-omega-first-class-pentester-speculation.md`](2026-07-31-omega-first-class-pentester-speculation.md)
  — the L0/L1 argument this assessment ends up supporting.

**Short answer: almost certainly not.** For structural reasons, not because of
model quality. A stronger model in the same harness would also miss it.

---

## 1. What the bug is, in one paragraph

A 2021 migration to `libsecp256k1` (via an embedded library, libNgU) changed
Coldcard's seed generation from a direct hardware-RNG call to
`ngu.random.bytes(32)`. Coldcard's board config defined
`MICROPY_HW_ENABLE_RNG (0)`, meaning *"we supply our own."* libNgU guarded on
`#ifndef MICROPY_HW_ENABLE_RNG` — whether the macro **exists**, not whether it
is **true** — so the build passed. MicroPython read the same zero conventionally
and compiled its software fallback. Both implementations satisfied
`uint32_t rng_get(void)`, so the linker silently chose one, and it chose the
fallback: a deterministic PRNG seeded from a chip UID and two timer registers.
Full anatomy in the companion document.

The essential property, for our purposes: **no single file contains the bug.**
Every individual file is locally correct and defensible.

---

## 2. Four structural reasons Loupe would miss it

Each is verified against the actual clone at
`~/work/projects/repos/coldcard-firmware`, HEAD `9a88e1a5`.

### 2.1 The unit of analysis is one file; the bug spans three codebases

Loupe launches **one agent session per source file**, and that session is told
to examine exactly that file. The Coldcard defect is a contradiction *between*:

| Where | What |
| --- | --- |
| Coldcard `mpconfigboard.h` | `#define MICROPY_HW_ENABLE_RNG (0)` |
| libNgU (separate repo) | `#ifndef MICROPY_HW_ENABLE_RNG` → `#error` |
| MicroPython (separate repo) | `#if MICROPY_HW_ENABLE_RNG` → fallback PRNG |

Read any one of those alone and there is nothing to report. The macro
definition even carries a comment explaining why it is zero. The `#ifndef` looks
like a safety check, and in isolation it *is* one. The `#if` is textbook
MicroPython. **The bug is the relationship, and the relationship is not in any
file.**

### 2.2 Two of the three files are not in the repository at all

Verified:

```
external/libngu:       0 entries
external/micropython:  0 entries
```

They are unfetched submodules. A default clone — which is exactly what Loupe
does — contains empty directories where the other two-thirds of the bug lives.

This is not a Loupe limitation so much as a fact about the target, but it is
decisive. And Loupe's prompt is explicit that the model may not paper over it:

> Your only filesystem access is the worktree mounted at `/workdir`. You cannot
> read external repositories, dependency source … Do not claim to have
> "verified against" or "checked" any out-of-tree source you cannot actually
> open.

So the harness both lacks the evidence *and* correctly forbids the model from
pretending otherwise.

### 2.3 Loupe reads source; this bug lives in linking

The failure was symbol resolution. Two objects exported the same name with the
same signature; the linker picked one; no error, no warning, no diagnostic.
Nothing about that is visible in source text.

Loupe never builds the target. It has no notion of object files, link order,
symbol tables, or reachability. Its entire input is bytes of source in one file.

The strongest evidence for how far outside Loupe's reach this sits is the
vendor's own fix: `rng-code-check`, a build step that **examines the resulting
object files** and fails unless the board object defines the global `rng_get`
and MicroPython's fallback defines no RNG symbols. The remedy operates at a
layer Loupe does not observe at all.

### 2.4 Loupe's best rule would have suppressed the finding

The prompt forbids reporting anything you cannot prove with a failing test:

> Do not call `submit_finding` for hardening notes, style issues, or bugs you
> can't write a regression test for.

And the proof required is specific: a diff adding a test that **fails on HEAD**
and passes once fixed, using the repo's existing test framework, validated with
`git apply --check`.

Now try to write that test for this bug. It would have to assert something
about which object supplied a symbol after linking — from inside a single-file
session, in Python, with the relevant code absent from disk. That test is
essentially `rng-code-check`, which is a build-system tool, not a unit test.

**So the discipline that makes Loupe trustworthy is exactly what filters this
bug out.** Requiring a falsifiable proof is what keeps slop from reaching
maintainers — the single most valuable property Loupe has — and it also excludes
the entire class of "the wrong implementation got linked." That is not a flaw to
patch away. It is a real and permanent trade, and it defines where a per-file,
test-proof scanner stops.

---

## 3. The one way it might partially hit

Loupe's prompt does not tell the model to stay silent when it cannot see a
dependency. It says the opposite:

> If a determination depends on an invariant the *caller* of this code is
> supposed to uphold, on a downstream crate's behaviour, or on a pinned
> dependency's internals, treat that as **uncertainty**, not as a clearance to
> dismiss the bug. Note the dependency in the `description` and submit the
> finding anyway, flagging the assumption — a false positive a human can dismiss
> is better than a false negative dressed as a confident cross-reference check.

A session examining a seed-generation call site — `ngu.random.bytes(32)`, or
the `ngu.random.*` uses in `shared/calc.py`, `shared/hsm.py`, `shared/backups.py`
— could plausibly reason: *this produces key material through a library whose
source I cannot read; if it does not reach a hardware TRNG this is
catastrophic; flagging as uncertainty.*

That is a real possibility and it should be scored as a partial hit if it
happens. But three things make it weak:

1. It is a **T0 hint**, not a finding — a statement about what the model cannot
   verify, not evidence of a defect.
2. It **collides with the no-untestable-findings rule.** The model is told both
   to flag dependency uncertainty and not to report what it cannot test. Those
   instructions conflict here, and the second is stated more forcefully.
3. **The verifier would likely dismiss it.** A second model, given "I couldn't
   check the library," has no basis to confirm, and `dismissed` beats
   `inconclusive` in Loupe's rollup — any dismissal kills the finding.

The honest expectation: a small chance of a vague, correctly-hedged note that
nobody would have acted on in 2021, and that Loupe's own pipeline would probably
discard.

---

## 4. The clone we have cannot test this

Two problems with running it as-is, both verified:

**HEAD is already fixed.** `stm32/COLDCARD/rng.c:82` contains:

```c
uint32_t rng_get(void)
{
    return rng_get_or_fault();
}
```

That is the fix — the explicit export that makes the linker's choice
unambiguous. Note the macro itself is *unchanged*: `MICROPY_HW_ENABLE_RNG (0)`
is still there at `stm32/COLDCARD/mpconfigboard.h:77`,
`COLDCARD_MK4/mpconfigboard.h:79`, and `COLDCARD_Q1/mpconfigboard.h:80`. The
remedy was not to correct the flag; it was to stop depending on how two
codebases each interpreted it.

**Scanning this tree would test nothing.** It would confirm the absence of a bug
that is already absent.

A fair experiment requires, at minimum:

1. Checking out a **pre-fix commit** — the v4.0.0–4.1.9 era for Mk3, or any
   commit before the corresponding fixed release on the other tracks.
2. Deciding, explicitly, whether to **fetch the submodules.** This is the most
   interesting knob in the whole experiment: with `external/libngu` and
   `external/micropython` populated, all three contradicting files exist in one
   corpus and the question becomes *"can a per-file scanner find a cross-file
   contradiction when the files are present?"* — a different and much more
   informative question than *"can it find one when they are missing?"*
3. Scoping. 458 Python/C/H files at one agent session each is a real bill, and
   it fans out uniformly across graphics, docs tooling, and test helpers.

---

## 5. What would actually have caught it

Ranked by how directly each addresses the failure:

**Symbol-level build verification.** Exactly `rng-code-check`. Assert that the
security-critical symbol resolves to the intended object. This is the narrowest,
cheapest, most decisive control, and it is what the vendor shipped.

**Whole-program reachability from a sink.** Ask "does seed generation actually
reach the hardware RNG?" and trace it. This is a question about the program, not
about a file, and it is answerable with a call graph — precisely the L0/L1
capability the speculation document argues for, and precisely what a per-file
scanner cannot ask.

**Attack-surface ranking.** Rank code by *"attacker-controlled bytes reaching
key material."* Entropy and seed-derivation paths land at the very top by
construction. This bug was in the highest-value code in the product, and an
alphabetical file walk gives it exactly the same attention as a graphics helper.

**A corpus of prior shapes.** "Entropy truncated or substituted before seed
expansion" is a known family. A variant hunt for that shape across every wallet
that forked or reimplemented the same code is the ecosystem-scale move, and it
is cheap once the shape is known.

---

## 6. What this means for the product thesis

This is the strongest concrete support so far for the central claim in the
speculation document: **per-file is the wrong unit of analysis.**

It also sharpens the value proposition. The Coldcard bug is not exotic — it is a
**dependency-integration defect**, and integration defects are where the highest
consequence sits precisely because no single repository owns them. A scanner
that reads one file at a time is structurally blind to the class. So is a
scanner that reads one *repository* at a time when the dependency source is not
present.

Two design consequences worth carrying forward:

- **Fetch the dependencies.** Any serious analysis of a firmware or embedded
  target must materialize submodules and vendored source, or it is analyzing a
  fraction of the program. This is unglamorous and it is probably the single
  highest-leverage configuration decision.
- **Build the target.** The evidence ladder's T2 rung (execute the PoC) already
  requires building. Once you are building, symbol-level and link-level
  assertions become available — and that is the layer this bug lived in. The
  capability that proves findings and the capability that would have found this
  one are the same capability.

There is also a warning here for our own marketing, if this ever becomes a
product: **the highest-profile bugs are frequently the ones a file-scoped
scanner cannot see.** Claiming otherwise on the strength of a finding count
would be exactly the slop the thesis says to avoid.

---

## 7. Honest caveats

- **Hindsight inflates findability.** Knowing the answer makes the `#ifndef`
  look glaring. It survived years of review by capable people who were looking
  at this code on purpose. Any confidence expressed here is cheap.
- **This is a prediction.** Running it is the actual test, and being wrong would
  be the more interesting outcome. Predicting a miss and observing a hit would
  mean per-file analysis reaches further than argued here, which would be worth
  knowing.
- **A miss is not a verdict on Loupe.** Loupe is good at what it targets:
  single-file, testable, source-visible defects. Our own Omega run produced 132
  candidates in that class. Judging it against a cross-repo link-resolution bug
  is judging it against something it never claimed.
- **We have not verified the exact pre-fix commit** or confirmed which
  historical tree reproduces the vulnerable link. That is part of setting up the
  experiment, not part of this prediction.

---

## 8. The experiment worth running

Cheap, bounded, and it answers a real question rather than confirming a prior:

1. Check out a **pre-fix** Coldcard commit.
2. Run Loupe twice: **without** submodules, then **with** them fetched.
3. Scope both runs to the RNG, seed, and key-derivation paths rather than all
   458 files, so the bill is bounded and the comparison is clean.
4. Score against this document: did it flag anything in the entropy path at all?
   Did it produce the T0 uncertainty note from §3? Did populating the submodules
   change the answer?

The second run is the one that matters. If a per-file scanner still misses the
bug **with every relevant file present on disk**, that is a much stronger result
than missing it while two-thirds of the evidence is absent — and it would settle
the per-file-versus-whole-program question with data instead of argument.
