# Independent signer verification, 2026-10-30

Codex / `signer-independent-verification`, supporting #4828. Coordination is
in [issue4828.md](../issue4828.md). No signer, kernel, analyzer or existing
secret-probe source was changed by this work. All inputs are public fixtures.

## Results

The new tooling checks exact signatures and refusal behavior and records
compiled-code secret-flow observations. It does not clear #4828 or approve
production keys.

| Source | Build profiles | Cases | Outcome |
| --- | --- | --- | --- |
| `5e4f553e20` | diagnostic `-O1 -fno-inline`, release optimization `-O2` | vector 0 plus leak control per profile | exact signature; control detected; 217 text reports versus 26 |
| `335fc24472` crypto, snapshot `3eeb3aedd2` | both profiles | 16 per profile | all fixture checks pass; both controls detected |
| `37df0fbb7c` signer declassification/challenge edits | `-O2` | 16 | all fixture checks pass; control detected; wrong expected signature rejected |

The 16 cases are eight official signing vectors (0–3, 15–18), three invalid
keys (zero, group order, maximum), short key views, overlap, message changed
after nonce preparation, unpublished candidate changed before self-check,
and a deliberately leaking key branch. Message lengths cover 0, 1, 17, 32
and 100 bytes. Checks include the expected stage count, actual zero workspace
bytes at the end, and unchanged refused output. Both message and candidate
changes refuse. The latest runner also supplies a deliberately wrong expected
signature: the harness must return failure rather than simply report success.

The nine measurement-tool unit checks pass on macOS and Linux, covering
incomplete/malformed reports, unsupported XML versions, missing controls,
unexpected memory errors, failed fixture semantics, source drift, fixture
inventory, ambiguous seed targets and a seeded build failure that must not
count as leak detection. Valgrind XML protocols 4 and 6 are
supported; the executed tool was 3.26.0 using protocol 6.

The frozen GCC 15.2.0 / Coax 0.24.0 producer is SHA-256
`1de40db57ff1808ac920228e00778f5184b2d9c8e6fd9140dc190de9a44d18bb`.
Tool, compiled source, generated C/header, binary and report hashes are in
the artifacts. Source hashes were checked after generation and after the run.
One generated release C file serves both native profiles in each comparison.
The source-revision label is supplied by the caller; compiled source hashes
are the recorded source identities.

## Secret-flow scope

During signing, the key and auxiliary randomness are undefined under
Memcheck. Only returned statuses are defined between calls. The final
signature/output and workspace are defined after signing solely so the
harness can compare actual bytes. There is **no intermediate declassification
of public key P, R, parity or validity fields** in this independent harness.
It therefore deliberately retains observations that the source review may
justify as declassification.

For both the kernel and latest signer snapshots, the `-O2` valid-vector runs
record 26 XML contexts each: 19 in `sf_normalize`, 3 in `smc_peer_bounded`,
and one each in `sw_key`, `sm_equation`, `ssv_prepare` and `sw_publish`.
These need to be reconciled with the reviewed publication boundaries, rather
than treated as 26 exploitable leaks. The three bad-key runs and message
change have one context each; short-key and overlap refusals have none.
The candidate-change run has 26, and the deliberate leak control has one.

The diagnostic XML runs contain 2,243 contexts per valid vector, except
the 100-byte message with 2,259. This differs from the initial 217-context
text experiment. XML/text grouping, stack context and native build settings
matter; these totals are neither execution counts nor a before/after security
score. In particular, the diagnostic build retains trap-check helpers that
optimized builds can fold away. Raw contexts and settings are preserved.

## Finding 1: the existing gate accepts a seeded secret branch

**The shell gate at `76e08098ca` is insufficient as an acceptance gate.**
It exempts every report whose first frame is `sw_key`, `sm_equation` or
`sw_publish`. In a private generated-C copy, inserting the following at the
start of `sm_equation` preserves signature behavior while adding a direct
branch on the raw key:

```c
static volatile uint32_t review_leak_sink;
if (l_key.data[0] & 1) review_leak_sink++;
```

The **unmodified shell gate** exits zero both times. Baseline: three reports,
one per exempt function. Seeded: four reports, two in `sm_equation`. Both
runs detect the positive control, and both say there is nothing outside the
declared decisions. Thus a working positive control does not validate the
exemption filter.

`seed_gate.py` reproduces this using private gate/probe copies and a compiler
output shim supplying the baseline or seeded C plus the same generated
header. Only debug line directives are removed to retain the `release.c`
locations the shell parser expects. This tests the existing shell gate; it
does not claim to test compiler provenance. The reproduction returns one,
as intended when the gate incorrectly accepts the seed.

The requested correction is to bind each exemption to a reviewed decision
and reject an additional secret branch in those same functions. The current
`sm_equation` report comes from its parity-range admission condition, not
its masked validity assignment. The request and evidence were posted in
the coordination doc in `0c0e73b570`; probe changes remain with its owner.

## Finding 2: whole-signer input exceeds the analyzer byte cap

At `37df0fbb7c`, the compiler-recorded signer load paths contain **29
physical files, 269,195 bytes**. `LA_inputs` still caps the aggregate at
262,144 bytes. The newer 256-procedure table does not remove this earlier
input refusal. The byte inventory pins each path, byte count and SHA-256
against that revision. The audit owner needs a bounded input-cap update
with exact-boundary checks before a whole-signer flow result is available.

## Artifacts and reproduction

- [Baseline text comparison](data/signer-independent-2026-10-30/baseline-comparison.json): source identities, original fixture harness, valid-run text reports and control outcomes.
- [Kernel-step comparison](data/signer-independent-2026-10-30/kernel-step1.json.gz): complete structured contexts and fixture outcomes for 32 runs, with the exact measurement-tool sources embedded.
- [Latest signer comparison](data/signer-independent-2026-10-30/signer-declassification.json.gz): complete structured contexts, fixture outcomes and wrong-signature check for `37df0fbb7c`, with measurement-tool sources embedded.
- [Initial gate negative test](data/signer-independent-2026-10-30/gate-negative.json) and [reusable-tool reproduction](data/signer-independent-2026-10-30/gate-negative-reproduction.json).
- [Signer input inventory](data/signer-independent-2026-10-30/signer-inventory.json).
- [Runner instructions](../../tools/coax-nostr/signer-verification/README.md).

Compressed artifacts are ordinary gzip JSON. For example:

```shell
gzip -dc docs/verification/data/signer-independent-2026-10-30/signer-declassification.json.gz > /tmp/signer-comparison.json
```

No full mutation runs were repeated: this work changes measurement tooling
only. The implementation owners retain their mutation and source-audit
gates. No claim is made about timing, arm64 address traces, Wasm JIT behavior,
independent cryptographic approval, or permission to use production keys.
