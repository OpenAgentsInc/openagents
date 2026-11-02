# Last-success snapshot after a late failure

The [public native oracle](../../../../tests/parakeet-runtime-snapshot.c) uses
only `psionic-parakeet.h` against the pinned, qualified resident library.
It observes one model lifetime with three essential GPU calls:

1. Short clip A succeeds and supplies copied metadata, tokens, JSON, normalized
   features and full encoder frames.
2. A different, longer clip B executes inference, then fails with the exact
   typed text-output status because capacity one cannot hold its text. Caller
   text remains untouched. Every public last-success observation still equals
   A, including its original inference time; feature/encoder arrays are compared
   as copied f32 snapshots. Struct padding is never compared.
3. B succeeds with sufficient capacity and replaces A with its own different
   length, text, tokens and tensor extents.

Too-small stage and JSON copy requests also preserve their caller outputs and
the saved A snapshot. The [receipt](record.json) records a passing zero exit in
16.89 seconds, actual library/compiler/source pins and raw output. The existing
29-clip corpus was not replayed. No product implementation changed.

The first oracle attempt misspelled the diagnostic phase as `transcribe`
instead of the implementation's `transcription`. Its failed receipt and source
are retained; the corrected test preserves the exact expected text status.
No failed result was relabelled as passing. No executable or model is committed.

```shell
cc -std=c11 -O2 -g -Wall -Wextra -Werror -Ipsionic/platform psionic/tests/parakeet-runtime-snapshot.c /tmp/parakeet-qualified/libpsionic-parakeet.a -framework Metal -framework Foundation -lpthread -lm -o /tmp/parakeet-snapshot
/tmp/parakeet-snapshot "$HOME/.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99" psionic/fixtures/parakeet/6930-75918-0000.pcm psionic/fixtures/parakeet/6930-75918-0006.pcm
```
