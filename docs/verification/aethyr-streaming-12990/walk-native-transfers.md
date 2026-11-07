# Owned native uploads — #12990

This checkpoint starts at `439b611555` in the reused
`/home/christopherdavid/oa-open-issues` worktree. The issue remains open.
Streamed mesh/index and RGBA8 image uploads now use owned submissions in
the actual corridor consumer. The qualified live interruption is a held
completion observation, separately labelled from physical GPU latency.
Integrated SDK-error/deadline histories, stale callbacks and complete
physical cost planning remain open.

## Transfer and lifetime boundary

The additive `verse/gpu/vks-transfer.{h,c}` records one buffer copy or an
initial tightly packed RGBA8 UNORM mip/layer upload. Buffer spans use the
declared four-byte profile. Dimensions, mip count, byte arithmetic, bounds,
resource presence and non-aliasing are checked before SDK effects. Image
copies use all declared levels/layers and explicit transfer/sample barriers.
Caller admission supplies same-device resources and their current epoch;
the raw SDK views do not replace the Coax owner/ledger.

`VksTransfer` owns the existing `VksSubmission` and copies of both complete
resource views. Begin accepts work without waiting. Poll observes the
actual fence; success or device loss permits retirement, while timeout or
an unknown error preserves every view and submission owner. A failed begin
unwinds its private submission and leaves the destination unchanged. See
[fence outcomes](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitForFences.html),
[buffer copies](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdCopyBuffer.html)
and [buffer-to-image copies](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdCopyBufferToImage.html).

The native carrier removes its duplicate mesh-copy recording and fatal
`vks_submit`/`vks_upload_buffer` calls from streamed uploads. It has one
serialized upload owner, matching this local-file profile's existing effect
order. The Coax driver polls that owner before offering completion or
releasing scratch. Replacement staging is retained in a concrete global
owner, included in measured Vulkan backing, and freed after retirement.
Its temporary renderer grant releases after terminal failure as well as
success; unfinished effects retain it.

The shared C face's Vulkan-only guards refuse finish, typed finish, destroy,
paid-frame advancement, staging-grant release and GPU-handle replacement
while the relevant transfer is owned. The Vulkan adapter also refuses
resource drop, draw and device close. Coax still controls desired state,
admission and the fixed-step clock. `native-upload-policy.coax` uses the
existing submission/loss/deadline policies: unknown pending errors stop paid
work and drain, without treating the error as completion. An unsubmitted
ordinary load refusal can complete as a failed load. Device loss and the
exact ten-second deadline stop the profile. The bounded drain does not
invent completion for a permanently unresponsive device.

## Checks and actual held uploads

The counted SDK check was written before implementation. Against the legacy
submission helper it exits 1 at command-pool creation instead of returning
the injected error. The final independent Vulkan model passes under
ASan/UBSan/LSan:

- Two transfer kinds × six creation/submit phases × three result inputs =
  36 failure cases. Failed command-pool, command-buffer and fence outputs
  are poisoned. No partial transfer publishes and cleanup counters reach zero.
- Thirty-three invalid calls cover missing resources, aliasing, short or
  unaligned spans, unsupported formats, invalid mips and overflow dimensions.
- Exact copy regions, mip offsets and barriers are checked independently.
  Pending timeout/unknown-error results retain the full views; duplicate
  begin and wrong-device poll/retire refuse. Success and device loss retire
  once, clear all metadata and do not re-query a cached terminal result.

The new Coax policy kills **100/100 mutants**, zero survivors. The initial
105-site run left ten survivors. Paired retention checks were factored into
one boundary helper and checks were strengthened at zero/one and matching/
mismatching boundaries; no mutant was waived as equivalent. The intermediate
92-site and final 100-site reports are retained separately, not added together.

`--upload-pending` holds the first image upload observation for 50 ms;
`--upload-buffer-pending` does the same for the first buffer. Real Vulkan
submission still executes. During the held observation, the Coax probe
attempts frame advancement, finish, typed finish, destroy, staging-grant
release, resource drop, handle replacement and device close. All eight
return refusal. Tick, source digest, CPU bytes, GPU payload bytes and the
allocation-event prefix remain unchanged. Completion then retires normally.

The separately versioned `aethyr.stream-native-upload-pending.v1` companion
binds the ordinary native receipt SHA-256, transfer kind, concrete
row/generation/epoch, held observation count, raw guard results and both
source digests. Its twenty words are: requested hold, held polls, kind,
row, generation, epoch, tick before/after, CPU before/after, GPU payload
before/after, trace extent before/after, pending, owned, source equality,
final pending, final owned, final result. This is an observation hold, not
a claim that physical GPU execution took 50 ms.

All eight combinations pass: image/buffer × tier 0/1 × RTX 4080 or Lavapipe
with sanitizers. Vulkan reports zero validation errors/warnings, sanitizer
logs are clean and final owners are zero. The independent new oracle
reconstructs the actual allocation prefix, live CPU and prior/current GPU
owners, retained container source spans, the first selected transfer kind
and source digest. It rejects 33 changed companions per history, including
every word, every guard, provenance and both hashes. The ordinary native
oracle separately rejects 30 changed histories each.

## Route, failure and replay evidence

Both hardware tiers also pass the full `--auto-route` profile: 12,931 paid
steps and 114 attempts each, three reported-loss replacements and zero final
owners. Each route's 3,238 frames and every paid-step digest equal its
factory-checkpoint history. Allocation/decode failures remain ordinary
failed loads: hardware tier 0 `--load-alloc-4` and sanitizer tier 1
`--load-decode` each pass 139 steps, 19 attempts, retirement before retry and
47 changed-history refusals.

| Hardware route | Dynamic CPU peak | Explicit Vulkan peak | Frame p50 / p95 / p99 ms |
| --- | ---: | ---: | --- |
| RTX 4080 tier 0 | 14,139 | 1,649,392 | 2.209 / 3.068 / 4.725 |
| RTX 4080 tier 1 | 14,232 | 1,651,952 | 2.203 / 2.979 / 4.668 |

The first short probe recordings exposed a real recording omission: their
`routeCells` footer was empty, so the existing CPU reader refused them.
The native recorder now queries actual cell/actor identities before and
after controlled replacement even without a walking route. Replay's checks
were retained unchanged. The earlier failed receipts/binaries remain
precursors; current evidence uses `transfer-native[-san]-v5` and `*-v3` runs.

All twelve final histories replay through the unchanged sanitizer CPU
executable from `85095c5a7e`: **27,188 paid steps**, exact accepted records,
final identities and zero owners. Its independent negative checks reject
174 altered histories. GPU/probe oracles reject 634 altered inputs. Across
sixteen replacements, 3,686,400 RGBA8 channels match exactly. CPU replay does
not execute GPU effects or model the held wall time.

Core statics remain 874,009 bytes; SDK statics are 23,046 bytes (252 more
than the factory checkpoint). Stack/report/driver internals remain separately
excluded. Full per-owner/category peaks, stage timings and frame quantiles
are retained in each compact oracle. Concurrent qualification runs and the
injected hold are not a before/after speed benchmark. Software timings are
not hardware performance claims. Logical RCM1 GPU costs still differ from
physical Vulkan backing; pre-allocation physical admission remains open.

## Reproduction, artifacts and limits

Build with `coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD
[--sanitize]`, then run the ordinary native arguments with
`--upload-pending` or `--upload-buffer-pending`. Inspect using
`python3 aethyr/tests/streaming_native_transfer_oracle.py SCENE_DIR NEW_OUTPUT
NEW_BUILD/stream-draw.spv --fault-checks`. CPU and route commands follow
the preceding records. The counted SDK Coax driver emits as a library and
links its fixture, `vks-transfer.c`, `vks-submit.c`, `tool-native.c`, installed
Vulkan headers, `-I verse/gpu -I coax/platform`, `-lm -lpthread` and sanitizers.

Private artifacts are under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`: final native builds,
twelve final observations, `transfer-sdk-v1` and `transfer-evidence`.
`transfer-source-sha256.txt` and `transfer-artifact-sha256.txt` pin source,
compiler, generated C, original fixture, binaries, raw receipts and images.
Compact `transfer-*-oracle.json`, comparison and mutation logs sit beside
this record. Every command is bounded below fifteen minutes.

Shared-face changes are Vulkan-only. Same-header preprocessing with
`__wasm__` and `SW_PRESENTATION`, ignoring whitespace, matches `439b611555`
(`9709db1c…e8971`). This is a conditional-source check, not a new browser
runtime qualification. Existing browser evidence is retained. Tools and
all six book registries pass; no chapter status is promoted.

Remaining work includes integrated upload/factory SDK failures and deadline
stops, actual stale/superseded callbacks, complete transport/transient and
physical backend planning, and the final consumer/profile/teaching audit.
Surface/pipeline/readback factories retain named legacy SDK adapters. Native
persistent rescue, physical driver resets and general asynchronous native
workers remain unqualified broader profiles. No issue push or closure is
part of this local checkpoint.
