# Actual unwanted loads and old callbacks — #12990

This checkpoint starts at `61944087f9` in the reused worktree. The issue
remains open for complete physical/transport/transient planning and the final
consumer/profile/teaching audit. The actual browser consumer now qualifies
an unfinished response becoming unwanted, physical join and retirement,
loading a newer generation, and redelivering the captured old callback.
The product runtime and its ownership gates are unchanged.

## What executes

`aethyr/tools/stream-callback-browser.coax` drives muted headless Chrome on
coderos-4080. It observes actual Wasm allocation/completion exports and holds
delivery of one actual HTTP response after downloading its bytes. The hold
deliberately outlives AbortSignal cancellation; it is a qualification input,
not a claim that ordinary fetch ignores cancellation. The private observer
holds those response bytes outside the product's resource model.

The selected source is cell 0's lighting image (owner 4) or vertex buffer
(owner 8), separately on browser visual tiers 0 and 1. While the response is
held, actual movement removes the resource's wanted state. The product calls
its normal abort path. The old generation remains loading, its source is
zero, and its exact allocation fields remain owned. A newer generation is
not allocated into that storage. Releasing the held response joins the
actual platform task; the pack host refuses the unwanted transfer before
copying its source. Completion reports decode failure 5 / stream failure 6.
The attempt publishes no ready prefix and physically retires once.

Walking back requests a newer generation of the same owner. After it becomes
ready, the observer redelivers the old completion's captured arguments through
the original Wasm export in a later Promise callback. It also tries allocate,
decode and destroy with that old key, then the four calls with the current
generation and wrong epoch 0. Finish/allocate/destroy return 0; decode returns
capacity 3. None changes current source/decoded/scratch bytes, pointers, row
metadata, owner counters, trace extent, paid tick or canonical state. Complete
320×180 images before and after the calls match. Eight continued movement
steps still work, then normal cleanup drains all owners.

These are actual consumer callbacks/effects with explicit response holds
and callback redelivery. They do not model simultaneous reuse: the existing
owner contract prevents replacement before the old effect has joined and
retired. The native local-file carrier is synchronous; general asynchronous
native workers remain outside its qualified profile. Accepted history replay
does not execute the browser's response hold or the refused callback calls.

## Versioned observations and independent checks

`aethyr.stream-walk-callback-history.v1` carries the existing browser profile,
frame tuples, allocation/stage observations, cell identities, final cleanup,
and `aethyr.recorded-source.v1` exception spans. Its `callbacks` companion has
schema `aethyr.stream-walk-callbacks.v1` and explicit qualification provenance.
It records the pending/retired and before/after probe snapshots, actual callback
arguments/results, source and image SHA-256 values. The 36 conformance words are:

| Indices | Meaning |
| --- | --- |
| 0–2 | Selected owner, old generation, epoch |
| 3–8 | Hold, unwanted, release, callback, accepted-completion and accepted-destruction ticks |
| 9–14 | New generation, probe tick, held phase/wanted/generation, allocated-field mask |
| 15–20 | Abort observed, released, completion decoded/status, old ready count, old destroy count |
| 21–29 | Equal probe snapshots, four old-key refusal results, four wrong-epoch refusal results |
| 30–35 | Equal pixels, probe ticks before/after, zero held source, authenticated new source, no early reuse |

`stream-callback-conformance.coax` was written with checks before its verdict.
The final root gate kills **101/101 mutants**, zero survivors. The first
101-site gate left eight survivors where equal-valued vectors hid index
mutations. Distinct generations/times and exact boundaries exposed them;
the intermediate gate left one equality boundary, then the final gate killed
it. No mutant was waived as equivalent.

The independent Python oracle authenticates the original scene, all source
containers/spans and browser RCM1 manifests. It reconstructs every allocation
extent/free, the old allocation prefix at the unwanted boundary, no early reuse,
all completion/destruction/ready records, dependency readiness, source hashes,
binary32 movement, byte/category/owner peaks and final zero ownership. It binds
the eight raw refusals and unchanged full probe snapshots, and compares all
image channels. It rejects **54 altered receipts per history**. It does not
infer hidden browser/GPU driver memory or independently shade the whole scene.

## Qualified evidence

All four final browser histories pass on Chrome 152 with the RTX 4080 adapter:

| History | Paid steps | Attempts | Allocation/free events | Dynamic CPU peak |
| --- | ---: | ---: | ---: | ---: |
| Image tier 0 (`callback-image-tier0-v2`) | 1,807 | 25 | 148 | 14,156 |
| Image tier 1 (`callback-image-tier1-v2`) | 1,807 | 25 | 148 | 14,236 |
| Buffer tier 0 (`callback-buffer-tier0-v4`) | 2,087 | 33 | 192 | 14,520 |
| Buffer tier 1 (`callback-buffer-tier1-v2`) | 2,087 | 33 | 192 | 14,584 |

Together they exercise **7,788 paid steps**, **116 attempts**, 32 raw guard
refusals, **921,600 equal RGBA8 channels**, and 216 altered callback receipts.
One failed old source is retained per history: 80 bytes for the image and
288 for the buffer. The physical field masks are 15 and 5 respectively,
derived from the manifest. GPU errors and runtime diagnostic messages are
empty. Resource, inbox, image/audio, renderer and transport effect owners
all drain; the pack host ends with its separate 8 MiB master reservation.

All four histories replay through the unchanged sanitizer CPU binary from
`61944087f9`: every accepted completion, paid-step digest and final actor/cell
identity agrees, with no sanitizer diagnostics. The existing independent CPU
oracle rejects eighteen altered histories each, **72 total**. It recomputes
the canonical observation transcript and binds the input receipt hash. This
is actual CPU allocation/codec replay of the browser backend/cost profile,
not native GPU execution or native asynchronous callback qualification.

Per-owner/category peaks, stage times, frame p50/p95/p99 and active/drawn/
updated counts are in the compact oracles and raw histories. Qualification
runs overlap and are not performance comparisons. Wasm initial memory is
2 MiB under the existing reserved 8 MiB ceiling; the pending observer does
not add product storage. The unchanged product gates reuse their prior
mutation evidence; this checkpoint changes only qualification code/docs.

## Precursors, reproduction and limits

The first buffer observer asked the canonical serializer for a digest while
facts were pending, outside its documented observation boundary. The unchanged
serializer asserted. Pending snapshots now use the last paid-step mark;
the before/after guard probe still reads live canonical state at a quiescent
boundary. No serializer assertion or gate was weakened.

The first CPU replay refused an actual recording omission: the final cell
footer preceded the eight continued steps. The recorder now captures another
actual cell snapshot immediately before close. The replay reader and all
identity comparisons are unchanged. Earlier logs/receipts remain precursors;
they are not included in final totals.

Serve the retained site with `coax/web/serve.sh SITE --static --listen
127.0.0.1:PORT --cache 0`. Run `coax/web/tool.sh
aethyr/tools/stream-callback-browser.coax BASE SCENE_JSON NEW_OUTPUT TIER OWNER`,
where OWNER is 4 or 8. Inspect with `python3
aethyr/tests/streaming_callback_oracle.py SCENE_DIR NEW_OUTPUT --fault-checks`.
CPU commands and receipt schema follow the existing browser replay record;
`streaming_walk_oracle.py --native-only` checks its report and negative inputs.
All commands are bounded below fifteen minutes. The owned test server is
stopped after browser work.

Private artifacts under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/` include
`callback-site-v1`, the four final outputs above, `callback-cpu-v1-san`,
precursor outputs and `callback-evidence`. The site's runtime JS matches the
current checkout; its unchanged executed Wasm is SHA-256
`2bf4397d355718ded0b79f38afa7ffb377afd0f7b16cc9b123d6ae5ad7e8c73a`,
reused from the prior browser qualification. Source/artifact manifests pin
the full source graph, served files, toolchain, binaries, raw receipts,
images, replay reports and logs. Tools and all six book registries pass;
no chapter status is promoted. No issue push or closure occurs here.
