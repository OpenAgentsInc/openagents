# Actual receive faults and retained-source replay — November 7, 2026

**#12990 remains open.** Five actual browser receive histories now replay
through native CPU codecs/allocation with identical canonical state after
every paid step. The streaming runtime, original owners and executed
browser Wasm are unchanged. The shared
[recorded-source admission](../../../aethyr/host/recorded-source.md) adds
portable exception payloads and exact metadata parsing to the offline
replayer; it adds no allocator or asset publication path. Source starts at
local checkpoint `d3d18af033`.

## Actual histories

Muted headless Chrome 152 runs pinned corridor-v2 sources on coderos-4080's
RTX 4080, browser backend, visual tier 0. Each history has 32 four-step
frames and three cleanup ticks. All 131 paid-step SHA-256 values match
native replay, including failure, retirement and retry.

| History | Failed owner | Attempts and final retirements | Allocation/free events | Browser/native CPU peak | Captured source bytes |
| --- | --- | --- | --- | --- | --- |
| Corrupt raw | 5 | 19 | 110 | 12880 | 24 |
| Oversized raw | 5 | 19 | 110 | 12880 | 24 |
| Cancelled raw | 5 | 12 | 72 | 11000 | 24 |
| Reordered raw | None | 18 | 104 | 12880 | 0 |
| Corrupt VPK | 0 | 19 | 112 | 12908 | 80 |

The corrupt raw allocation has bit 0 of byte 0 changed. The oversized
response offers 25 bytes to a 24-byte transfer. The cancelled stream offers
a complete valid 24-byte prefix but never ends: its reader is still live
before close and actual ReadableStream cancellation finishes. Both leave
their retained resource source entirely zero. A received prefix is not
copied before whole-transfer admission. The failed generation never becomes
ready; retries become ready only after physical retirement.

In the reordered history, body owner 6 completes before collision owner 5.
Readiness still waits for its dependency. The independent oracle walks the
manifest dependency table and requires every ready dependency to remain
live. In the VPK case, a changed byte outside the texture triggers the
actual error `Coax refused complete content identity`. The texture's
80-byte allocation stays zero: complete-container authentication preceded
its entry copy.

Every allocation/free extent agrees with the pinned RCM1 model. CPU,
inbox, audio, image and GPU owners drain. The pack host ends with its
8 MiB master and no operations/readers/SDK effects. Logical resource bytes,
transport arenas, diagnostic strings, hidden browser/driver backing and
process RSS remain separate. This stage does not finish transport/transient
accounting.

## Versioned source observations and integer identity

The optional `aethyr.recorded-source.v1` extension supplies both
sourceSchema and sourceSnapshots. It allows 64 snapshots of at most 4,096
bytes, bound by owner/generation/epoch and all eight words of the accepted
completion's source SHA-256. These are retained allocation bytes, not
network packet recordings. The [profile](../../../aethyr/host/recorded-source.md)
specifies admission.

The recorder observes the unchanged Wasm completion export before the SDK
face finishes. It calls the original export exactly once; observation
failure poisons the test receipt while the original SDK completion still
runs. The native reader authenticates the complete payload in bounded
scratch before copying any output. The SDK still checks the actual
allocation's full key and extent. Shared decoding sees those bytes, and
the recorded SDK result enters the normal paid-step gate. Recording a bad
span does not admit it as an asset.

Unknown/missing versions or tables, non-array tables, excessive counts,
wrong identities/lengths/spelling, duplicate matches and unconsumed
snapshots refuse. Earlier v2 histories without this extension remain
readable: the previous actual GPU upload-failure history passes the new
instrumented executable. Unversioned prototype exception tables are not
silently upgraded.

The complete shared-module root gate kills **76 mutants, zero survivors**,
retaining all imported checks. The initial 33-mutant gate left two:
a limit error was hidden by a smaller output span and a skipped first byte
by zero-valued vectors. Exact-capacity refusal and a nonzero first byte
sharpened the checks; no equivalent annotations were used. Unchanged
coordinator 958/958 and serializer 185/185 results are reused.

The identity audit replaced swr_word's binary64 conversion with checked
signed-decimal parsing. Checks cover 2^53+1, both signed-64 endpoints,
every single-byte digit classification, malformed spellings and overflow.
A separate native Coax tool emits sixteen observations independently
compared with Python arbitrary-precision integers. The JSON reader bridge
also checks the endpoints. Player coordinates retain their binary32 path.

## Native replay and hostile histories

The current native executable uses ASan/UBSan. Positive histories drain
and produce no sanitizer diagnostics. CPU allocation/decoding are real;
the native observer replays browser receive/SDK outcomes and retained
spans. It does not re-run native HTTP fault injection or create GPU objects.

Independent transcript reconstruction gives:

| History | Observation SHA-256 | Altered histories rejected |
| --- | --- | --- |
| Corrupt raw | `d421f07ade1770cc05532646bfa3c091eb3bfabe011df05cef822f04bc7701ec` | 18 |
| Oversized raw | `1f4aac11a7b0a3d25b06c126c05d0192ea2c860c25b01930e0bc25ccb73bf617` | 18 |
| Cancelled raw | `70a8c3bae7f3ced0f0ed3f94c3746da4afa41436794d281f1bb128e8983dca5f` | 18 |
| Reordered raw | `512adc2aedc146cccd4636d3973fb27c9b26e828b2f93aea81c38c3aa4f6ee41` | 10 |
| Corrupt VPK | `b1b3c001d91f3eabc247c62be011365737c41b7f782a91cacdf76e4e38dd931d` | 18 |

The 82 negative histories cover changed positions/generations, missing
completions, altered/omitted/reordered paid-step marks, unsupported/missing
versions/tables, wrong table type, missing/edited/miskeyed payloads,
duplicate/orphan payloads, bad hex, wrong length and capacity overflow.
Each requires exit 1, no result report and no unexpected stderr. Only
these intentionally interrupted negative children disable leak checking;
memory-error checking stays enabled.

## Evidence and remaining exits

Shared artifacts under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/` are
`stream-walk-source-{corrupt,oversize,cancel,reorder,pack}-v1.json`,
`native-replay-source-current-san/` (generated C/header, instrumented binary
and reports), and `recorded-source-words-native.json`. Exact source/artifact
pins, compact oracle summaries and mutation records accompany this record.

Two early harness failures are preserved, not used as product evidence:
the new-document observer was absent until CDP Page was enabled; then a
first-response selector hit body owner 6 instead of collision owner 5.
Final probes check observer installation and select the full content ID.
VPK selection also observes synchronous allocation/request context to
distinguish resources sharing a container; the oracle requires the exact
owner and digest-gate error.

Qualification covers these sources, caps, counters, histories and target.
Actual stale/superseded callbacks, other SDK/allocator failures, automatic
and repeated GPU loss/rescue, native GPU load/unload/recovery, persistent
HUD, shader qualification and complete transport/transient planning remain
unfinished. No issue or chapter is promoted.
