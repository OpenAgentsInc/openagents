# Integrated native SDK failures — #12990

This checkpoint starts at `cd7852e390` in the reused
`/home/christopherdavid/oa-open-issues` worktree. The issue remains open.
The actual corridor consumer now records scoped staging, destination,
submission and upload-poll errors, plus an upload deadline stop. Its CPU
replayer reproduces the allocation/codec effects and every paid step of
those histories. Actual stale/superseded callbacks, complete transport/
transient and physical allocation planning, and the final adoption/teaching
audit remain open.

## Input and lifetime boundary

`native-sdk-fault.coax` admits 34 explicit modes. The independent
`tests/stream-sdk-fault.c` linker wrapper claims the first matching streamed
resource kind, binding row, generation and epoch. Only that attempt's
selected SDK domain is scoped; unrelated calls forward normally. One error
is delivered. Creation/submit prefixes run all preceding phases against
the real Vulkan driver, return `VK_ERROR_OUT_OF_HOST_MEMORY` before the
selected call, and poison failed output handles/pointers. Private child
counts and handles must retire exactly, without destroying poison outputs.

| Mode suffix, for both `--sdk-buffer-` and `--sdk-image-` | Ordered phases |
| --- | --- |
| `stage-1` … `stage-4` | Create buffer, allocate memory, bind memory, map memory |
| `create-1` … `create-4` | Create buffer/image, allocate memory, bind memory, map buffer / create image view |
| `submit-1` … `submit-6` | Create command pool, allocate command buffer, begin, end, create fence, queue submit |
| `poll-1` | First fence query reports an unknown negative result; following real queries drain it |
| `poll-2` | Real fence reaches success, then the wrapper reports device loss |
| `poll-3` | Return not-ready for 10.1 seconds, then observe the actual fence |

The last three inputs stop paid work. An unknown result retains the transfer,
command pool, fence, staging and destination until actual terminal observation.
The 10.1-second observation hold crosses the unchanged ten-second Coax
deadline; it does not claim physical GPU execution took ten seconds.
Reported device loss follows real completion, so this probe cannot release
physically pending GPU use. It does not reset the device. See Vulkan's
[queue-submit failure rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit.html)
and [fence result meanings](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetFenceStatus.html).
Prefix probes are explicit errors over real effects, not physical allocator
exhaustion. The wrapper is linked into this test consumer only; shared SDK
implementations and ordinary realm hosts are unchanged by this checkpoint.

The carrier closes each creation scope before its outer resource cleanup;
private factory/submission cleanup remains inside its scope. Poll scope ends
before transfer retirement, since that transfer's children were created in
the submission domain. Command-pool destruction accounts for the implicit
command-buffer release. Wrong handles and duplicate private destruction are
violations, never forwarded as valid SDK destruction.

Staging refusal finishes with existing capacity status 3; destination and
submission refusals use decode status 5. Prefix failures retire normally
before retry under a newer generation. After a fatal upload stop, subsequent
already-admitted GPU load commands fail with status 5; CPU-only effects can
still finish with status 0. At the stopped cleanup boundary, successful
completion is unwanted (stream status 5), while failed completion has stream
status 6. No stopped attempt publishes ready content. Finish retains partial
CPU/decoder ownership until shared destruction, and temporary renderer grant
5 remains live through transfer join and scratch retirement.

## Receipt and replay

Native observation v6 requires an `sdkFault` companion with schema
`aethyr.stream-native-sdk-fault.v1`. Earlier observation versions retain their
original contracts. Its case is 0–16 for buffers and 17–33 for images, in
the table's order. The qualification string explicitly excludes physical
exhaustion/reset. The 26 words are:

| Indices | Meaning |
| --- | --- |
| 0–4 | Kind, domain, phase, claimed attempt count, injected error count |
| 5–7 | Resource row, generation, epoch |
| 8–11 | Reported result, last real result, real calls, phase/query observations |
| 12–19 | Final private buffers, images, allocations, views, maps, pools, fences, command buffers |
| 20–25 | Scope start/end ns, peak private children, violations, static bytes, actual terminal observed |

Before any real prefix call, the last-real sentinel is `INT32_MIN`; otherwise
the qualified prefixes observe success. Poll records separately retain the
reported error and the actual successful fence result. Fatal inputs add a
24-word stop snapshot: key (0–2), tick before/join (3–4), CPU before/join
(5–6), GPU payload before/join (7–8), pending/owned at stop (9–10), final
pending/owned (11–12), reason (13), caller start/stop/join ns (14–16), reported/
terminal result (17–18), fixed grant at stop/join (19–20), allocation trace
extent before/join (21–22), source equality (23). Both retained-source hashes
are bound separately. Paid tick, source, CPU and trace stay unchanged while
joining; the transfer reaches zero ownership before CPU scratch is freed.

The independent GPU oracle reconstructs source/container identities, RCM1
costs, partial allocation prefixes, per-owner/category peaks, successful
dependency readiness, physical GPU object presence, stop snapshots, source
fingerprints and binary32 movement. Normal prefixes also require exact
before/after replacement pixels and HUD bytes. Fatal profiles have no draw,
readback image or replacement; `recovery` is empty. The driver's historical
stdout success phrase includes “replacement” for every successful run;
the receipt and oracle distinguish stopped profiles explicitly.

CPU replay executes actual allocations, source copying and codecs. For a
staging failure it refuses field 4 at the recorded resource row, preserving
the real prefix. Other GPU failures use typed completion after the CPU
effects. Stop source hashes are recomputed from retained actual sources.
Replay accepts the same completion history and matches each paid-step
digest; it executes neither Vulkan nor the observed wall-clock delay.

## Qualified results

All **136 histories** pass: 34 modes × two tiers × RTX 4080 ordinary Vulkan
or Lavapipe with ASan/UBSan/LSan. There are 112 prefix histories and 24 fatal
poll histories, **15,736 paid steps**, and **2,224 attempts**. Vulkan reports
zero validation errors/warnings, sanitizer logs are clean, private prefixes
fully retire and final engine/SDK owners are zero. GPU negative checks reject
**5,088** changed receipts; CPU negative checks reject **6,384** changed
histories. Across prefix replacements, **25,804,800 RGBA8 channels** match
exactly. Stopped profiles do not contribute image comparisons.

The hardware is NVIDIA GeForce RTX 4080 with driver 595.71.05; software is
Lavapipe/llvmpipe LLVM 21.1.8. Builds use Coax 0.52.0 and GCC 15.2.0 with
contraction/fast-math disabled. Vulkan loader, headers and validation layer
are the installed 1.4.341.0 packages. Runs select the installed NVIDIA or
Lavapipe ICD explicitly, enable `VK_LAYER_KHRONOS_validation`, and software
uses `LP_NUM_THREADS=2`. Sanitizer runs set leak detection, abort-on-error
and halt-on-UB. The prior NVIDIA sanitizer loader failure is retained in the
factory record; it was not retried or counted as hardware sanitizer coverage.

Dynamic native CPU peaks range from 980 to 13,232 bytes; explicit Vulkan
allocation peaks range from 1,024,516 to 1,650,640 bytes. The longest measured
scope is 10,101,153,837 ns. Each compact oracle records owner/category peaks,
frame quantiles and stage-count evidence; raw receipts retain per-stage
times and work/refusal counts. These concurrent qualification runs, including
the deliberate observation hold, are not a speed comparison. Core statics
remain 874,009 bytes. SDK statics are 23,326 bytes, including the wrapper's
280 bytes. Observer stack/report storage, allocator overhead and opaque
driver allocations are outside the declared dynamic resource model.

Both healthy hardware `--auto-route` regressions pass: each has 3,238 frames
and 12,931 paid steps, with exact canonical digest and accepted-motion/actor
identity equality to `cd7852e390`'s corresponding route. Both replay with
sanitizers and reject twelve altered histories. Current CPU replay also
accepts the earlier v4 texture-pressure and v5 allocation-failure histories,
rejecting 34 and 27 altered histories respectively. The prior browser
pending-rescue history replays 139 steps; no new browser runtime is claimed.
Same-header `__wasm__`/`SW_PRESENTATION` preprocessing of the shared C face
matches `cd7852e390` after whitespace normalization (SHA-256
`b3d142c190b32530b8ef7d4ecf3fd30694662b4f8aa4f075eccec379548f52a2`).
This proves the conditional source is unchanged, not a new browser run.

The SDK-input policy kills **366/366 mutants**, and the expanded replay
profile kills **11/11**, zero survivors. Earlier failed invocations and the
five-survivor policy precursor are retained separately; no equivalent-mutant
waiver was used. Existing checked-factory and owned-transfer SDK unit gates
from `439b611555` and `cd7852e390` are reused unchanged.

## Reproduction and retained evidence

Build with `coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD
[--sanitize]`. Run its ordinary scene/hash/shader/output/tier arguments with
one mode from the table. Inspect with `python3
aethyr/tests/streaming_native_sdk_oracle.py SCENE_DIR NEW_OUTPUT
NEW_BUILD/stream-draw.spv --fault-checks`. Build CPU replay with
`coax/web/tool.sh aethyr/tools/stream-walk-native.coax SCENE_DIR RECEIPT
NEW_CPU_BUILD --sanitize`; use its resulting `replay` executable for the
remaining receipts. The existing native CPU oracle's `--cpu-replay` mode
checks positive and altered histories. Each command is bounded below fifteen
minutes; deadline probes are bounded individually.

Private artifacts are under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`: `sdk-native-v1`,
`sdk-native-san-v1`, `sdk-cpu-v2-san`, the 136 named `sdk-*-tier*-v1`
observations, two healthy routes, precursors and `sdk-evidence`.
`sdk-source-sha256.txt` and `sdk-artifact-sha256.txt` pin current source,
compiler, fixture, generated C, binaries, receipts, images, replay reports
and raw logs. Compact per-case oracles and comparisons sit beside this
record. Tools and all six book registries pass; no chapter status is promoted.

Remaining issue work is actual stale/superseded callbacks, complete physical
and transport/transient admission planning, and final consumer/residual-
adapter/profile/teaching reconciliation. Surface/pipeline/readback and device
initialization retain named legacy failure adapters outside this streamed
load boundary. Native persistent rescue, general asynchronous native workers,
genuine physical OOM/reset and independent whole-world shader parity remain
broader unqualified profiles, not newly added issue prerequisites. No issue
push or closure is part of this local checkpoint.
