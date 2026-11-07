# Actual upload failure replays at every paid step — November 7, 2026

**#12990 remains open.** This stage extends the exact-owner WebGPU
validation-failure probe to the canonical replay format from
[the paid-step record](walk-state-replay.md). The failed attempt, physical
retirement and successful replacement now replay natively with identical
state after every paid step. No runtime/policy change was needed; the
browser recorder and independent oracle gained this shorter failure route.

Source starts at local checkpoint `fc2aa6ecca`. Its existing runtime,
958/958 coordinator and 185/185 serializer mutation gates, current Wasm
artifact and instrumented native executable remain unchanged and are
reused. Exact source/receipt pins accompany this document.

The muted headless Chrome 152 consumer runs on coderos-4080's RTX 4080,
browser tier 0, corridor-v2's pinned sources. The driver injects a real
out-of-range write into the 24-byte floor index buffer. Exactly owner 1
fails; its failed generation never becomes ready, its actual physical
owner retires and a higher generation of the same content becomes ready.
The test records 32 four-step frames and three cleanup ticks. All 131
paid-step canonical SHA-256 values match the native replay.

The independent owner oracle verifies all 108 allocation/free events,
nineteen attempts and retirements, full source identities, no failed-ready
prefix, the destruction-before-replacement timeline and final zero owners.
Both browser and native requested CPU storage peak at 12,908 bytes. The
native positive run has ASan/UBSan and leak detection enabled, with no
diagnostics. It replays the browser's recorded SDK failure; it makes no
native GPU objects and does not claim a native GPU failure injection.

Independent transcript reconstruction yields observation SHA-256
`6a5b8f6edd1cd1ea2dd0ef039fdb4accbe8076955a8a2266bd61f1b67dc92059`.
The six altered histories are also rejected for this failure route:
position, generation, omitted completion, altered intermediate state,
omitted paid step and reordered steps. The intermediate-state edit
preserves inputs, accepted events, frame endpoints and final actor records;
native replay names the first affected paid tick.

Shared artifacts under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/` are
`stream-walk-upload-state.json` (SHA-256
`a7f4c9c4c58f9cd5be71f630e4f2fc5f8c74b98c7138fabfe8865780ee27de4d`)
and `native-replay-state-san/replay-upload-state.json`. Source bytes are
canonical in this probe: decoding succeeds before the GPU refuses the
write. Corrupt or incomplete received bytes require retained payload
capture before their replay can be qualified. Cancellation, stale and
superseded histories, automatic/repeated GPU loss and rescue, native GPU
recovery, persistent HUD, shader qualification and transport/transient
accounting remain separate unfinished exits.
