# Dictation controller qualification

The Coax controller owns consent/focus/readiness admission, positive request
tickets, audio selection, the silence gate and final text eligibility. Its
native C face allocates and transports opaque state. All calls are serialized
by the host; recognition uses a separate copied clip so cancellation remains
responsive while a model call runs.

The controller retains a one-second ring and selects at most 450 ms of
pre-roll by capture timestamps. A hold below 300 ms is a tap; release includes
200 ms of tail. Complete input is capped at 60 seconds, including pre-roll and
tail. The capture ring can keep warming while an immutable clip is recognized.
Partial source frames emit only their valid resampled prefix; retained history
and emitted-prefix bookkeeping let the continuing input frame finish without
duplicating samples. The Verse's existing resampler and speech likelihood are
reused. Its causal filter delay is retained.

The gate needs at least 160 ms of speech-likely samples and a complete rolling
30 ms window above −50 dBFS. Original Parakeet accepts at most 327520 samples;
the controller partitions longer clips into complete, disjoint bounded
segments without leaving a sub-320-sample final segment. Segment transcripts
are joined in Coax with one space between nonempty strings. This does not
claim whole-context equivalence across segment boundaries.

Only a current, uncancelled, successful **nonempty final** exposes text bytes.
Successful empty ASR becomes NoSpeech. Partial recognition, cancellation,
focus/consent/readiness loss and stale tickets cannot expose a draft candidate.
There is no send operation. Caller copy buffers are unchanged on refusal.

## Actual checks

- Pure policy: 14 checks; **250/250 root mutants killed**, zero survivors or
  equivalent waivers. The initial 243/216/27 observation is retained alongside
  later passing source epochs. Missing boundary/state-reset oracles were added;
  redundant equality branches and duplicate upper-bound checks were removed.
- Audio composition: **20 focused checks pass**, including exact partial-frame
  PCM, existing Verse resampler equality, silence/tap admission, immutable
  recognition input, stale results and final-only byte publication.
- Public native ABI: **O0, O2 and fatal ASan/UBSan pass**. Concrete timelines
  establish exact 14400-sample output for a 900 ms recording with a split final
  frame, untouched undersized copy buffers, invalid tickets, no partial text,
  empty recognition, NaN refusal and a real 960000-sample/three-segment cap.
  LeakSanitizer is disabled on this macOS host; leak checking is not claimed.

A red/green regression found a real late-keyup bug: after automatic capture
finalization, a release could erase a frozen clip or partially assembled text.
Ignored releases now preserve both; only a real held-to-tap/fault transition
clears them. Earlier fixture capacity and code-generation invocation errors
remain as failed observations, never relabeled as passing.

The imported Verse quiet-gap check already documents a 40-million-step test
invocation but lacked its per-check annotation. The only Verse change adds
`#steps 40000000` to that check so C generation can import it. All other source
bytes, including production audio logic, are identical. No compiler or global
analysis budget changed, and no full Voice mutation gate was run.

[record.json](record.json) pins sources, compiler, generated C/header and native
profiles. The source snapshot contains no model weights or executable. Actual
microphone permission/capture, asynchronous model ownership, composer insertion
and real Parakeet recognition have separate host/integration receipts.

```shell
coax/bin/coax test apps/shared/dictation-policy.coax --library
coax/bin/coax mutate apps/shared/dictation-policy.coax --library --root-only --jobs 4
coax/bin/coax test apps/shared/dictation.coax --library --only dictation --steps 40000000
```
