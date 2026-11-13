# Bitcoin from scratch: why Coax, where it fits, and what Coax should add

Date: 2026-11-11. Status: analysis. It decides nothing; the owner decides.

This document answers the owner's question: why can Coax help us write our
own Bitcoin implementation from scratch? What does Coax enable that other
languages don't? Where does that matter in a node? What should we add to
Coax to make it better?

It builds on the
[build-everything audit](2026-11-11-build-everything-audit.md) (called "the
audit" below) and the
[implementation survey](chatgpt-pro-implementation-research.md). It reads
Coax from its own sources: [`coax/start-here.md`](../../../coax/start-here.md),
[`coax/spec.md`](../../../coax/spec.md),
[`coax/checked-state.md`](../../../coax/checked-state.md),
[`coax/EFFECTS.md`](../../../coax/EFFECTS.md),
[`coax/memory-safety.md`](../../../coax/memory-safety.md),
[`coax/docs/native-checks.md`](../../../coax/docs/native-checks.md), the
[Coax book](../../../coax/book/) and the compiler under `coax/src/` and
`coax/lib/compiler/`. Every claim about Coax cites a file. Where a search
found nothing, this document says so. The compiler version is coax0 0.52.0
([CHANGELOG](../../../coax/CHANGELOG.md)).

## Contents

1. [Summary](#1-summary)
2. [What Coax is today](#2-what-coax-is-today)
3. [What Coax enables for a Bitcoin node](#3-what-coax-enables-for-a-bitcoin-node)
4. [Where Coax doesn't help, or hurts](#4-where-coax-doesnt-help-or-hurts)
5. [Where Coax fits in a node, layer by layer](#5-where-coax-fits-in-a-node-layer-by-layer)
6. [What to add to Coax](#6-what-to-add-to-coax)
7. [The ten most valuable additions, as issues](#7-the-ten-most-valuable-additions-as-issues)
8. [Corrections to the audit, and what this document didn't check](#8-corrections-to-the-audit-and-what-this-document-didnt-check)

## 1. Summary

**What Coax gives us is enforcement, not features.** Rust, C++ and Go
also have property tests, mutation tools, fuzzers and effect-like
disciplines. Coax is different in three ways:

- **The checks are part of the build.** An `#authority;` file can't compile
  if a check fails, and every procedure in it must run in some check
  (E0380).
- **Integer semantics are exact and the same everywhere.** That holds for
  every build profile and on wasm.
- **We own the compiler.** Anything Bitcoin needs from a language, such as a
  secret type the compiler tracks, coverage counters for fuzzing, or a
  durable-write effect, is a change to code we control.

For a project where most of the risk is "we disagree with Bitcoin Core in a
corner", "we leak a key through timing" or "we release a Lightning message
before its state is on disk", making checks impossible to skip is worth more
than raw speed.

**Coax is strongest today on consensus exactness and parser edges:**

- 84 of the 86 files in `coax/lib/bitcoin/` are `#authority`.
- The consensus chapters ran `coax mutate` with zero survivors: 425 of 425
  mutants killed for chapter 57
  ([record](../../verification/bitcoin-ch57-2026-11-04.md)).
- The arithmetic is checked by default, and its explicit `_wrap`, `_sat` and
  `_checked` forms behave the same in every profile.

**Coax is weakest where a node is weakest:**

- **Speed.** Signature verification is about 45 times slower than
  libsecp256k1.
- **Parallelism.** There are no threads.
- **Constant time.** The language has no secret type.
- **Fuzzing.** No coverage-guided fuzzer exists.
- **State machines.** No model checker exists.

**Coax's strongest tools are not yet used in the money code:**

- Every one of the 2,592 `#effects` declarations in `coax/lib/bitcoin`,
  `coax/lib/crypto` and `coax/lib/lightning` is `#effects(all)`.
- No money file uses `#transaction` or `#persisted`.

Using what already exists is the cheapest win on this list.

**Where Coax fits.** Write consensus, cryptography, codecs, wallet logic,
the signer and the Lightning state machines in Coax, with Bitcoin Core,
libsecp256k1 and the Lightning implementations as test oracles. The audit
already sets that rule. Two layers need help from the language:

- **The hot elliptic-curve and hash kernels** need wide limbs (`mul_wide`
  already exists), carry intrinsics and hardware SHA-256 through named
  intrinsics. They don't need inline assembly.
- **The node's event loop and parallel script checks** need a deterministic
  job runtime.

**The top five additions:**

1. Narrow the effects of money code, and add `durable`, `net`, `fs` and
   `random` capabilities.
2. Secret types checked by the compiler through arrays and loops.
3. `coax fuzz`: coverage-guided fuzzing with a corpus, sharing its targets
   with `#forall`.
4. Deterministic worker jobs.
5. A deterministic simulation host with fault injection.

[Section 7](#7-the-ten-most-valuable-additions-as-issues) has all ten as
issues.

## 2. What Coax is today

This table is the ground truth the rest of the document relies on.

| Feature | Status | What it does, and its limits | Evidence |
| --- | --- | --- | --- |
| `#test`, `#assert` | Built | A check runs once in the compiler's sealed evaluator. It never appears in the product's C. | [book ch11](../../../coax/book/ch11-writing-checks.md) |
| `#forall` | Built | Runs **every** combination of its bindings: bool, enum, integers of at most 16 bits, or `int` ranges with constant bounds. It reports the first failing case as a counterexample. There are no random generators, seeds or shrinking. A product above 2^24 cases is E0400, and an unbounded `int` binding is E0379. | ch11 lines 200–259, 380–422; `spec.md` 1096 |
| `#steps`, `#traversal`, `#run_steps` | Built | Per-check budgets: 20,000,000 evaluator steps by default, 134,217,728 aggregate visits, and 100,000 steps for `#run` (up to 250,000,000). Running out of any of them is E0400. | `spec.md` 1015–1018, 1128–1135; [book ch41](../../../coax/book/ch41-compile-budgets.md) |
| Native checks | Built, default on | A unit's checks are compiled to C at `-O0` with checked arithmetic and run in parallel processes. Any check that fails is rerun in the evaluator, so failure messages come from the evaluator. `COAX_NATIVE_CHECKS=compare` runs both and reports differences. The whole-tree comparison found 479,316 agreements and 0 disagreements. | [native-checks.md](../../../coax/docs/native-checks.md) |
| `#authority` and E0380 | Built | Every compile of the file runs its checks; a failed check fails the build. Every procedure except `main` and `#foreign` must execute in some check. The coverage counted is per procedure, not per branch. | ch11 lines 676–732; `coax/src/errors.cpp` 132 |
| `coax mutate` | Built | Operators: boundary swaps, `==`/`!=` swaps, arithmetic, logical and bitwise swaps, literals ±1, flipped booleans, negated conditions, removed assignments and calls. A mutant that runs past 10 times the original's steps counts as killed. [`mutation-gate.sh`](../../../coax/mutation-gate.sh) blocks the compiler build on a survivor on a changed line. Zero survivors is repository policy (AGENTS.md), not a compiler rule. | ch11 lines 734–977 |
| `coax verify` | **Not found** | `coax/src/main.cpp` dispatches no `verify` command. `verify.cpp` implements `test` and `mutate`. | `coax/src/main.cpp` 2368–2373 |
| Sanitizers | **No flag** | ASan and UBSan run only from external scripts: `BOOK_SANITIZE=1 coax/book/check-examples.sh`, the protobuf suite, and per-chapter records such as `coax/tests/bitcoin/chainstate_sanitized.c`. `memory-safety.md` still lists "run … under UBSan and ASan in CI" as a recommendation. | [book-maintenance.md](../../../coax/book-maintenance.md) 22–25; `memory-safety.md` 306–309 |
| `#effects` | Built (0.52) | Bounds a procedure's **transitive** effects. The capabilities are `reads`, `writes`, `traps`, `io`, `foreign`, `unsafe` and `clock`; `suspends` and `allocates` are reserved. An undeclared effect is refused, with the shortest call path to it. Declarations are required on `#authority`, `#export_c` and `#foreign` procedures. Foreign summaries are trusted. There is no `fs`, `net`, `random` or durable-write capability. | [checked-state.md](../../../coax/checked-state.md) 41–81; [`effect-contract.coax`](../../../coax/lib/compiler/effect-contract.coax) |
| `#transaction { }` | Built (0.52) | An escaping `err` or `none` restores the block's in-memory state. The body may only read, write and trap. "This language block does not make durable writes atomic across a crash." | checked-state.md 83–121 |
| `#persisted`, `coax compat` | Built (0.52) | Marks records as saved values. `coax compat` refuses incompatible changes to published procedures, layouts, effects and persisted fields. It doesn't generate serializers. | checked-state.md 123–173 |
| Integers | Built | `s8`–`s64` and `u8`–`u64`; `int` is `s64`. Arithmetic is checked by default and traps on overflow. Division by zero, `MIN / -1`, out-of-range shifts and failing casts trap in every build. `add/sub/mul_checked` return `T?`, the `_sat` forms saturate and the `_wrap` forms wrap. These named forms give identical results in every profile, in `#run` and on wasm32. `#counter` fields refuse plain `+`. There is no `u128`. | `spec.md` 855–931 |
| `mul_wide` | Built | Takes two `u64`s and returns the exact 128-bit product as `{hi, lo}`. It uses `unsigned __int128` natively and 32-bit partial products on wasm. `muhash.coax` and `blockfilter.coax` use it. There is no add-with-carry intrinsic. | `spec.md` 918–931 |
| `value_barrier`, `secure_zero` | Built | `value_barrier` is an optimizer barrier for masks: an empty `asm` natively and a volatile identity on wasm. `secure_zero` is a volatile wipe of a view. Neither proves constant time. | `spec.md` 933–960 |
| Secret type | **Not found** | "There is no `#secret` language annotation." The only checker is a separate source audit that handles scalar `u32`, `u64` and `bool`, with no arrays or loops. | `spec.md` 961–975; [`secret-flow.coax`](../../../coax/lib/crypto/secret-flow.coax) line 1 |
| Threads, jobs, atomics | **Not found** | "No job syntax, scheduler integration, atomic API or static race checker exists in the bootstrap yet." `job` and `for_parallel` are design only. | `spec.md` §12 (1554–1574); [book ch14](../../../coax/book/ch14-jobs.md) |
| SIMD, inline assembly | **Not found** | CPU vector types are reserved (E0301). The only assembly is the compiler's `value_barrier`. The escape hatch is `#foreign`, a trusted C call. | `spec.md` 2064–2110; `CHANGELOG.md` 381 |
| C emission, wasm | Built | The compiler emits C11. `#export_c` and `coax header` produce headers that `_Static_assert` every struct layout. Wasm is the same C compiled through clang/WASI (`coax/web/wasm.sh`), which the spec calls "feasibility evidence". Floats have one rounding per operation and no FMA contraction. | `spec.md` 1576–1603, 2064–2110 |
| Allocation | Host-owned | Coax code works on fixed arrays and borrowed views, and has no global variables. `New`, `free` and arenas are planned. | `spec.md` 1505–1536 |
| `coax serve` and reuse records | Built | A resident compiler memoizes lexing and syntax per unit by hash. A passed `#authority` check is skipped only when the hash of everything it ran is unchanged. | [book ch46](../../../coax/book/ch46-resident-compilation.md); `coax/src/serve.cpp` |
| Model checking | **Not found** | No TLA+, explicit-state exploration or `state_machine` declaration. "Models" are `#forall` checks over event sequences, such as 8^4 histories. `state_machine` and `#consistency` are stage V4 of the [verification plan](../../verification/2026-10-27-formal-verification-brainstorm.md). | ch11 lines 1107–1140 |
| `coax fuzz` | **Not found** | Planned (stage V3). Bitcoin fuzzing is a Python harness, [`coax/tests/bitcoin/fuzz.py`](../../../coax/tests/bitcoin/fuzz.py), with no coverage feedback. | verification plan §7.6 |
| Differential testing of the compiler | Built, small | [`coax/tools/differential.py`](../../../coax/tools/differential.py) generates programs of at most 64 operations, with no loops, and compares the evaluator and native `-O0`/`-O2` against a Python model. | [verification-differential.md](../../../coax/verification-differential.md) |
| Bootstrap | Partly self-hosted | The lexer, parser, checker and importer are Coax, committed as C seeds. The evaluator, the C emitter and the driver are C++20. `bootstrap-build.sh` rebuilds every seed and compares it byte for byte. `seed-portability.sh` checks that Clang and GCC produce the same seeds. Full self-hosting is planned (R25). | [bootstrap-build.sh](../../../coax/bootstrap-build.sh); [book ch44](../../../coax/book/ch44-self-hosting.md) |
| Agent tooling | Built | `--diagnostics json` with spans and fix edits; `coax explain`; `coax vocab`; E0225 names the Coax spelling of foreign syntax; `coax query` answers from the checked program. `coax drive` doesn't exist; the page driver is `coax/web/tool.sh drive`, which has no Bitcoin use. | `spec.md` 2111–2207; [start-here](../../../coax/start-here.md) |

## 3. What Coax enables for a Bitcoin node

Each subsection names one of the node's real risks and explains how Coax
helps, with evidence. It ends with a rating of the help today and of what
is possible with work we can do in our own compiler.

### 3.1 Consensus exactness and divergence

**The risk.** A node that rejects a valid block stalls. A node that accepts
an invalid one follows a false chain. The btcd bugs (audit §5.12) came from
code that "implemented the BIP" but disagreed with Core at an edge.

**How Coax helps.**

- **The checks can't be skipped.** Every consensus file is `#authority;`, so
  every compile runs its checks and every procedure must be reached by one.
  `script.coax` carries its oracle inside the file: `#forall` checks over
  `sighash.json` rows, BIP-143 examples and Taproot spends (lines
  1546–1648). A change that breaks one of Core's 500 sighash rows doesn't
  compile.
- **Mutation testing proves the checks have teeth.** The chapter 57 record
  kills 425 of 425 mutants
  ([record](../../verification/bitcoin-ch57-2026-11-04.md)). Chapter 58
  kills 115 and 195, and chapter 63 kills 129 and 175, all with no
  "equivalent" marks ([ch63](../../verification/bitcoin-ch63-2026-11-04.md)).
  A rule nobody tests is a rule a mutant survives.
- **Core is the oracle without being a dependency.** The live regtest runs
  ([chapter 57](../../verification/bitcoin-ch57-2026-11-04.md), 35 verdicts)
  and the JSON corpora (all 1,263 rows of `script_tests.json`) put Core in
  tests only. `PLAN.md` lines 20–22 state that rule.
- **The check runs in two engines.** The evaluator and the native check
  binary are two executions of the same source. `compare` mode found 0
  disagreements across 479,316 checks
  ([native-checks.md](../../../coax/docs/native-checks.md)). That is direct
  evidence that our C emitter and the reference semantics agree on the code
  that ran.

**Strength.** Today it is **strong** for rules that have vectors or a
crafted block. Mainnet equivalence is unproven until chapter 80, and Coax
can't prove it; only history and shadow mode can (audit §6.3). With work,
a `coax diff` harness (§6.6) would make Core a standing oracle in the
toolchain rather than a set of Python scripts.

### 3.2 Parser edge cases

**The risk.** "A parser limit is a consensus rule" (btcd PR 1896). A
CompactSize that is too long, a count one past a limit, or a truncated
witness must get Core's verdict and Core's reject reason.

**How Coax helps.**

- Every index and slice is bounds-checked in every build (E0410). A parser
  can refuse, but it can't read past its input.
- The fuzz harness feeds 100,000 mutated inputs to each of seven parsers in
  both checked and release builds. It found 0 crashes and 0 disagreements
  with a Python reference
  ([ch119](../../../bitcoin/book/ch119-fuzzing.md)).
- `#forall` checks every length or position in a small range. Chapter 3's
  "one flipped bit anywhere" check runs 256 cases, one for each bit.

**Strength.** It is **partial**. Bounds checks are strong. Fuzzing is weak:
it has no coverage feedback, uses no Core `qa-assets`, and has no script,
block or descriptor targets (ch119, "Not done"). `#forall` can't reach
values wider than 16 bits except as bounded `int` ranges, and it has no
random sampling. A `coax fuzz` with emitter-inserted edge counters (§6.3)
would make this strong. Coax can do that more easily than most languages,
because we write the emitter.

### 3.3 Integer boundaries

**The risk.** The btcd signed-version bug: a 32-bit field read as signed
compared differently from Core's unsigned comparison under BIP-68 and
BIP-112.

**How Coax helps.**

- **Overflow is never silent.** `int` is 64 bits and checked arithmetic traps
  on overflow by default. The named forms state intent in the source:
  `add_checked` returns `T?`, which the caller must handle before it writes.
  The named forms are "one definition for every profile … so no C undefined
  behavior remains" (`spec.md` 878–905). A release build can't change
  meaning.
- **The version bug class is avoided.** Our code reads the version as an
  unsigned value. `tx_le_vartime` builds the value byte by byte into a
  64-bit `int` (`tx.coax` lines 411–415). The sequence-lock check
  (`locktime.coax` line 93) and `CHECKSEQUENCEVERIFY` (`script.coax` line
  878) compare `tx_le_vartime(b, 0, 4) < 2`. A version of `0x80000000` is
  then 2,147,483,648, which is not below 2. That matches Core's unsigned
  comparison, not btcd's.
- **That is a property of how the code is written, not of the language.** No
  type distinguishes "a 32-bit consensus field read as unsigned". The audit
  is right that no test pins the case (audit §5.12).
- `#counter` fields refuse plain arithmetic (E0840). This suits commitment
  numbers, generation counters and nonce counters that must never wrap.

**Strength.** It is **strong** for overflow, which can't be silent. It is
**partial** for representation, because nothing stops a signed read of a
field Core treats as unsigned. Nominal integer IDs exist (`@id`,
`spec.md` 422–440), and integer-carrier units would add more (§6.15).

### 3.4 Constant-time cryptography

**The risk.** A key leaks through a secret-dependent branch, memory
address or variable-time instruction.

**How Coax helps today.**

- `value_barrier` and `secure_zero` are language intrinsics with defined
  lowering on native and wasm (`spec.md` 933–960).
- `mul_wrap` builds masks without a checked multiply's hidden branch. Chapter
  3 explains why `cb_mask` must use it.
- The `_vartime` suffix is a naming rule the audit tool enforces: 465
  procedures in `coax/lib/bitcoin` carry it.
- The evidence chain is real but sits outside the language:
  - the source audit
    ([`tools/coax-nostr/secret-check/`](../../../tools/coax-nostr/secret-check/README.md));
  - a disassembly scan ([`coax/tests/constant_time.py`](../../../coax/tests/constant_time.py));
  - Memcheck probes for the ECDSA and Schnorr signers, with declassifications
    bound to a line
    ([`ecdsa_sign_secret_probe.sh`](../../../coax/tests/bitcoin/ecdsa_sign_secret_probe.sh),
    including a `PLANT=1` control that must fail);
  - a dudect-style timing harness that detects planted leaks on arm64,
    x86-64 and wasm
    ([ch3](../../../bitcoin/book/ch003-constant-time-rules.md)).

**What's missing.** The source audit handles scalars only, "not arrays or
loops". All field and scalar arithmetic is arrays and loops, so the audit
can't see the code that matters most. `secp_field.coax` lines 3–5 say so:
"No … secret-taint enforcement or target constant-time guarantee is
claimed here."

**Strength.** It is **weak** as a language guarantee and **moderate** as
evidence for two signers. What is possible here is the most distinctive item
in this document. We own the type checker, so a `secret` qualifier that
flows through arrays, loops and calls and refuses secret-dependent branches
and indexes at compile time is ordinary compiler work for us (§6.2). In
Rust or C++ it would need a research tool outside the compiler.

### 3.5 Determinism and reproducible validation

**The risk.** Two of our nodes on different hosts reach different verdicts,
or a release build behaves differently from the tested build.

**How Coax helps.**

- **Same meaning in every profile.** Integer meaning doesn't depend on the
  profile: the named operations, the traps and the casts are defined the same
  in checked, release, `#run` and wasm32.
- **No hidden state or clocks.** There are no globals (E0212). A `clock`
  effect marks every nondeterministic source.
- **Deterministic compiler output.** `bootstrap-build.sh` compares seeds byte
  for byte. `seed-portability.sh` checks that Clang and GCC produce the same
  seeds. The emitter refuses unsequenced calls that once made GCC and Clang
  output differ.
- **Floats are reproducible but irrelevant.** Floats have one rounding and no
  FMA contraction (`spec.md` 1576–1603), but consensus code doesn't use them.

**Strength.** It is **strong** for consensus integer code. It is **partial**
for builds: there is no `coax build --manifest` and no two-builder
attestation (verification plan §7.9). The effect system could prove that
validation reads no clock, but no Bitcoin procedure declares anything
narrower than `all` (§3.6).

### 3.6 Lightning persistence as a checked effect

**The risk.** LDK's `Persist` contract says a channel update returns
*done* only after it is durable, and the channel releases no message
before that. Breaking it can lose a whole channel to the penalty
(audit decision 9).

**How Coax helps today.**

- **`#effects` is transitive and enforced.** A procedure declared
  `#effects(reads, writes, traps)` can't reach I/O, a foreign call or a clock
  through any callee (checked-state.md 41–81). That is exactly the shape of
  "the channel state machine is pure; a driver does I/O" that audit §5.6
  takes from Eclair.
- **`#transaction` rolls a refused transition back in memory.** A channel
  update that fails halfway leaves no partial state.
- **`#persisted` plus `coax compat` refuses an incompatible change to a saved
  record.** That is half of the audit's non-negotiable 10 ("every stored
  record carries a version, and every migration is one-way and tested").
- **The storage discipline is already qualified at file level.** `kvstore.coax`
  commits by append and fsync. `coinscache.coax` line 74 says "Call only after
  the exact prepared frame is appended and synced". The crash tests cut every
  byte of every write: 7,582 cuts for chapter 63.
- **The host has a durable append.** `fs_append_durable` in
  [`coax/lib/sys/fs.coax`](../../../coax/lib/sys/fs.coax) returns only once
  the data is on storage, and the native store uses `F_FULLFSYNC` on macOS.

**What's missing.**

- **The money code doesn't use the effect system.** All 2,592 effect
  declarations in the Bitcoin, crypto and Lightning libraries are
  `#effects(all)`. When 0.52 landed, existing authority procedures were set
  to `all` by default (checked-state.md, "Effects"). So today
  `channel.coax` could call a file write or a clock, and the compiler would
  accept it.
- **No `#transaction` or `#persisted` appears in `coax/lib/bitcoin` or
  `coax/lib/lightning`.**
- **No effect says "durable".** The language can't express "this message
  send must follow that sync". The ordering lives in comments and crash
  tests.

**Strength.** It is **weak** in use today and **strong** in what's possible.
Narrowing the state machines to `reads, writes, traps` is mechanical and
cheap (§7, issue 1). A `durable` capability and a typed "durable receipt"
token (§6.5) would make "no send before sync" a compile error.

### 3.7 Key isolation and signer policy as effect boundaries

**The risk.** A node process with keys in it is one memory bug from a
theft. A signer that signs whatever it's asked is one compromised node from
a theft (audit decision 8).

**How Coax helps.**

- **Effects can prove the signer's reach.** `#effects` can show that the
  policy procedures reach no `io`, `foreign` or `clock`, and that the only
  `foreign` edge in the signer is the entropy source.
- **Mutation testing covers the policy.** A chapter-122 refill rule that no
  check distinguishes is a surviving mutant.
- **One request shape on every host.** The same Coax signer compiles to a
  native process and to wasm, so chapter 18's single request shape (a local
  key, a browser signer, a phone) is one implementation, not three.
- **Within pure Coax, memory errors don't happen.** Spatial safety holds in
  pure Coax (`memory-safety.md`): bounds are checked and no pointer
  arithmetic exists. That lowers the chance that a parser bug in the node
  reads a key, but it isn't process isolation. Foreign calls and the host
  are trusted.

**Strength.** It is **moderate** today, because the pieces exist but no
signer process does. It is **strong** in what's possible: an effect
capability `secret_io` or `sign` that only the signer unit may hold (§6.1)
would make "the wallet holds no private key" checkable instead of a review
item.

### 3.8 One language from wasm to native

**The risk.** A browser wallet or explorer written in another language
disagrees with the node about an address, a sighash or a descriptor.

**How Coax helps.**

- **The same source runs in both places.** The same Coax compiles to C11 for
  the node and through clang/WASI to wasm for the page.
- **The same checks run on both builds.** Each crypto module has a wasm
  driver (`coax/tests/bitcoin/*_wasm.c` and `*_wasm.cjs`: addresses,
  base58, BIP-32, BIP-39, DER, ECDSA, ChaCha20-Poly1305 and more). The book
  qualifies 32 chapters on wasm
  ([README](../../../bitcoin/book/README.md)).
- **Integer meaning is the same on wasm32.** `mul_wide` has a dedicated
  wasm32 lowering.

**Limits.**

- The explorer page shares request and response code
  (`explorer-request.coax`), not consensus code. Chapters 57–63 claim no
  wasm result.
- Field multiplication has separate native and web files
  (`secp_mult_base_web.coax`).
- A browser's JIT makes no timing promise (audit §6.5), so wasm signers suit
  small balances only.

**Strength.** It is **strong** for codecs, addresses, PSBT and verification.
It is **unproven** for consensus in the browser.

### 3.9 Agent-written code, checked mechanically

**The risk.** Most of the 90,000–136,000 remaining lines (audit §3.9) will
be written by agents. An agent's code is wrong in confident ways.

**How Coax helps.**

- **A missing check is a compile error.** In an `#authority` file, a
  procedure no check reaches is E0380.
- **A weak check is visible.** `coax mutate` reports survivors as E0414 at
  the mutated token, in JSON.
- **The toolchain teaches the spelling.** Diagnostics carry fix edits
  (`--diagnostics json`). E0225 names the Coax spelling when an agent writes
  Rust or C syntax, and `coax vocab` answers what a name is.
- **The loop is fast.** The resident compiler and per-check reuse records keep
  the check-first loop short. Native checks cut fivelab's cold compile from
  266 s to 16 s ([native-checks.md](../../../coax/docs/native-checks.md)).

**Limits.**

- Procedure coverage isn't branch coverage. A procedure can be reached
  while a branch inside it never runs.
- The mutation gate blocks only survivors on changed lines, unless
  `COAX_MUTATION_FULL=1`.

**Strength.** It is **strong**, and the best-developed advantage Coax has.
It is the reason this repository could write 56,490 library lines with
61,673 lines of tests in weeks.

### 3.10 Summary of the ratings

| Risk | Today | Possible | The main lever |
| --- | --- | --- | --- |
| Consensus divergence | Strong (vectors, mutation) | Strong plus a standing oracle | `coax diff` (§6.6) |
| Parser edges | Partial | Strong | `coax fuzz` (§6.3) |
| Integer boundaries | Strong for overflow | Strong for representation | Integer-carrier units (§6.15) |
| Constant time | Weak (outside the language) | Strong | Secret types (§6.2) |
| Determinism | Strong (integers) | Strong plus attestation | Build manifest (§6.9) |
| Lightning persistence | Weak in use | Strong | Narrow effects, `durable` (§6.1, §6.5) |
| Key isolation | Moderate | Strong | Capability effects (§6.1) |
| Wasm and native | Strong for codecs | Strong for light validation | Wasm consensus qualification |
| Agent code | Strong | Stronger | Branch coverage and `#forall` sampling (§6.10, §6.11) |

## 4. Where Coax doesn't help, or hurts

### 4.1 Performance

- **Signature verification is about 45 times slower than libsecp256k1:**
  675 µs against 14.9 µs for ECDSA
  ([ch77](../../../bitcoin/book/ch077-faster-verification-the-glv-endomorphism.md)).
  Mainnet has on the order of a billion signatures, so at this speed a full
  validation spends days on signatures alone, even before storage.
- **Most of the gap is algorithmic, not the language:**
  - 10×26-bit limbs instead of 5×52;
  - complete addition formulas;
  - small precomputed tables.
- **What Coax already has for closing it:** `mul_wide` already gives the
  128-bit products a 5×52 field needs, so the audit's step 3 can start now.
- **What Coax costs:** checked arithmetic. Release builds use wrapping, but
  every `add_checked` that stays in a hot loop costs a branch.
- **What Coax lacks for hashing:** SHA-256 is portable code. There is no
  SHA-NI or ARMv8 SHA path.
- **The verdict:** a 3-times target (audit step 3) is plausible in Coax, and
  parity with libsecp256k1 is not without vector intrinsics.

### 4.2 No threads

- **There are no threads, jobs or atomics** (`spec.md` §12).
- **Initial block download is dominated by script checks** that Core runs on
  every core. A single-threaded node is several times slower than it needs
  to be, on top of 4.1.
- **What runs in parallel today runs outside the language.** Native checks
  run in parallel processes, and the GPU compute backend (`spec.md` §15) has
  barriers. Neither is a language feature a node can use.
- **A hand-made workaround is unsafe.** Running several Coax instances over
  `#foreign` threads would bypass the effect and aliasing rules, because
  "Hosts that call Coax concurrently own the synchronization". That is
  exactly the kind of hand-made concurrency we want to avoid in a consensus
  node.

### 4.3 An immature ecosystem

- **No standard collections or allocation inside the language.** There is no
  allocator, hash map or growable array: `New`, `free` and arenas are planned
  (`spec.md` 1505–1536). Every Bitcoin store manages its own caller-owned
  arrays. That is explicit and bounded, but it is why chapter 62's table is
  in memory and linear.
- **No direct platform API.** There is no socket API, `pread`, `pwrite` or
  `mmap` in Coax. Everything goes through `#foreign` into the C host, and
  each of the roughly 1,100 foreign declarations is trusted.
- **No outside reviewers.** No other team has reviewed a Bitcoin
  implementation written in Coax. Every bug we hit, we hit first.
- **Agents learn the language from our tools, not from training data.**
  E0225 and `coax vocab` exist because of this.

### 4.4 Trusting our own compiler for consensus

This is the most serious objection. A consensus bug in our C++ emitter
would make our node diverge while every check passes. That could happen if
the evaluator and the emitter agree on the code the checks ran, but the
release C differs.

**The trusted base.** It is larger than Core's:

- Core trusts GCC or Clang.
- We trust the C++20 evaluator and emitter (`coax/src/`), the Coax-written
  frontend's committed C seeds, and the host C compiler.
- The compiler is at 0.52 and changes daily.

**What already mitigates it:**

- **Two executions per check.** The evaluator and the native check binary
  run each check. 479,316 agreed and 0 disagreed.
- **Self-consistent seeds.** Seeds are rebuilt and compared byte for byte, and
  they are the same under Clang and GCC.
- **Release-build vectors.** The money-path tests run vectors on **release**
  builds. For example, `ecdsa_sign_libsecp.py --mode release`, and
  chapter 119 fuzzes both the checked and the release build.
- **Core in the loop.** Core is the final oracle in shadow mode (audit gate
  N3–N4). A compiler bug that changes a verdict on mainnet shows up there
  as a difference.

**What doesn't mitigate it yet:**

- **Native checks compile at `-O0` with checked arithmetic.** The product
  compiles at `-O2` or `-O3` with wrapping. The `compare` result says nothing
  about the release profile.
- **`differential.py` is too small.** It generates programs with no loops and
  at most 64 operations, so it can't find an emitter bug in a loop over limbs.

**Our position.** Accept the risk, and make it measurable:

- Run the consensus suite on every release-profile build with two C
  compilers.
- Grow the compiler differential to loops and arrays.
- Pin the compiler hash in every verification record, as the records already
  do.

§6.8 proposes this as translation validation for consensus units. Until
gate N3, Core's second verdict covers the rest.

### 4.5 The cost of tooling

- **Every prerequisite is compiler work** that competes with the language's
  other consumers, Everglade and the OpenAgents Metaverse:
  - P1 (threads), P3 (fuzz), P5 (secret types) and P7 (simulation host) in
    the audit;
  - most of §6 here.
- **Checks dominate compile time.** Before native checks, they took 78.8 s of
  fivelab's 92.6 s compile.
- **A large `#authority` crypto unit costs real time to mutate.** The
  chapter 63 record needed 128 workers. Heavy runs must go to remote hosts
  (start-here, pitfall 1).

## 5. Where Coax fits in a node, layer by layer

"Coax" means the layer is written in Coax with its oracle in tests only.
"Coax + intrinsic" means Coax with a named compiler intrinsic that has a
portable fallback checked against it. "Host" means the C host, behind a
narrow `#foreign` surface.

| Layer | Recommendation | Why | What Coax needs |
| --- | --- | --- | --- |
| Hashes (SHA-256, RIPEMD-160, SipHash, ChaCha20) | Coax + intrinsic | Correctness is done (NIST, RFC, Wycheproof vectors). Mainnet IBD hashes every transaction, so SHA-NI and ARMv8 SHA matter. | SHA intrinsics with the portable kernel as oracle (§6.4) |
| secp256k1 field and scalar | Coax + intrinsic; keep 10×26 as reference and wasm path | The 45-times gap is mostly algorithmic. 5×52 and 4×64 need `mul_wide` (exists) and add-with-carry (missing). | Carry intrinsics, limb magnitude checking (§6.4) |
| Verification multiply (wNAF, GLV, tables) | Coax | Public inputs, variable time allowed. Tables can be `#run`-computed constants. | Larger `#run` budgets or `#embed` for tables |
| Signing, ECDH, BIP-32 private, MuSig2, FROST | Coax, behind the signer | This is where constant time matters, and Coax can enforce it. | Secret types (§6.2); the signer capability (§6.1) |
| Script interpreter, sighashes | Coax | Already vector-exact; `#authority`; 1,263 of 1,263 Core rows | `coax fuzz` targets; `coax diff` against Core's interpreter |
| Consensus rules, block validation | Coax, with Core as a standing oracle | The strongest case for Coax: exact integers, mandatory checks, mutation | Narrow effects (`computes`/`reads, traps`); translation validation (§6.8) |
| Chainstate, coins cache, undo | Coax | Crash-qualified by exhaustive cuts | `#persisted` on records; `durable` effect |
| UTXO page store | Coax logic, host I/O | Decision 2: `pread`, `pwrite` and our cache, not `mmap` | Positional I/O in the host (§6.7); fault-injecting host (§6.5) |
| Block files | Coax logic, host I/O | Core's format, already byte-exact | Same as above |
| P2P codecs, v2 transport, handshake | Coax | Pure codecs with vectors | `coax fuzz` with Core's corpora |
| Peer manager, sockets, timers, event loop | Coax state machines, host sockets | The state machines exist; the program doesn't | A deterministic job and event runtime (§6.3); the simulation host |
| Parallel script checks, signature cache | Coax, with deterministic jobs | Verdicts must not depend on scheduling | Jobs with deterministic reduction (§6.3) |
| Mempool, policy, fee estimation | Coax | Pure logic over the UTXO view; policy exactness against Core | `#consistency` checks (the analogue of Core's `-checkmempool`); `coax diff` |
| RPC, REST, notifications | Coax | Already partly built (`explorer-rpc.coax`) | Nothing specific |
| Indexer | Coax | Crash-qualified at small scale | The page store |
| Light client (BIP-157/158) | Coax, native and wasm | The same code on a phone and in a page | Wasm qualification of headers and filters |
| Explorer UI | Coax wasm (exists) | Already built | Nothing specific |
| Wallet engine, descriptors, miniscript | Coax | Pure logic; miniscript's type system suits `#forall` | `#forall` sampling for wide domains (§6.10) |
| PSBT, coin selection | Coax (exists) | Done and fuzzed | Nothing specific |
| Signer process | Coax, separate binary | One implementation for server, phone and browser | Capability effects; secret types; the simulation host |
| Lightning channel state machine | Coax, pure | Eclair's shape: return actions, the driver runs store before send | Narrow effects now; `state_machine` exploration (§6.6) |
| Lightning persistence | Coax logic; the host or Postgres does I/O | Decision 9's contract | `durable` effect and receipts (§6.5); a Postgres wire client in Coax |
| Chain watching and BOLT 5 | Coax | Pure resolution logic plus notifications | The simulation host with reorg injection |
| Onion routing, gossip, pathfinding | Coax | Codecs exist and are vector-exact | `coax fuzz`; deterministic jobs for gossip checks |
| Taproot Assets (MS-SMT, proofs, VM) | Coax | Pure and vector-pinned in `tap-ldk` | Nothing beyond the above |
| Block templates | Coax | Pure | Nothing specific |
| Stratum V2 | Coax, later | Codec plus Noise; the primitives exist | Nothing specific |
| TLS (third-party HTTPS) | Host library (current rule) | Not used by Bitcoin P2P or Lightning | None |
| Entropy | Host, one source behind `entropy.coax` | Coax can't create entropy; it can refuse to substitute | A `random` effect so only `entropy.coax` can reach it (§6.1) |

The only layers this document would not write in Coax are TLS and the
operating system calls themselves. The hot kernels stay in Coax as long as
Coax gains named intrinsics. If one kernel can't reach its target that way,
`#foreign` to C is the escape hatch. That C then needs its own constant-time
and differential evidence, and it breaks the "one language" property for that
kernel. Prefer an intrinsic.

## 6. What to add to Coax

Ranked by how much of the Bitcoin plan each unblocks or de-risks, divided by
effort. "Effort" is rough: S is days, M is one to three weeks, L is a month
or more of compiler work.

| Rank | Addition | Unblocks or de-risks | Effort | Priority |
| --- | --- | --- | --- | --- |
| 1 | Narrow effects in money code, and new capabilities (`durable`, `net`, `fs`, `random`, `sign`) | Lightning persistence, signer isolation, pure consensus | S (narrowing), M (capabilities) | P0 |
| 2 | Secret types checked through arrays and loops | Every signing path; gate W2 | L | P0 |
| 3 | `coax fuzz` with coverage and corpora, sharing `#forall` targets | Parser edges, consensus differential, Lightning messages | M–L | P0 |
| 4 | Deterministic jobs (worker threads for pure batches) | IBD speed; parallel script checks | L | P0 |
| 5 | A deterministic simulation host with fault injection | Lightning L1, the node's crash tests, the signer | M | P0 |
| 6 | `state_machine` declarations with exploration | BOLT 2, fencing, MuSig2 sessions, BOLT 5 | L | P1 |
| 7 | `coax diff`: a standing differential harness against process oracles | Gates N2 and N3; chapter 117 | M | P1 |
| 8 | Translation validation for consensus units | Trust in our compiler | M | P1 |
| 9 | Carry intrinsics, limb magnitude checking and a fixed-width integer library | The 5×52 field; 4×64 scalar; MuHash; Taproot Assets sums | M | P1 |
| 10 | Positional file I/O and a page-store library | Decision 2; every store at mainnet scale | S (host), M (library) | P1 |
| 11 | Hardware SHA-256 and named vector intrinsics | IBD speed | M | P2 |
| 12 | Reproducible-build manifest and attestation | Releasing a node others can verify | S–M | P2 |
| 13 | `#forall` sampling, seeds and shrinking for wide domains | Integer-boundary checks on 32- and 64-bit fields | M | P2 |
| 14 | Branch coverage in `#authority` (beyond E0380) | Agent-written consensus code | M | P2 |
| 15 | `#consistency` checks (Core's `-checkmempool`, `-checkblockindex`) | Mempool, block index, coins cache | S–M | P2 |
| 16 | `#embed` for pinned vectors | Builds that carry their oracle data with a hash | S | P2 |
| 17 | A `--sanitize` build profile | Every money-path suite under ASan and UBSan as a standard target | S | P2 |
| 18 | Integer-carrier units (sat, msat, vbyte, weight) | Fee and amount unit mixing | M | P3 |
| — | Inline assembly | Not recommended: named intrinsics give the speed with a checkable fallback | — | No |
| — | `mmap` as the store's I/O | Not recommended (audit decision 2): errors arrive as signals, and write order is the kernel's | — | No |

### 6.1 Narrow effects, and capability effects for money

**Motivation.** The effect system exists and is enforced, but the money
code doesn't use it (§3.6). And the vocabulary is too coarse to express the
node's three most important boundaries:

- the state machine never touches the disk;
- only the signer reaches keys;
- only `entropy.coax` reaches randomness.

**Design.**

1. **Narrow every money procedure now.** Use `computes` or
   `reads, writes, traps` for consensus, codecs, the channel state machine,
   policy and pathfinding. Keep `all` only on host boundaries.
2. **Split `io` and `foreign` into named capabilities:**
   - `fs` (read files);
   - `durable` (a write that returns after sync);
   - `net`;
   - `random` (the entropy source);
   - `sign` (a procedure that reads a private key).

   A `#foreign` declaration names exactly one of them.
3. **Let a unit forbid capabilities for everything it loads.** A unit-level
   `#forbid(sign, random)` would cover the node and the wallet, and the
   compiler would refuse any path to them.

**Effort.** S for step 1 (mechanical, though each narrowed procedure may
reveal a hidden effect). M for steps 2 and 3 in `effect-contract.coax` and
`src/effects.cpp`.

### 6.2 Secret types

**Motivation.** Chapter 3's rules are checked only for scalars. The field,
scalar, nonce and key code is arrays and loops. Gate W2 needs "the source
rule, compiler-enforced on arrays and loops" (audit §6.5, item 1).

**Design.**

- **The qualifier.** A type qualifier `secret T` for integers, fixed arrays
  and views of them. Secret-ness flows through assignment, arithmetic, calls
  and struct fields. Generics instantiate with it.
- **What the compiler refuses:**
  - a secret `if`, `while`, `ifx`, `&&` or `||` condition;
  - a secret index or slice bound;
  - a secret shift count;
  - a secret operand to `/` or `%`;
  - a secret argument to `_vartime` or to a `#foreign` procedure that isn't
    declared `#ct`;
  - a checked arithmetic operator on a secret. Checked arithmetic branches,
    so secrets must use the `_wrap` forms or `mul_wide`.
- **Declassification.** `declassify(x, "reason")` is the only way out. It is
  bound to its line and listed by `coax query FILE declassifies`, so review
  sees every one, as the ECDSA probe's line-bound declassification does.
- **The evidence stays.** The Memcheck probe and timing harness remain the
  evidence that the C compiler kept the property.

**Effort.** L. It is a type-system extension in the checker and the
generic instantiator. The existing audit tool's error codes (`SC_BRANCH`,
`SC_INDEX`, `SC_VARTIME`, `SC_SHIFT`) and its seeded fixtures become the
test suite.

### 6.3 `coax fuzz`, and deterministic jobs

These two share a dependency, so this section covers both.

**`coax fuzz`: motivation.** Chapter 119's fuzzer has no coverage feedback,
so it rarely reaches deep parser states. Core's value comes from `qa-assets`
corpora and coverage-guided targets.

**`coax fuzz`: design** (verification plan §7.6, made concrete):

- **Targets.** A target is `#fuzz "name" (input: [] u8) { ... }` beside the
  code, like `#forall`. Any `#forall` whose bindings are integers can also run
  as a fuzz target over random values outside its exhaustive range. That
  makes the counterexample format and the target the same thing.
- **Coverage.** The emitter inserts edge counters, the native-check machinery
  already compiles units to C, and a Coax driver mutates and keeps a corpus
  of inputs that reach new edges.
- **Crashes become fixtures.** Each crash is minimized and written as a
  `#test`.
- **Corpora.** They live in a data repository with pins, as `qa-assets` does,
  seeded from Core's corpora.
- **Sanitizers.** A clang `COAX_CC` adds ASan and UBSan.

**`coax fuzz`: effort.** M–L.

**Deterministic jobs: motivation.** Script checks dominate IBD (audit P1).

**Deterministic jobs: design.** Start with the narrowest useful form:

- `for_parallel i: 0..<n` over a body whose effects are `reads, traps` plus
  writes to a disjoint slot `out[i]`. The checker proves this from the
  effect and view rules; it is `spec.md` §12's "disjoint range validation"
  for the simplest case.
- A fixed worker pool in the host.
- Reductions in index order, so the result can't depend on scheduling.
- A trap in any worker stops the batch.

That alone is enough for chapter 76's script checks. General `job`
syntax, atomics and async come later.

**Deterministic jobs: effort.** L. The acceptance test is the audit's: the
same verdicts with 1, 2, 4 and 8 workers, and ThreadSanitizer clean on the
generated C.

### 6.4 Big integers, carries and hardware kernels

**Motivation.**

- A 5×52 field needs `mul_wide` (exists), add-with-carry and
  subtract-with-borrow.
- libsecp256k1 tracks each field element's **magnitude**, its bound before
  normalization. In C that is a debug-time assertion. In Coax it could be
  checked at compile time.
- SHA-256 needs SHA-NI and ARMv8 SHA.

**Design.**

1. **`add_carry` and `sub_borrow`.** `add_carry(a, b, carry_in) -> {sum, carry}`
   and `sub_borrow`, for `u64`, lowering to `__builtin_addcll` or
   `_addcarry_u64` natively and to explicit compares on wasm.
2. **A `U256` library and limb bounds.** A fixed-width unsigned library
   (`U256`, `U512`) built on them, for MuHash, chainwork and Taproot Assets
   sums. A `@limbs(bits=52, magnitude=M)` annotation that `#run` checks, so
   a field addition that could exceed its magnitude bound is a compile error.
   `secp_field.coax` already keeps "`#run`-checked bound tables"; this
   generalizes them.
3. **Named hardware intrinsics with a portable twin.** Start with
   `sha256_rounds(state, block)` that uses SHA-NI or ARMv8 SHA when the
   target has it. Its portable Coax twin is the oracle in a `#forall` over
   the NIST vectors. CPU vector types follow the same rule later.

**Effort.** M for items 1 and 2, M for item 3. Inline assembly is not
recommended. It would bypass the effect system, the secret checker and the
evaluator, the three things that make Coax worth using for this code.

### 6.5 Durable writes, and a simulation host

**Durable writes: motivation.** Non-negotiable 3 says "no authoritative
write is acknowledged before it is durable". Today it is enforced by
comments and crash tests.

**Durable writes: design.**

- `durable` is a capability (§6.1).
- A durable write returns a `Durable(T)` receipt token. Only the host's
  sync path can construct one, in the way `Handle(T)` is constructed only
  by its store.
- The procedure that releases a message, `send(peer, msg, proof: Durable(ChannelState))`,
  takes the receipt for the state that justifies it. A send without the
  receipt doesn't type-check.
- A recovery-only store returns values of a type that can't construct a
  live channel. That is decision 9's rule, expressed in types.

**Durable writes: effort.** M.

**Simulation host: motivation.** Lightning and node tests need thousands of
reproducible crash and reorder schedules (audit P7).

**Simulation host: design.**

- A host that implements the `fs`, `durable`, `net`, `clock` and `random`
  foreign surfaces from a seed.
- It can fail, shorten, reorder or lose any call, and it can crash at any
  write.
- Chapter 63's per-byte cut method becomes a host mode instead of a
  per-chapter harness.
- Because the effects are named (§6.1), the simulation host is complete by
  construction: a program can't reach a real disk around it.

**Simulation host: effort.** M. The existing agent clock
([agent-clock](../../metaverse/agent-clock.md)) is the same idea for pages.

### 6.6 State machines and `coax diff`

**`state_machine`: motivation.** BOLT 2's commitment dance, fencing,
MuSig2 sessions and BOLT 5 resolution need exploration of interleavings and
crashes. Audit §6.4 lists seven models.

**`state_machine`: design.** This is the verification plan's §7.5:

- A declaration with states, events and `#invariant`.
- `coax test` explores several actors and crashes up to a stated depth and
  prints the shortest failing trace.
- The trace is kept as a `#test`.

Today's substitute is `#forall` over event sequences (8^4 histories in
ch11). It works, but it is limited by the 2^24-case cap and can't dedupe
states.

**`state_machine`: effort.** L.

**`coax diff`: motivation.** Chapter 117 and gates N2 and N3 need a standing
comparison with Core on generated inputs.

**`coax diff`: design.**

- `#oracle "core-script" process "..."` declares an oracle process with a
  line protocol, and `coax diff TARGET --oracle NAME` feeds the same fuzz
  inputs to our procedure and to the oracle.
- Verdicts and reject reasons are compared.
- Each difference is written as a pinned regression vector.
- Oracles run only in test builds, which `#effects` can enforce: an oracle
  procedure is `foreign`, and product units forbid it.

**`coax diff`: effort.** M, after `coax fuzz`.

### 6.7 Positional I/O and a page store

**Motivation.** The host has only append-only files and whole-file reads
(`fs.coax`, `store-file-native.c`). Decision 2's page store needs random
reads and writes.

**Design.**

- **Host calls:** `pread`, `pwrite`, `fdatasync`, `F_FULLFSYNC`,
  `fallocate` and directory `fsync`, each under the `durable` or `fs`
  capability.
- **A page store in `coax/lib/store/`, not `coax/lib/bitcoin/`:**
  - fixed pages;
  - a checksum per page;
  - chapter 62's log as the write-ahead log;
  - a cache over caller-owned arrays.

  Bitcoin is its first user, and Lightning's channel log and the indexer
  are its next.
- **No `mmap` binding.**

**Effort.** S for the host calls, M for the library.

### 6.8 Translation validation for consensus units

**Motivation.** §4.4. A bug in our emitter's release path is the one
failure every other check trusts.

**Design.**

1. A `#consensus` unit marker (or a build profile) makes `coax build` emit
   the unit's checks a second time at the **product's** optimization and
   arithmetic profile, and run them natively. Today native checks run only
   at `-O0` with checked arithmetic.
2. The release check binary is built with two C compilers (Clang and GCC),
   and their results must agree.
3. `differential.py` grows to loops, fixed arrays, `mul_wide` and the
   `_wrap` forms. Those are the constructs the field code uses.
4. Every verification record pins the compiler's source digest
   (`coax source-digest`), as the chapter records already do.

**Effort.** M. Full self-hosting (R25) shrinks the trusted base further
but doesn't replace this.

### 6.9 to 6.15 in brief

- **6.9 Build manifest** (verification plan §7.9). `coax build --manifest`
  records the hashes of the sources, the `#embed` inputs, the generated C,
  the C compiler and the binary. Two builders compare manifests before a
  node release. S–M.
- **6.10 `#forall` sampling and shrinking.** Today a binding above 16 bits
  needs a bounded range. Add `#forall ... sample N seed S` over full `u32`
  and `u64` domains that always includes the boundary values (0, 1, max,
  max−1, `0x80000000`), and shrink the counterexample toward zero, as
  `differential.py`'s reducer does. This makes the btcd test class one
  line. M.
- **6.11 Branch coverage.** Extend E0380 from "every procedure ran" to
  "every branch arm in an `#authority` unit ran", with
  `// unreachable: reason` as the reviewed exception. M.
- **6.12 `#consistency`** (verification plan §7.4). A whole-structure check
  that test builds run every N mutations. It is the analogue of Core's
  `-checkmempool` and `-checkblockindex`. S–M.
- **6.13 `#embed`** (verification plan §7.7). Vectors as pinned constants
  with their hash in the build, instead of Python parsing JSON. S.
- **6.14 `--sanitize`.** A standard build profile that compiles the
  generated C with ASan and UBSan and runs the unit's suite. Chapters 57–63
  already did this by hand. S.
- **6.15 Integer-carrier units.** The quantity system admits only `f32` and
  `f64` carriers (`spec.md` 518–562). Admit `u64` and `s64` carriers for
  sat, msat, vbyte and weight units, so a fee computed in msat can't be
  added to an amount in sat. M.

## 7. The ten most valuable additions, as issues

Each is one issue. Issues 1, 5 and 9 can start today with no compiler work
beyond the stated change.

### 1. Narrow the effects of all Bitcoin, crypto and Lightning code

Replace `#effects(all)` with the narrowest true set across
`coax/lib/bitcoin`, `coax/lib/crypto` and `coax/lib/lightning`.

Acceptance:

- No procedure outside a named host-boundary file declares `all`.
  `grep -c "#effects(all)"` over the three directories reports only those
  files, and the list of host-boundary files is in the issue.
- `channel.coax`, `commitment.coax`, `validation.coax`, `script.coax`,
  `sighash.coax`, `tx.coax` and `policy.coax` declare at most
  `reads, writes, traps`.
- A seeded `fs_append` call inside `channel.coax`'s transition procedure
  fails compilation, and the error's call path names it.
- Every unit's tests pass unchanged, and `coax mutate` on the touched
  `#authority` files has zero survivors.

### 2. Secret types through arrays and loops

Add `secret T`, `declassify` and the refusals of §6.2.

Acceptance:

- Chapter 3's leaky-branch and leaky-index fixtures are refused at compile
  time with stable error codes, including when they sit inside a loop over
  limbs.
- `ecdsa_sign.coax`, `schnorr_sign.coax`, `secp_mult_const.coax` and
  `bip32.coax` compile with their key and nonce inputs typed `secret`. Every
  `declassify` is listed by `coax query FILE declassifies`, and each matches
  a line-bound declassification in the Memcheck probe.
- A planted secret branch in `secp_field.coax` fails compilation.
- The checker, with the feature, has zero mutation survivors in the new
  code.

### 3. `coax fuzz` with coverage feedback

Acceptance:

- `#fuzz` targets compile with emitter-inserted edge counters, and the
  driver keeps a corpus directory.
- Planted-bug test: a bug behind three nested byte comparisons is found
  within a stated budget, which chapter 119's fuzzer fails to do in the same
  budget.
- The seven chapter-119 targets plus script, block, descriptor and BOLT 1
  message targets run nightly, seeded from Core's `qa-assets`.
- Each crash is minimized and written as a `#test`.
- With `COAX_CC=clang`, targets run under ASan and UBSan.

### 4. Deterministic parallel batches

`for_parallel` over disjoint output slots, with a host worker pool and
reductions in index order.

Acceptance:

- The checker refuses a body that writes outside `out[i]` or declares any
  effect beyond `reads, traps` and its slot.
- Chapter 76's script checks over a pinned block range give the same
  verdicts with 1, 2, 4 and 8 workers.
- ThreadSanitizer reports nothing on the generated C.
- Throughput is at least 6 times on 8 cores.

### 5. The simulation host

Acceptance:

- A host with `fs`, `durable`, `net`, `clock` and `random` from one seed.
  It can crash at any write and can fail, shorten or reorder any call.
- One seed replays a run byte for byte.
- Chapter 63's cut tests run through it and find the same 7,582 cuts
  recover.
- `channel.coax`'s open-pay-close between two in-process nodes runs under
  10,000 seeds with random crashes. Both nodes finish with the right
  balances or detect `LNH_BEHIND`.

### 6. Durable receipts and the persistence contract in types

Acceptance:

- `durable` is a capability, and `Durable(T)` receipts are constructible
  only by the host's sync path.
- The Lightning driver's send procedure requires the receipt of the state it
  depends on. A seeded send-before-sync fails compilation.
- The recovery-only store's reader returns a type that can't construct a
  live channel (a compile-time refusal test).
- Records in the channel store are `#persisted`, and `coax compat` runs on
  them in CI.

### 7. Positional file I/O and the page store

Acceptance:

- The host offers `pread`, `pwrite`, `fdatasync`, `F_FULLFSYNC`,
  `fallocate` and directory `fsync`, each with one named capability.
- `coax/lib/store/pagestore.coax` passes the audit's step 1 criteria: it
  matches a reference map, recovers at every write cut under the simulation
  host, loads 200 million entries within 8 GB, and has zero mutation
  survivors.

### 8. Carry intrinsics and the 5×52 field

Acceptance:

- `add_carry` and `sub_borrow` for `u64` give identical results in the
  evaluator, native C at `-O0` and `-O2`, and wasm32 on a `#forall` over
  boundary values.
- A 5×52 field and a 4×64 scalar agree with the 10×26 path on every existing
  field check, and with libsecp256k1 on 10 million random and edge inputs.
- ECDSA verification is within 3 times libsecp256k1, measured as chapter 77
  measures.

### 9. Translation validation for consensus units

Acceptance:

- A `#consensus` marker makes the build run the unit's checks natively at
  the product's profile (`-O2`, wrapping), as well as at `-O0`.
- The release check binary is built with Clang and GCC, and both agree on
  every check in `validation.coax`, `script.coax`, `sighash.coax`, `tx.coax`
  and `chainstate.coax`.
- `differential.py` generates programs with loops over fixed arrays,
  `mul_wide` and the `_wrap` forms. A planted emitter bug in loop lowering
  is found within one run.

### 10. `state_machine` with exploration

Acceptance:

- A declaration with states, events, `#invariant` and `#eventually`. The
  compiler checks that every state is reachable and every event is handled.
- `coax test` explores two actors with crashes to a stated depth and prints
  the shortest failing trace.
- The fencing model of audit §6.4 is written with it. A seeded bug that
  lets a stale owner sign is found, and its trace becomes a `#test`.
- The BOLT 2 commitment model of audit §6.4 runs to depth 12 in minutes.

## 8. Corrections to the audit, and what this document didn't check

| The audit says | Found |
| --- | --- |
| P6: add "a 64×64→128 multiply the C backend maps to `__int128` or `umulh`" | `mul_wide` already exists (`spec.md` 918–931) and is used in `muhash.coax` and `blockfilter.coax`. What is missing is add-with-carry and the 5×52 field itself. |
| P4: sanitizers "as a standard target" | Confirmed missing as a compiler flag. They run only from scripts. |
| Coax effects are not mentioned | `#effects` is enforced and transitive (0.52), but every money procedure declares `all`. This is the cheapest gap to close (issue 1). |
| `coax fuzz` is planned | Confirmed: no `fuzz` command in `coax/src/main.cpp`. |
| Coax has no model checker | Confirmed. `state_machine` and `#consistency` don't exist in the compiler. |

Not checked:

- **Generated C performance.** Checked arithmetic costs speed beyond what
  chapter 77 measures, and this document didn't measure how much.
- **The GPU backend.** It could batch-verify signatures or hash on a GPU
  (`spec.md` §15), but this document didn't evaluate whether that suits a
  consensus path. It would need its own differential and is not
  recommended before CPU parallelism.
- **The effort estimates.** They are judgements, not measurements.
