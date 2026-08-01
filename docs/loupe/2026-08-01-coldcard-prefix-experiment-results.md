# Results: Loupe on the pre-fix Coldcard firmware

Status: **experiment complete. Prediction refuted.**

Scored against the rubric in
[`2026-08-01-coldcard-prefix-experiment.md`](2026-08-01-coldcard-prefix-experiment.md),
which was written and pushed **before** the run and has not been edited since.

The prediction in
[`2026-08-01-would-loupe-have-caught-coldcard.md`](2026-08-01-would-loupe-have-caught-coldcard.md)
was **almost certainly not**. That prediction was **wrong**, in a way that is
more useful than being right would have been.

---

## 1. Verdict

| Arm | Files | Findings | Sev | **Verdict** |
| --- | --- | --- | --- | --- |
| **A** — default clone, submodules absent | 12 | 12 | 3 High, 9 Med | **MISS** |
| **B** — submodules materialized | 16 | 22 | 5 High, 17 Med | **HIT** |

Target: `bcc2c382`, the last commit before the fix, verified vulnerable (board
`rng.c` exports no `rng_get`). Both scans `Succeeded`. Verification disabled per
the pre-registration, so every finding is one model's unconfirmed claim.

**Arm B found the defect three times, from two directions, on two boards.**

---

## 2. The hit

**#152**, `stm32/COLDCARD/mpconfigboard.h:77`, CWE-338. Quoting the finding:

> `external/libngu/ngu/random.c` selects `CHIP_TRNG_32() = rng_get()` whenever
> `MICROPY_PY_STM` is set, and **its guard only uses
> `#ifndef MICROPY_HW_ENABLE_RNG`. With this board header, that guard is bypassed
> even though the value is 0.** If the normal STM `rng_get()` object is linked,
> the disabled-RNG branch in `external/micropython/ports/stm32/rng.c` exports a
> **deterministic Yasmarang fallback seeded from boot-visible/predictable
> registers**; `ngu.random.bytes()` is used by `shared/seed.py` for **new wallet
> seed entropy**.

That is the complete causal chain: the zero-valued macro, the `#ifndef` testing
existence rather than value, the linker resolving to MicroPython's object, the
deterministic PRNG, and the path to seed generation. It is the bug.

It also **flagged its own limit honestly** — *"the mounted tree does not include
full build metadata, so a downstream build that replaces `rng_get()` with the
board-specific hardware path would change the impact"* — and explicitly
deduplicated against #148 as a distinct guard path.

Corroborating findings in the same arm:

- **#161**, `stm32/COLDCARD_Q1/mpconfigboard.h:80`, **High** — *"Q1 RNG macro
  disables hardware entropy for `ngu.random`."* The same defect on a second
  board, found independently.
- **#148**, `external/micropython/ports/stm32/rng.c:74` — *"Do not back
  `rng_get` with replayable Yasmarang output."* The same defect approached from
  the fallback's side.

Under the frozen rubric this clears the HIT bar with room to spare: it names the
`#ifndef` testing existence rather than value, **and** key material derived from
a deterministic PRNG, **and** the wrong `rng_get` being linked.

Arm A, by contrast, produced RNG-*adjacent* hardening notes — `rng.c:69`
"reject error status", `rng.c:86` "apply the repeated-output check" — and
nothing naming the macro, the guard, the linker, or the fallback. That is the
rubric's explicit not-a-hit case.

---

## 3. Why this is a clean result despite the contamination worry

The pre-registration warned (§7) that a HIT would be **muddy** evidence, because
the model may have read about the July 2026 incident, and that only a MISS would
be clean.

**The two-arm design turned out to control for exactly that**, by accident:

- Same model, same effort, same prompt, same training data, same knowledge of the
  incident.
- The **only** difference is whether two files existed on disk.
- If the hit were recall, **arm A would have hit too.** It did not.

Sharper still: **the winning finding is located in a file that was present in
both arms.** `stm32/COLDCARD/mpconfigboard.h` was scanned in arm A and produced
nothing; the same file scanned in arm B produced the full chain. Loupe assigns
one file per session but mounts the **whole worktree** read-only at `/workdir`,
so in arm B that session could open libngu's `random.c` and MicroPython's
`rng.c` and see the contradiction.

**Same file. Same model. Same instructions. The discriminating variable is what
else was on disk.**

---

## 4. What I got wrong

The prediction's core architectural claim was that per-file analysis is
**structurally blind** to cross-file contradictions. That is false, and the
experiment falsified it cleanly.

The error was conflating *"the agent is assigned one file"* with *"the agent can
see one file."* It cannot: the prompt directs attention to one file, but the
tooling exposes the entire worktree. Given the relevant files, a single session
reasoned across three codebases and reconstructed a linker-level defect from
source alone.

Two sub-claims survive, and one is weakened:

- **Survives:** Loupe never builds the target, so it inferred the linker outcome
  from source rather than observing it — which is why the finding correctly
  hedges on build metadata. A symbol-provenance check would still be strictly
  better evidence.
- **Survives:** the "must be testable" rule did not suppress the finding, but the
  PoC it produced is a *source-level* test asserting the guard's behaviour, not
  proof that the shipped binary linked the wrong object. The proof is weaker than
  the claim.
- **Weakened:** attack-surface ranking (L0). This run handed the scanner 12–16
  hand-picked files. Nothing here shows it would have surfaced this among 458,
  and the pre-registration flagged that scoping as a deliberate bias toward the
  tool. Ranking still matters; this experiment says nothing about how much.

---

## 5. The finding that actually matters for practice

**Loupe's default path would have missed this**, and not because of anything a
user would notice.

From the source, `crates/loupe-worker/src/repo_cache.rs:314`:

```rust
fn clone_bare(path: &Path, clone_url: &str, token: Option<&str>) -> Result<()> {
    let url_with_token = inject_token(clone_url, token);
    let output = std::process::Command::new("git")
        .arg("clone")
        .arg("--bare")
```

Two facts, both verified:

1. **`git clone --bare` cannot fetch submodules.** No `--recurse-submodules`, and
   a bare clone has no worktree to place them in.
2. **The string `submodule` appears nowhere in Loupe's `crates/` tree.** No init,
   no update, no warning, no configuration option, no mention in the README.

So an operator pointing Loupe at the Coldcard repository the week before the
theft gets **arm A**: twelve findings, three rated High, several genuinely worth
fixing — and not the one that mattered.

**The failure mode is worse than a silent miss: the scan looks productive.**
Nothing in the output indicates that two-thirds of the relevant program was never
on disk. A tool that reports "12 findings" against a program it only partly read
is making an implicit completeness claim it has not earned.

This is the same shape as every other defect this repository has been chewing on
for a week: **a green result that structurally could not have been red.** A spend
ceiling that read zero; a privacy scan clean because canaries were never
injected; a sweep reporting healthy while unable to close a loop it opened. Add:
a security scan that analysed a third of a program and said nothing about it.

---

## 6. What to build, reordered

The [hardening analysis](2026-08-01-hardening-against-ai-assisted-attacks.md)
ranked a symbol-provenance checker and a reachability oracle at the top. This
result inserts something smaller and cheaper above both:

**1. Materialize dependencies before scanning.** Submodules, vendored trees,
lockfile-pinned sources. On this evidence it is the single highest-leverage
change available to any scanner, and it is a day of work.

**2. Refuse to report confidently on an incomplete program.** If `.gitmodules`
exists and those paths are empty, the run is incomplete and must say so in the
output, in the receipt, and in any coverage claim. This is the
[attested-absence](2026-07-31-fix-as-a-service-company-thesis.md) discipline
applied to ourselves: a scan's honesty about what it could not see is part of the
product, not a footnote.

**3. Then** symbol provenance, then reachability, then ranking.

Worth sending upstream: both items are contributions to Loupe rather than
reasons to build a competitor, and the second is a design argument its authors
would likely accept, given how carefully the rest of the tool guards against
overclaiming.

---

## 7. Limits on this result

- **One run per arm.** Agent runs are stochastic; no repeat trials.
- **No verification.** Loupe's verify stage is broken, so all 34 findings are
  unconfirmed. The hit is a claim, and its PoC has not been executed.
- **Scoping was generous.** Hand-picked files, chosen knowing the answer. This is
  an upper bound on real-world performance, not an estimate of it.
- **The PoC proves less than the finding claims.** Source-level assertion, not a
  build-artifact observation.
- **Nothing here says the tool would find an unknown bug of this class.** It says
  that, given the files, it reconstructed a known one.

The honest one-line summary: **Loupe can find this class of defect, and its
default configuration guarantees it will not.**
