# System One owner prerequisites — 2026-11-01

Bounded implementation for [#12625](https://github.com/OpenAgentsInc/openagents/issues/12625),
after delivery of #12624. This does not close the language/ownership umbrella.
The [execution map](../../books/coax-outcomes-request-ownership.md) preserves
existing R2, B2 and B3 task identities and the separate R0 gates.

| Change | Observed checks | Scope |
| --- | --- | --- |
| `so_start` checks admission before overwriting the owner | [Transport receipt](transport.json): policy debug/release checks, **2/2 focused mutants killed**, actual generated transport at O0/O2 plain and ASan/UBSan | Empty call view refuses; only initialized/retired handle `-1` starts. Active/invalid refusal preserves named state, descriptors, results, backing bytes and process/environment effects. |
| Native process lifecycle | [Process receipt](process.json): four native modes link actual `process-native.c` and an owned fixture child | Success and exit-28 stderr; a sleeping child is cancelled and reaped; duplicate terminal polls/cancels cannot release a subsequently reused unrelated slot. Caller-owned borrowed bytes remain valid through observed uses. No HTTP service or key is used. |
| Desktop root construction | [Factory receipt](factory.json): six native cases at O0/O2/ASan+UBSan, exact generated defaults and production C factory/quit functions | Each successful allocation calls the actual generated constructor once; failed allocation skips it. Exact complete state/default declarations plus an owner-ready attachment projection prove never-started quit preserves unrelated slot zero. Full `zr_attach` is not established. |
| Lab early refusal | [Lab receipt](lab.json): exact production `lab_ask`, complete authored types, five active/invalid handles, O0/O2/ASan+UBSan | All named fields, descriptors and guarded bytes remain unchanged. Nine nonactive-path dependencies are signature-matched fatal test stubs; this qualifies the early-return boundary, not a new complete Lab request. |

The factory replay uses the original C factory; the isolated missing-init and
slot-zero checks fail, and the fixed factory passes. The Lab guard-removed
function matches the actual `a2e4931045` function byte for byte and exits at the
forbidden test boundary; its guarded counterpart returns without touching it.
The transport's original active-slot-zero behavior also fails the new native
oracle. Setup/compiler refusals and fixture corrections are retained separately
from these behavioral failures.

The transport producer is retained Coax image `f7eb9551…`; the final factory
and Lab projections use qualified H `6018024e…`. These are observations on
those explicit images, not qualification of a newly built compiler. The four
scripted native runs are retained across later shell-only fixes; the
[continuity record](transport-continuity.json) identifies exactly that drift
and its selector/default/path/exclusivity checks. It does not say the final
shell script produced an older observation. The later real-process failure
fixture additionally verifies cleanup when its own harness deliberately exits
after starting the sleeping child.

Full desktop `settings.coax` generation refused with E0381 on three observed
images; the full Lab `#test` command also refused before its new check ran.
Those are not passing application checks. The subset emitter copies complete
actual type/default declarations and records their hashes/offsets; it does not
substitute a smaller state or a handwritten constructor. The owner-ready
projection uses actual relevant initialization and `lab_attach` statements,
not the complete attachment/configuration path. Native padding is not compared.

[proof.tar.xz](proof.tar.xz) retains text sources, generated C/headers, raw logs,
failures and receipts. [The manifest](proof-manifest.json) maps each original
private path to its archive member, size and SHA-256; all members were read back.
Copied receipts retain their original bytes and paths. Executable/object/dSYM
bytes are omitted; their observed hashes remain in the original records.
Some historical identities were observed after execution and say so; they are
not retroactively labeled before-captured manifests or execution attestation.

Reproduce only the intended slice, using an existing qualified compiler and
new output directories:

```shell
COAX=/absolute/path/to/qualified/coax \
  coax/tests/systemone-lifecycle.sh /tmp/systemone-scripted
coax/tests/systemone-process-lifecycle.sh \
  /tmp/systemone-process /tmp/systemone-scripted
python3 apps/openagents-desktop/tests/check_zeron_defaults.py \
  --coax /absolute/path/to/qualified/coax --out /tmp/zeron-defaults
python3 apps/openagents-desktop/tests/lab-owner.py \
  --coax /absolute/path/to/qualified/coax --out /tmp/lab-owner
```

The compiler's owned release workflow now includes the small admission-policy
and transport/process gates. No full compiler/bootstrap/model gate or broad
mutation run was performed here. The app subset commands remain explicitly
scoped checks; they do not conceal the full-unit refusal.

Remaining: deliver the existing R2 compiler/synchronous adapter, precise B2
helper composition, typed asynchronous outcomes, and complete B3 consumer and
backing-buffer evidence on the qualified integrated compiler. The app-life
pool, copied-active-owner/handle transfer, foreign retention and general
lifetimes remain outside this slice. Device, budget/review and golden approvals
remain in [NEEDS_OWNER.md](../../../NEEDS_OWNER.md#r0-adoption-gate).
