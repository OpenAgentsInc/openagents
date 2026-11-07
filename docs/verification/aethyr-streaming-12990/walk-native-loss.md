# Native streamed draw-result recovery — November 7, 2026

**#12990 remains open.** Starting from local checkpoint `fcbb026f79`, the
existing native corridor adopts owned queue submissions and checked Coax
result/deadline policy. It automatically rebuilds after reported draw loss,
retains pending effects through a stop and drains them before destruction.

The input profile is **SDK result injection over actual Vulkan rendering**.
The qualifier does not reset a physical GPU. Factory/upload failure recovery,
ordinary native host migration and native persistent rescue are not inferred
from these observations.

## Boundary and ownership

`verse/gpu/vks-submit.{h,c}` is an additive SDK API. A successful begin means
the driver accepted work, not that the work completed. Its command pool,
command buffer and fence have one concrete submission owner. Polling retains
that owner after timeout, not-ready or an unknown error. Retirement refuses
while pending, rejects another device and is idempotent after the owner is
empty. Successful creation publishes each handle separately; undefined
failed-creation outputs never enter the retained owner.

Vulkan treats device loss as completion for the purpose of pending-use
lifetime rules. A timeout alone does not permit destruction. These SDK rules
follow the [device-loss contract](https://docs.vulkan.org/spec/latest/chapters/devsandqueues.html)
and [fence contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitForFences.html).
Failed command outputs are generally undefined; see
[VkResult](https://docs.vulkan.org/refpages/latest/refpages/source/VkResult.html).
The independent stub deliberately poisons those outputs.

The actual Coax consumer uses `render/submission-policy.coax` to distinguish
completion, pending work, device loss, invalid observations and terminal
errors. It owns the exact ten-second per-submission/cleanup deadline,
including fail-closed invalid clock inputs. `render/loss-policy.coax` supplies
the existing three-losses-per-minute and absolute ten-second rebuild cycle.
Its new abort operation stops a terminal error or timeout without inventing
a device-loss count, serial or clock observation. An earlier stop reason
cannot be overwritten.

Draw borrows remain live while a command is pending. After a reported loss,
the existing source/CPU owners survive physical old-device teardown and
checked GPU-generation replacement. All granted visual sources upload again;
rebuilding draws the current accepted state without a paid step or new source
read. A second loss while rebuilding consumes another serial under the
original deadline. A failed replacement or fourth loss stops paid frames.

The stopped frame gate is the same checked C/Coax face used by the browser.
The actual native caller attempts a paid frame after a stop and requires
refusal without a tick change. Closing uses the real closing state. A pending
submission first quiesces; only then are its borrows returned and the normal
resource retirement performed. Fixed display/HUD grants remain live through
physical surface destruction. Every final resource, decoded image/audio,
registry, transfer and concrete SDK owner drains in the measured cases.

The checkpoint observer is reused only to observe accepted state with queued
facts still pending. This does not adopt the browser's whole-Wasm rescue on
native pointers or consume pending facts outside their paid boundary.

## Actual qualification matrix

Every run enables Khronos validation. Hardware runs use coderos-4080's RTX
4080; fully instrumented ASan/UBSan/LSan runs use Lavapipe with two workers.
Both render the actual 320×180 Coax world/HUD/presentation shaders, allocate
real resources and retire real old/new devices. Software timings are not
hardware performance claims.

| Input | Outcome |
| --- | --- |
| `--auto-loss` | One reported loss automatically rebuilds from the same CPU sources; full image/state and continued play match |
| `--auto-repeat` | Three recoveries followed by a fourth-loss stop; stopped paid call refuses and cleanup drains |
| `--auto-restart` | A reported loss in the replacement draw supersedes that attempt; the next attempt succeeds with the original deadline |
| `--auto-timeout` | A completed-or-pending hardware fence observation is held for 10.1 s; the ten-second stop retains its pool/fence and all resource owners, then cleanup waits for actual observation |
| `--auto-late` | A successful completed SDK observation returns after 10.05 s; the post-return deadline check stops the session without another paid frame |
| `--auto-fail` | Replacement admission is refused after old-device destruction; CPU/source state remains intact until cleanup |
| `--auto-oom` | A terminal non-loss result stops without spending a loss-window event |
| `--auto-route`, tier 0 | Three reported losses across the original 768 m world's out-and-back route, with stable sleeping actor identities |
| `--auto-route`, tier 1 | The same route/events/actor behavior with the second original native image/cost profile |

These nine cases run on **both** hardware and the instrumented software
backend: **18 final GPU histories**. The injection records retain the actual
Vulkan result separately from the reported result and cumulative injection
count. Loss injection occurs after a real completed draw; the held-fence
case delays observation instead. Late-success delivery waits after actual
retirement. The deadline check refuses that late success; it does not preempt
a synchronous SDK call while blocked. Replacement-admission refusal is an explicit
SDK input, not an actual `vkCreateDevice` allocation failure.

Interruption probes offer zero ns so that pre-loss and replacement images
name the same pose. They remain recorded frames and do not produce paid-step
marks. No equal-image claim compares different moving frames. Full routes
have **3,238 recorded frames and 12,931 paid steps**, including three zero-step
interruptions and cleanup. Each visits all 64 resources in 114 attempts with
776 allocation/free events. Active/drawn/updated peaks are **4/3/4**.

The independent oracle reconstructs every source/container identity, RCM1
cost, allocation lifetime, dependency publication and binary32 movement
sample, plus each result/count/deadline outcome. Full readbacks compare every
channel after successful recovery. The eighteen cases reject **324 altered
receipts**. Four full routes additionally compare 3,238 corresponding frames
per pair: movement, accepted events, actor checkpoints and active/drawn/
updated counts match across tiers. For the same manifest, every paid-step
canonical hash also matches across hardware and software. Different cost
manifests keep different canonical identities.

The nine hardware histories replay through the native CPU/codec receiver:
**26,795 paid steps** compare exactly, including stopped-session cleanup.
The receiver uses the original backend-1 manifest and accepts the new native
v3 input schema additively. Its existing native-replay v3 output is unchanged.
The independent CPU oracle reconstructs the complete observation byte stream
and rejects twelve altered histories per case: **108 rejections**. GPU effects,
loss notifications and wall-clock timings are not simulated by CPU replay.

## Measured costs and timing

The native manifests retain their current logical GPU cost model. Explicit
Vulkan allocation extents are measured, not declared opaque or inferred to
fit the logical category caps. Physical backend/transport/transient planning
is still an issue acceptance gap.

Final route metrics are recorded per owner/category in `native-loss-*-oracle`
files. CPU peaks remain **14,139 / 14,232 bytes** at tiers 0/1. Reported core
and SDK static sums are **873,929 / 22,794 bytes**. These diagnostics do not
measure all linker padding, allocator/driver internals or process commitment.
The observer's fixed report, shader, readback and event arrays are separate
bounded test storage. The measured Vulkan peak includes display/HUD targets
and temporary readback/replacement backing, separately from dynamic resource
payloads. The companion `native-loss-runtime.json` records final-source
frame quantiles, allocation peaks, identities and exact executed hashes.

The timeout case retains a pool/fence, CPU bytes, GPU resources and ledger
owners at the stop. Roughly the remaining tenth of a second is spent waiting
for observation before cleanup. Its frame elapsed time includes the ten-second
hold; p99 over mostly ordinary frames is not a replacement for that explicit
maximum. Parallel headless qualifiers overlap, so frame quantiles are unpaced
observations rather than interactive or phone FPS measurements.

## Gates, regressions and failed trials

- Submission-result/deadline authority: **39 mutants killed, zero survivors**.
- Changed loss/abort authority: **140 killed, zero survivors**.
- Native loss observation authority: **9 killed, zero survivors**.
- Additive native replay schema authority: **5 killed, zero survivors**.
  Total **193**, with dependency baselines enabled and no equivalents.
- The counted independent Vulkan stub runs the actual SDK under sanitizers.
  It checks twelve creation/submit failures, poisoned failed outputs,
  completion/loss retirement, pending timeout/error retention, cached polls,
  wrong-device refusal and exactly-once destruction.
- The old controlled native profile still completes its full route through
  the new SDK and passes the unchanged v2 oracle and its 30 negative cases.
- The rebuilt browser rescue carrier still passes actual pending-load and
  failed-adapter reloads, complete state/image restoration and cleanup. The
  unchanged independent rescue oracle rejects nine altered receipts each.
- Tools gate and all chapter registries pass; status counts are unchanged.
  No whole-book pass or unrelated listing fix is claimed.

The stub first allowed failed output parameters to stay zero, hiding a
cleanup bug. Poisoning them produced an actual assertion failure. The SDK
now stores creation outputs in local temporaries and publishes handles only
on success. The red executable/log and fixed instrumented executable remain
retained. This is a product fix, not an equivalent-mutant exemption.

The first native loss oracle accepted a changed event tick because a later
frame happened to have the same retained source set. It now binds interruption
events to the actual recorded zero-step boundaries and rejects that change.
Earlier compiler failures (unused result/shadowing) and v4/v5 observations
remain separate development artifacts. The final matrix was rerun after the
checked deadline, failed-output and late-success fixes; unchanged browser/SDK/policy gates
are reused where their inputs did not change.

## Reproduction and remaining scope

Artifacts live under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`.
Final native builds are `native-loss-v7` and `native-loss-san-v7`; cases are
`native-loss-NAME[-san]-v7`, where NAME is `steady`, `auto-repeat`,
`auto-restart`, `auto-timeout`, `auto-fail`, `auto-oom`, `auto-route` or
`auto-route-high` or `auto-late`. The unchanged CPU receiver and final input-bound reports
are in `native-loss-cpu-v1-san`. `native-loss-evidence` retains exact source
snapshots, compiler dependencies, SDK checks, failed trials and execution
logs. Source/artifact manifests pin these plus the original scene/manifests.

```sh
coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD [--sanitize]
NEW_BUILD/stream-native SCENE_DIR SCENE_SHA256 NEW_BUILD/stream-draw.spv NEW_OUTPUT TIER --auto-route
python3 aethyr/tests/streaming_native_loss_oracle.py SCENE_DIR NEW_OUTPUT NEW_BUILD/stream-draw.spv --fault-checks
python3 aethyr/tests/streaming_native_oracle.py --cpu-replay SCENE_DIR NEW_OUTPUT CPU_BUILD/replay-CASE.json
```

The counted SDK test has a Coax driver and a C Vulkan stub. Emit it with
`coax/bin/coax c aethyr/host/tests/submission-sdk-check.coax --library`, then
compile that C with `submission-sdk-fixture.c`, `verse/gpu/vks-submit.c` and
`coax/platform/tool-native.c`, using the installed Vulkan header include flags,
`-I verse/gpu -I coax/platform`, `-lm` and ASan/UBSan. Run with leak detection
and halt-on-error. The exact generated C and executed positive/red binaries
are retained in the evidence directory.

Build environment/ICDs follow the preceding native runtime record. Hardware,
software, build, mutation and oracle commands remain individually below
15 minutes. Browser regressions use muted headless Chrome. No visible window,
audio output, system change, owner action, push or issue closure is part of
this local stage.

The legacy `vks_submit`, resource/image/pipeline factories and transfer/readback
helpers still have fatal error paths; those are named residual SDK adapters.
Native HTTP/file workers, genuine driver-loss injection and native persistent
rescue are unqualified. Remaining #12990 acceptance work includes complete
native physical and transport/transient cost planning, actual category pressure
with prior resident content usable, remaining failed/superseded/allocator
histories and the final consumer/limits/book audit. Broader shader-reference
and host-service qualification keep their separate scope.
