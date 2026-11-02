# Owner-approved compiler profile and Mac dictation adoption

The owner approved the bounded shared compiler work cap of **2,097,152**
and finishing Coder for Mac's Parakeet dictation path. The current-source
focused compiler, full app build and offscreen dictation run **pass**. Both
keyboard and mouse holds insert the exact public-recording transcript, Undo
restores the original draft, and all three observations have zero sends and
zero message parts. The fixture took 19.732 seconds including startup and two
recognitions; this is not a key-release latency measurement.

The earlier [diagnostic application](../app/README.md) passed keyboard hold,
mouse hold, exact transcript insertion, Undo and zero sends using its isolated
2M producer. Its source snapshots, artifacts, failures and receipts retain
their original scope. The previous 1M refusal is historical evidence, not an
outstanding request for budget permission.

The adoption changes the shared compiler work cap and paired boundary
fixtures. The exact retained-effect/path capacities and independent sum-scan
bound remain separate. Qualification identifies the adopted Coax source and
trusted seeds, focused development compiler, changed-code checks, full app C
generation, native/Swift link and offscreen application result.
A focused development compiler is not a normal bootstrap or release proof.
Unchanged model/corpus/mutation results are reused through their existing
records rather than repeated.

| Boundary | Adoption result |
| --- | --- |
| Owner approval of bounded 2M cap | Approved; no pending budget-choice request |
| Adopted source and seeds | Canonical checker regeneration passes with unchanged header; current producer/source identities are in [compiler/record.json](compiler/record.json). |
| Changed compiler fixtures | 66 affected groups and 14 selected external-policy groups passed; [record and scope](record.json). Full authority coverage and mutation are not claimed. |
| Current compiler/native boundaries | Current focused host built; [10 independent cases](native/record.json) pass per O0, O2 and ASan/UBSan profile. Normal bootstrap/release qualification is not claimed. |
| Current full app generation and link | Passed with the current-source focused 2M host; [build logs](receipts/dictation-adopt-app-build.stdout). Only the existing Swift warning and signature replacement appear in stderr. |
| Current offscreen keyboard/mouse, draft, Undo and no-send run | [Passed](integration/receipt.json), exit 0, exact drafts, no timeout/truncation and unchanged producer identities during the run. The [PNG](integration/dictation.png) is byte-identical to the historical fixture screenshot. |
| Physical microphone and macOS permission/focus acceptance | Pending owner check in [NEEDS_OWNER.md](../../../../NEEDS_OWNER.md) |

The dependency watcher rebuilt the Parakeet archive; its new identity is pinned
in [record.json](record.json). Model arithmetic is unchanged. Existing finite
model/corpus qualification is reused explicitly; a new 29-recording replay is
not claimed. The app oracle's unchanged source and previously qualified native
runner were reused, while the app executable was freshly built and exercised.

The recorded-audio checks use the public fixture and injected capture. They
do not open a microphone, show a visible window or play audio. Software
completion does not mark the physical/TCC checklist complete. Device selection,
repository vocabulary, real streaming and measured key-release latency remain
future work in the [dictation guide](../../../voice/coder-dictation.md).
