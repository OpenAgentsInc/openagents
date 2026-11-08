# Actor phase compiler qualification

Base: `e705ea346a` on `origin/main`. This is a partial delivery toward #12992,
not a completed actor runtime or realm adoption. The older local #12991
alternative remains on `archive/state-12991-alternate-612e6783ad`; its two
unfinished recovery files were retained outside the checkout. Current main's
`87d9210c3a` state delivery is the integration baseline.

## Behavior checked

The compiler resolves stable dependency IDs, adds the six phase barriers,
rejects cycles and unordered read/write conflicts, and emits a deterministic
flat task order. Channel ownership prevents a presentation task from declaring
gameplay writes. Input admission precedes indexing and no partial plan escapes.
Maximum task/dependency counts, unknown identities, duplicate dependencies,
phase/channel errors and the maximum signed 64-bit ID are checked.

Five Coax checks pass, including all 64 directed three-node graphs under four
access/phase profiles. Their independent oracle enumerates all six possible
execution orders rather than computing transitive closure.

The final root mutation gate kills **137/137 mutants, zero survivors**. The
initial gate exposed missing exact dependency-boundary/refusal assertions and
a strict comparison with equivalent ties. Admission assertions now cover the
full 16-dependency boundary. Canonical selection uses one-based ID ranks,
including each ID itself, and explicit rank priority. No mutant was waived or
marked equivalent. Retained logs: `phases-mutate-initial.txt` and
`phases-mutate.txt`.

## Actual native emitted code

Normal Coax C emission and execution observes **all 4,096 directed four-task
graphs × four profiles = 16,384 results**. The separate Python oracle checks
each against all 24 execution orders, including conflict ambiguity among
otherwise legal orders and exact lexicographic ID priority. Counts:

| Result | Count |
| --- | ---: |
| Complete admitted plan | 1,119 |
| Cycle refusal | 14,691 |
| Unordered access conflict | 574 |

Every refused result contains an invalid zero-length plan and zero task IDs.
The complete observation text SHA-256 is
`b6d644f4a2bc576b9134acf0321a6a892503e67a1d63fb57a67959e26f5bb00b`.
The same emitted C compiled with GCC 15.2.0, `-O1 -g`, ASan/UBSan,
`-fno-sanitize-recover=all`, `-fno-omit-frame-pointer`, `-fno-fast-math` and
`-ffp-contract=off` produces byte-identical observations with leak detection
enabled and no sanitizer diagnostics. Default native stack is unchanged.
This is CPU task-plan qualification, not actual actor execution, Wasm replay,
GPU work, physics, frame timing or allocation qualification.

Owner-local artifacts:
`~/.openagents/aethyrforge/actors-12992/phases-v1/` contains `native.txt`,
the unchanged emitted `native.c`, the instrumented executable and
`sanitized.txt`. The independent oracle command is in the module README.

## Identities

Compiler: Coax 0.52.0, SHA-256
`bd13739490a65b8969c6fb4298ca8f94264416e41ad1a146ca8170fdec8acacc`.

| Source | SHA-256 |
| --- | --- |
| `aethyr/actors/phases.coax` | `dd6de00607940c2eda4d7ed86c3025aed20698545985e947b5fbd18f3e9e48d8` |
| `aethyr/actors/tests/phases.coax` | `9c6802fc3d09439dae0961344fdf183e113839c7fa5b7579bce6eddfec8f6075` |
| `aethyr/actors/tests/phase-orders.coax` | `e5b762cfc81885eb69dc2b9115d84e4080e6b2bd92af2fb5fb4f29676980cd2a` |
| `aethyr/actors/tests/phase_oracle.py` | `e68cc4eeccb225b109c89d634d2c196fa6c2bc3a85c8c0acec202d231a1eb1a1` |

The complete issue remains open. The module README lists the remaining
contracts, runtime, integrations and adoption evidence; no book chapter
status is promoted by this component check.
