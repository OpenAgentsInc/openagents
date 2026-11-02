# Discard resident utterance buffers

The dictation controller already discarded its captured clip, but Parakeet's
resident engine retained its raw F64 waveform, scratch and inspection snapshots.
`psionic_parakeet_clear` now explicitly wipes those live owned regions while
keeping the pinned model, vocabulary, allocations, decoder pool and GPU weights
warm. Its additive declaration leaves the public info struct unchanged.

The API locks the engine, wipes full CPU arena capacities and all result fields,
then clears shared GPU work buffers 1–3 under the encoder's lock. Buffer 0 and
the loaded-weight identity remain unchanged. The encoder holds that lock through
command completion, including partial submissions, so no GPU command can race
the wipe. Apple uses non-elidable `memset_s`; the portable fallback writes
through volatile bytes. Closing also wipes owned work before freeing it.

Coder's real recognizer defers clear until its output text has been copied,
including error returns. A focused injected-ABI Swift oracle checks that order
on success and failure. The existing fourteen host-effect cases and earlier
actual-model producer remain historical receipts; their unchanged paths are
reused. No full model corpus or unchanged math gate was rerun.

[record.json](record.json) and [installed.json](installed.json) identify exact
sources, producers and the qualified archive installed by atomic rename.

| Focused check | Result |
| --- | --- |
| CPU owned-memory oracle, O0/O2 | Every poisoned mutable byte becomes zero, immutable weights/vocabulary and caller buffers remain intact; clear is idempotent and invalid handles refuse. |
| CPU oracle, fatal ASan/UBSan | Pass; changed C memory effects are instrumented. |
| Shared GPU-buffer effect oracle, normal and ASan/UBSan | Full work capacities, including odd tails, become zero; outside guards and resident weight/pipeline bindings survive. No GPU math runs in this fixture. |
| One actual Metal recording, twice on one engine | Exact known text before and after clear; same decoder pool; all public snapshots refuse after clear. The original Coax math objects are reused. |
| Swift recognizer clear ordering, normal and ASan | Returned text survives; success and thrown failure both invoke resident clear. No device/model executes in this injected effect check. |

These are live resident-buffer guarantees. Caller-owned input/output, transient
stack/register copies and OS/driver forensic remnants are outside this API's
scope. Cancelling immediately prevents delivery; a bounded in-flight recognition
still needs its immutable input until it completes, after which deferred clear
and buffer release run. No microphone was opened, no private audio fixture was
created, and no audio was written to disk. macOS leak detection is disabled
explicitly; no LeakSanitizer result is claimed.
