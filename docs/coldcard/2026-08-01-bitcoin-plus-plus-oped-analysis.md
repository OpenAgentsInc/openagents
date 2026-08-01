# Assessing the Bitcoin++ Coldcard commit-history op-ed

Status: **independent source review of a published opinion article.** This document
does not attribute motive, reproduce the vulnerable firmware, or change incident
response advice.

Subject: Dusty Daemon, [“When `random.bytes()` runs but doesn't
work”](https://insider.btcpp.dev/p/when-randombytes-runs-but-doesnt), published by
bitcoin++'s Insider Edition on 2026-08-01.

Source review:

- Coldcard firmware history through the affected pre-fix revision, including
  [`9f04ac1b`](https://github.com/Coldcard/firmware/commit/9f04ac1b885297627954b9a5c8ec390b02c3bc74),
  [`b18723dd`](https://github.com/Coldcard/firmware/commit/b18723dddb6d751c39978e4364b56b2414f68b47),
  and
  [`37e4af54`](https://github.com/Coldcard/firmware/commit/37e4af5451c260c1e7d429fe8972c4cb5e68ee59).
- libNgU commit
  [`f19de052`](https://github.com/switck/libngu/commit/f19de0527a49e560203102288ae4bc9740a32d96).
- The existing Coldcard and Loupe analyses in this repository.

---

## Executive verdict

The article is strongest as a code-review-process critique. Its two featured
commits really do have nearly content-free subjects—`runs` and `x`—despite adding
or changing more than a thousand lines, including security-critical board and RNG
integration code. It also correctly identifies the decisive call-path mismatch:
Coldcard's custom code implemented the Python-visible `pyb.rng()` path, while
wallet generation moved to `random.bytes()`, whose libNgU backend required the
different global symbol `rng_get`.

The article is not reliable as a complete causal history. It presents a speculative
compiler-error narrative as though the commit history established it, identifies
the May 2021 Mk4 `runs` commit as the origin of the general Coldcard defect even
though the vulnerable Mk3 integration predates it, and says hardware RNG use was
disabled “in every case” when direct Coldcard hardware-RNG paths continued to work.

Its net contribution is therefore procedural, not cryptographic: it gives a vivid
additional example of why security-critical changes need small reviewable commits,
explanatory rationale, mandatory review, and artifact-level checks. It does not
change the feature blast radius, entropy analysis, on-chain evidence, or owner
remediation in the other documents.

---

## What the commit history establishes

### The zero-valued macro predates the vulnerable integration

Coldcard's initial public firmware snapshot from 2018 already contained:

```c
// We have our own version of this code.
#define MICROPY_HW_ENABLE_RNG (0)
```

It also contained Coldcard's custom hardware-RNG implementation. At that point the
zero disabled MicroPython's built-in RNG module so that Coldcard could provide its
own Python-visible RNG and direct `ckcc.rng_bytes` path. The macro was not, by
itself, the vulnerability.

This matters because the article's central reconstruction says a developer probably
set the macro to zero after encountering a duplicate definition. The public history
does not show that sequence. For the original board, the zero and explanatory
comment were already present years earlier.

### The cross-repository failure formed in 2021

The source-supported chronology is:

1. On 2021-01-28, libNgU commit `f19de052` added its STM32 randomness adapter. It
   calls the global `rng_get()` symbol and uses `#ifndef MICROPY_HW_ENABLE_RNG` as
   its safety check. A macro defined as zero passes that existence test. The commit
   subject is `x`; GitHub reports 881 additions and 200 deletions.
2. On 2021-03-01, Coldcard commit `b18723dd` imported libNgU and changed Mk3 wallet
   generation from the direct `rng_bytes(seed)` path to `random.bytes(32)` inside a
   120-file migration. Coldcard's custom RNG file did not provide libNgU's required
   global `rng_get`, so MicroPython's deterministic fallback could satisfy the
   symbol. This is the earlier integration that made the Mk3 seed path vulnerable.
3. On 2021-05-21, firmware commit `37e4af54`, titled `runs`, added the Mk4 board
   port in 1,534 new lines. It carried forward the same zero-valued macro and copied
   the original board's `rng.c` byte-for-byte. It extended the same hazardous
   integration pattern to Mk4; it did not originate the full cross-project defect.

The defect was the composition: a longstanding board configuration, libNgU's
existence-only guard, migration of secret generation to libNgU, a missing exact
global symbol in Coldcard's custom implementation, and an ABI-compatible fallback
provided by MicroPython.

### The call-path distinction is real

The article correctly separates two interfaces that are easy to confuse:

- Coldcard's custom `pyb_rng_get` and `pyb_rng_get_obj` implement the
  Python-visible `pyb.rng()` function.
- libNgU's `random.bytes()` backend calls the C symbol `rng_get()`.

The custom file did not export that second symbol. With MicroPython's normal hardware
RNG branch disabled, its fallback object supplied `rng_get()` and returned Yasmarang
output. This is the same central mechanism documented by Block, Coinkite, and the
existing source review.

---

## Claim-by-claim assessment

| Article claim                                                                      | Assessment                                                                                                                                                                                                                                                                                 |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| The `runs` commit has a five-character message and 1,534 changed lines.            | Substantially correct as rhetoric: the visible subject is the four-letter word `runs` (five bytes if its terminating newline is counted), and GitHub reports 1,534 additions with no deletions.                                                                                            |
| The `x` commit changes about 1,000 lines.                                          | Correct at that precision. GitHub reports 1,081 changed lines: 881 additions and 200 deletions. Its `ngu/random.c` diff introduces the faulty STM32 guard and `rng_get()` dependency.                                                                                                      |
| A low message-to-change ratio demonstrates poor engineering.                       | It demonstrates weak recorded rationale here, but the numeric ratio is not a security metric. Generated files, moves, and mechanical changes can make any universal ratio meaningless. Security-path ownership, review state, diff scope, tests, and build evidence are stronger controls. |
| The May 2021 `runs` commit introduced the Coldcard low-entropy bug.                | Too broad. It introduced the Mk4 board port with the affected pattern. The libNgU guard and the Mk3 migration that routed seed generation through it were already present in January and March 2021.                                                                                       |
| Setting `MICROPY_HW_ENABLE_RNG` to zero caused the bug.                            | Incomplete. The zero was a longstanding intentional configuration that disabled MicroPython's module while Coldcard used its own hardware RNG. It became dangerous only in combination with libNgU's existence check, the missing global `rng_get`, and the migrated consumers.            |
| The developer probably set the macro to zero to silence a duplicate-symbol error.  | Unsupported by the public history. No intermediate failing build, compiler log, or commit records that sequence. The original board had the same macro and custom RNG code in 2018, and the `runs` commit copied the existing `rng.c` exactly. This may be a hypothesis, not a finding.    |
| The zero-valued macro turned off hardware RNG use in every case.                   | Incorrect. It disabled MicroPython's built-in RNG implementation. Coldcard's custom hardware paths, including direct `ckcc.rng_bytes` consumers, remained functional. The failure affected callers routed through the incorrectly integrated libNgU path.                                  |
| Wallet generation used `random.bytes()` rather than the custom `pyb.rng()` object. | Correct and important. The names are similar, but they terminate at different C symbols.                                                                                                                                                                                                   |
| MicroPython's complexity caused the catastrophe.                                   | Opinion, not a result established by the diff. Framework and FFI complexity increased review difficulty, but the demonstrated failures were an integration contract, an incorrect libNgU guard, missing symbol provenance, inadequate review, and absent end-to-end build assertions.      |
| The commit messages reveal that the developer did not understand the code.         | Not established. They prove that the repository records almost no rationale for those large changes. They do not prove a person's mental state, the order of local experiments, or why a configuration choice was made.                                                                    |

---

## What the article adds

The useful new emphasis is not “write longer messages” in isolation. It is that
security-critical history should preserve enough structure for a reviewer to answer:

- Which invariant is this change intended to preserve?
- Which callers are moving to a new randomness provider?
- Which exact symbol must the final artifact resolve?
- What failure was observed before the change?
- Which test or build assertion proves the intended path after the change?
- Who independently reviewed the security-boundary change?

A concise commit can answer those questions, and a long commit can evade them. The
article's character-per-line formula should not become a gate. The defensible gate is
reviewable scope plus explicit security invariants and executed evidence.

The article also reinforces the procedural conclusion already drawn from the Kelbie
postmortem: security-sensitive changes landed without adequate independent review.
That remains the cheapest prevention lesson. A required reviewer for board
configuration, entropy, seed generation, key derivation, and cryptographic adapters
would have been more valuable than a generic requirement for verbose prose.

---

## Effect on the existing OpenAgents analyses

### [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md): one history clarification added

The technical root-cause account remains correct, but it did not say clearly that
the zero-valued macro predated the vulnerable libNgU integration. This review adds
that chronology and records that the repository does not support the op-ed's
duplicate-symbol-error story.

### [Kelbie postmortem analysis](2026-08-01-kelbie-independent-postmortem-analysis.md): no conclusion change

The Kelbie analysis already records the stronger procedural evidence: the four
commits that formed and prolonged the defect had no recorded reviewer, and two had
no pull request. It also identifies `f19de052` and `b18723dd` as the critical 2021
pair. The Bitcoin++ article makes that history more accessible but does not replace
or materially extend it.

### [Wizardsardine feature assessment](2026-08-01-wizardsardine-impact-assessment.md): no change

The op-ed does not trace ancillary feature callers, prerequisites, wallet policies,
or exploit consequences. It supplies no reason to alter the paper-wallet, clone,
USB, Key C, password, teleport, HSM, PIN-shuffle, side-channel, Seed XOR, or
dice/import conclusions.

### [Loupe](../loupe/2026-08-01-coldcard-prefix-experiment-results.md) and [hardening](../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md) documents: no conclusion change

The article reinforces the existing recommendation for mandatory review on
entropy-critical paths. It does not change the Loupe experiment: dependency-complete
source was sufficient for a model to identify the known chain, the default incomplete
checkout missed it, and neither run proved the exact final artifact.

### [Bitcoin-node forensic analysis](2026-08-01-bitcoin-node-forensic-capability.md): no change

The article concerns source history and developer process. It adds no chain data,
fingerprint validation, victim evidence, or attribution evidence.

---

## Bottom line

The Bitcoin++ article should be retained as a forceful review-process critique and
as a clear explanation of the `pyb.rng()` versus `random.bytes()` call-path mistake.
It should not be used as evidence that a developer encountered a duplicate-symbol
error, changed the macro out of frustration, or failed to understand the code. The
repository does not establish those claims.

The actual source history is both less personal and more actionable: a longstanding
configuration became unsafe when a new dependency interpreted it differently and
secret generation moved behind that dependency. Large poorly explained changes and
missing independent review made the integration harder to challenge; missing symbol
and reachability assertions allowed the wrong provider to ship.

No owner-facing recommendation changes. Fixed firmware prevents future weak outputs
but cannot repair previously generated material; affected secrets still require the
feature-specific assessment and migration already documented.
