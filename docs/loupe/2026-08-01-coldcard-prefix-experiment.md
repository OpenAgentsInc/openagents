# Experiment: run Loupe on the pre-fix Coldcard firmware

Status: **pre-registered experiment. Scoring criteria fixed before the run.**

One question, answerable yes or no:

> Would Loupe, run on the Coldcard firmware as it stood immediately before the
> fix, have reported the RNG defect that led to roughly 594 BTC being swept?

The prediction is recorded separately in
[`2026-08-01-would-loupe-have-caught-coldcard.md`](2026-08-01-would-loupe-have-caught-coldcard.md):
**almost certainly not.** This document exists so that prediction can be scored
against a real run rather than argued about. The rubric in §5 is fixed **before**
execution and must not be edited after results are known — a moved goalpost
makes the whole exercise worthless.

Background: [`../coldcard/chatgpt-pro-analysis.md`](../coldcard/chatgpt-pro-analysis.md)
(the defect), [`loupe-in-plain-words.md`](loupe-in-plain-words.md) (what Loupe
does).

---

## 1. The exact commits

All verified against the local clone at `~/work/projects/repos/coldcard-firmware`
(full history, 3,914 commits).

**The fix:** `ca72463709f4e3f8964952039d5caf955f566a87` — *"fixes rng"*,
Thu 30 Jul 2026 22:40:23 -0400, by Peter D. Gray. Touches 11 files across
`stm32/`.

**The target — last commit before the fix:**
`bcc2c382a324690a2fcf972c0bac3b79bf923f7b`, Tue 21 Jul 2026. This is the tree to
scan. It is vulnerable by construction: it is `ca724637^`.

**Verified vulnerable state at `bcc2c382`.** `stm32/COLDCARD/rng.c` contains
`static uint32_t rng_get_or_fault(void)` and `STATIC mp_obj_t pyb_rng_get(void)`
but **no exported `uint32_t rng_get(void)`**. That absence is the defect: the
symbol libNgU needs is not provided by the board object, so the linker resolves
it to MicroPython's fallback.

**Submodule pins at `bcc2c382`**, needed for arm B:

| Submodule | Commit |
| --- | --- |
| `external/libngu` | `537519a829259622ea6b0334fbafd6cae852852f` |
| `external/micropython` | `4107246f8a080807b62c3b4838e71e812ea68b6f` |
| `external/ckcc-protocol` | `3d1dfa858beb58b8dac37d8c66d7aed2909812f2` |
| `external/mpy-qr` | `11347d83f4eb325b10676a4eb8e17deccfe0df44` |

**What the fix did**, which is also the answer key. From `stm32/shared.mk`:

```make
rng-code-check:
	@upstream_symbols="$$($(NM) --defined-only $(BUILD_DIR)/rng.o)" || exit $$?; \
	if test -n "$$upstream_symbols"; then \
		echo "ERROR: micropython's stm32/rng.o must not define any symbols"; \
		printf '%s\n' "$$upstream_symbols"; \
		exit 1; \
	fi; \
```

The remedy is a **symbol-table assertion at build time** (`arm-none-eabi-nm`),
wired into `all:`. Note what it is not: not a source pattern, not a lint, not
something visible by reading a file. This is the strongest single reason to
expect a miss, and it is why the run is worth doing anyway — a hit would mean
per-file source analysis reaches further than anyone here expects.

---

## 2. Two arms

The comparison is the point. Arm A alone would be uninformative, because a miss
could always be blamed on absent evidence.

**Arm A — as an operator would actually run it.** Default clone: submodules
unfetched, `external/libngu` and `external/micropython` empty. Two of the three
contradicting files are physically absent.

**Arm B — best case for the tool.** Submodules fetched at the pinned commits, so
all three files exist in one corpus:

- Coldcard `mpconfigboard.h` — `#define MICROPY_HW_ENABLE_RNG (0)`
- libNgU — `#ifndef MICROPY_HW_ENABLE_RNG` → `#error`
- MicroPython `ports/stm32/rng.c` — `#if MICROPY_HW_ENABLE_RNG` → fallback PRNG

**Arm B is the scientifically interesting one.** A miss in arm A says only "it
couldn't see the evidence." A miss in arm B says something much stronger: that a
per-file scanner does not find a cross-file contradiction **even with every
relevant file on disk**. That result settles the per-file-versus-whole-program
question with data instead of argument, which is the whole reason to spend the
money.

---

## 3. Scope, and why it is not the whole tree

`bcc2c382` carries **458** `.py`/`.c`/`.h` files. At one agent session per file
that is a real bill for mostly irrelevant code (fonts, docs tooling, test
helpers).

Both arms scan a **fixed, named file set** — identical across arms except for
the submodule files that only exist in arm B. The set is chosen to be
*generous toward a hit*: if Loupe cannot find the bug when handed exactly the
files containing it, the negative result is strong.

**Arm A file set** (from the pre-fix tree):

```
shared/random.py
shared/seed.py
shared/xor_seed.py
stm32/COLDCARD/rng.c
stm32/COLDCARD/rng.h
stm32/COLDCARD_MK4/rng.c
stm32/COLDCARD_MK4/rng.h
stm32/COLDCARD_Q1/rng.c
stm32/COLDCARD_Q1/rng.h
stm32/COLDCARD/mpconfigboard.h
stm32/COLDCARD_MK4/mpconfigboard.h
stm32/COLDCARD_Q1/mpconfigboard.h
```

**Arm B adds**, at the pinned submodule commits: libNgU's RNG source and header
carrying the `#ifndef` guard, and `external/micropython/ports/stm32/rng.c`
carrying the `#if` and the Yasmarang fallback.

Scoping is done through the same `exclude_path_substrings` mechanism used for
the Omega run, registered via `POST /v1/repos` (`loupectl repo add` does not
expose `scanner_config`).

**This scoping is a deliberate bias toward the tool, and must be stated in the
result.** A real operator scanning this repo cold would not know to pick these
files — that ranking problem is itself the thing the attack-surface-mapping
idea exists to solve. The experiment answers "can it see the bug when pointed
at it," not "would it have found the bug unaided." The second question is
strictly harder and the answer cannot be better than this one.

---

## 4. Run configuration

Identical to the Omega run so results are comparable, on the same GCE host:

| Setting | Value |
| --- | --- |
| Host | `oa-loupe-scanner-1`, n2-standard-8, Debian 12, `us-central1-a` |
| Loupe | `c94aac5` |
| Agent | `codex` / `gpt-5.5` / effort `xhigh` |
| Sandbox | bubblewrap enabled |
| Reporting | `manual` — nothing files anywhere |
| Verification | **disabled** (see below) |
| Approval gate | enabled |

**Verification is off, deliberately.** Loupe's verify stage is broken —
the worker reports a verdict the server never receives, and every verify job
fails (see
[`2026-07-31-omega-first-scan-preliminary.md`](2026-07-31-omega-first-scan-preliminary.md)
§6). Leaving it on would strand every finding in `validating` and produce no
readable output. Consequence to state in the result: **findings are T1 at best**
— a single model's claim with a patch that applies, never a second opinion.

For this experiment that is acceptable, because the question is *did it report
the defect at all*, not *was the report confirmed*. A verifier can only remove
findings, never add them, so its absence cannot manufacture a false positive
result — it can only make a hit look better than it is, which §5 handles by
requiring the finding's substance to be right.

---

## 5. Scoring rubric — fixed before the run

Each arm gets exactly one of these. Judged on the finding's **substance**, not
its severity label or wording.

### HIT

A finding that identifies the actual defect: that seed/random generation does
not reach the hardware TRNG, **or** that the `MICROPY_HW_ENABLE_RNG` /
`#ifndef` / symbol-resolution chain lets a software PRNG supply key material.

Must name at least one of: the wrong `rng_get` being linked; the `#ifndef`
testing existence rather than value; or key material derived from a
deterministic/timer-seeded PRNG.

A finding that merely says "this file uses randomness, ensure it is
cryptographically secure" is **not** a hit. Generic advice is not detection.

### PARTIAL

A finding that flags the seed/RNG path as **unverifiable** without naming the
mechanism — e.g. *"`ngu.random.bytes()` reaches a library whose source I cannot
read; if it does not use a hardware TRNG this is catastrophic."*

This is the outcome §3 of the prediction document anticipated as the plausible
near-miss. It is worth recording separately because a human triaging it in 2021
*might* have pulled the thread — but it is not detection, and it would very
likely have been dismissed.

### MISS

No finding in the entropy/seed/RNG path, or only generic findings that would
read identically against correct code.

### INVALID

The run failed for unrelated reasons: harness error, provider failure, empty
scan. Record and re-run; do not score.

**Additional data to record regardless of outcome:** total findings per arm,
findings in the entropy path, wall time, and — if measurable this time — token
usage. The Omega run's usage was destroyed by the sandbox's fresh `$HOME`; if
that is not fixed here, say so rather than estimating.

### The headline answer

- **Arm B HIT** → the prediction was wrong; per-file analysis reaches further
  than argued. The most valuable possible outcome.
- **Arm B PARTIAL** → prediction substantially right; the uncertainty-flagging
  instruction does real work and is worth strengthening.
- **Arm B MISS** → prediction confirmed. Per-file source analysis cannot see
  cross-file integration defects even with all files present. This is the
  result that most strongly supports whole-program and symbol-level analysis.
- **Arm A ≠ Arm B** → submodule materialization is decisive, and "fetch the
  dependencies" becomes a hard configuration requirement for any real target.

---

## 6. Protocol

1. Start `oa-loupe-scanner-1` (currently `TERMINATED`; disk retains the Loupe
   build and server data dir).
2. Register the pre-fix target twice, as two separate repos so their findings
   never mix — arm A pinned to `bcc2c382` without submodules, arm B the same
   commit with submodules materialized at the pins in §1.
3. Scan both with verification disabled and reporting manual.
4. Export findings from each, score against §5 **without editing §5**.
5. Write results into a follow-up document — leave this one unmodified as the
   pre-registration.
6. Stop the VM.

---

## 7. Threats to validity

Stated in advance, so they cannot be invoked selectively afterwards.

- **Scoping favours the tool.** §3. The result is an upper bound on real-world
  performance, not an estimate of it.
- **Hindsight favours the tool.** We chose the file set knowing the answer. No
  2021 operator had that.
- **No verification stage.** Findings are unconfirmed; a HIT would still need
  its claim checked by hand before being believed.
- **Single run, single model.** No repeat trials, one model family, one effort
  level. Agent runs are stochastic; a single miss is weaker evidence than a
  single hit.
- **The fix commit is in the repository's history.** The scanned tree is
  `bcc2c382`, before the fix, so the remedy is not present in the worktree — but
  the model may carry knowledge of this widely-reported 2026 incident from
  training. **A HIT is therefore not clean evidence of discovery capability**,
  and a hit must be examined for whether it reasons from the code or recalls the
  incident. A MISS is unaffected by this concern, which makes the predicted
  outcome the more trustworthy one to obtain.

That last point deserves emphasis. This experiment can produce a clean negative
and only a muddy positive. That asymmetry is worth knowing before reading the
result, and it is an argument for eventually repeating this design on a defect
the model cannot have read about.
