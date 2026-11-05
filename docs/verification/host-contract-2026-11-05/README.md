# Host contract qualification (#12869)

This is an **incomplete** conformance record. The normative boundary is
[`aethyr/host/README.md`](../../../aethyr/host/README.md). Aethyrforge's world
default remains web. Metal's interactive host is #12868; neither these
browser checks nor the earlier offscreen Metal qualification authorizes a
default switch or issue closure.

## Qualified sections

The actual `coax/web/host.js` loads a real Coax fixture realm and generated
Coax WGSL in muted headless Chrome. It uses a private page server on IPv4
loopback 9169 and a finite Coax HTTP/WebSocket server on loopback 9170.
It does not replace the production host with a test host, access the
owner's clipboard, start a room/model, or use the speakers.
The recording realm explicitly exports host contract version 1. Merely
loading the shared renderer does not migrate an older realm's transport
or engine replacement behavior; absent/zero opt-in keeps that boundary.

- Twelve core dispatch probes observe the compiler's nonzero struct
  default, exact frame arguments, first-frame dt zero, next dt 1/64,
  320×180 target at DPR 2, key code points and modifier bits, key release,
  blur release, wheel normalization, mouse/touch coordinates and IDs,
  short touch cancellation, synthetic mouse suppression, raw UTF-8 inbox
  bytes and byte truncation, asset copying/refusal and one action acknowledgment.
- The real GPU readback contains exactly 57,600 identical BGRA pixels
  `[64,128,128,255]`. Its colour depends on the first and last uploaded
  uniform words. The shader uses exact binary scales; an exploratory
  decimal division produced a permitted one-byte reciprocal-rounding
  difference and was replaced in the fixture. Production G5 sources and
  tolerances were not changed. This tiny image qualifies the fixture's
  upload path, not Everglade's production render graph.
- Real WebSockets exercise four reservations/fifth refusal, deferred open,
  UTF-8 and empty echo, per-socket FIFO, oversized-frame error, binary
  suppression, immediate close/reuse fencing and deferred close delivery.
- Real HTTP exercises eight reservations/ninth refusal, complete response
  byte bounds, invalid UTF-8 refusal, cancellation, slot reuse and late
  completion suppression. A same-layout engine replacement cancels old
  transport, notifies the retained realm of the network change and opens a
  fresh slot; old responses do not enter it.

The separate `check-transport.coax` runs the browser scalar adapter against
the real Coax Wasm authority: 21 ABI and route probes plus nine HTTP-effect
probes passed. The seven recording-realm checks and shader's focused check
pass. At foundation commit `3f1396f5a2`, root mutation killed all 292
transport, 59 route and 9 copy-span mutants, with no equivalents. Those
unchanged passing gates were not repeated. Native carrier C emission and
the scalar renderer face's native C compilation passed.

`check-observations.coax` reads the same core receipt from web, Vulkan or
Metal and admits the actual rows through `conformance.coax`. All eight
words of every authored input event, both frame rows, asset copy/refusal,
and observed version/default/dropped/held metadata are required. It admits
the browser receipt here. Root mutation killed all 190 semantic checker
and 60 JSON adapter mutants, with no survivors or equivalents. This shared
core check does not imply that unrecorded sections or hosts passed.
The same tool admits the actual browser transport receipt through
`transport-conformance.coax` and `receipt-transport.coax`. It compares all
commands, import results, callback bytes and per-kind order while permitting
cross-connection scheduling. It also checks open delivery follows the
reservation's return. All 277 transport semantic and 20 JSON adapter root
mutants were killed, with no survivors or equivalents; every word of all
79 actual rows is perturbed by the quantified check.

The Linux Vulkan realm adapter (`verse/native/realm-host.c`) also admits
the same core receipt on coderos-4080. `check-vulkan.coax` drives the real
realm arena, face, explicit still-frame input, inbox/action dispatch and
measured asset-copy SDK. Its tiny C adapter contains only those SDK calls;
Coax authors all probes and JSON. The same run passes ASan/UBSan with leak
checking. `vulkan-core.json` retains its rows and
`vulkan-artifacts.sha256` identifies the actual Linux compiler, generated
C/header pair, executable and output. No GPU/window or audio device is
opened by this core-only observer.

The opt-in Linux native transport SDK now executes the same real 79-row
loopback workload. `socket-stream.coax` incrementally validates RFC6455
headers, fragmentation, controls and complete UTF-8; oversized text is
drained to its boundary and emits one empty error, and binary messages are
drained silently. Outgoing text is masked and fragmented in Coax. SDK
threads copy physical bytes and queue owner-tagged callbacks; the main
thread applies the shared slot/generation/engine authority before delivery.
HTTP helper results pass that same ownership, deadline, complete-byte and
UTF-8 authority. Cancellation revokes ownership before aborting the helper.

The actual Linux transport workload passes normally and with ASan/UBSan,
including leak checking. `vulkan-transport.json` retains the normal raw
events; `vulkan-transport-artifacts.sha256` identifies both executables,
observations, generated Coax C/headers, the HTTP helper and SDK sources.
Root mutation killed all 205 stream, 24 outgoing-fragment, 14 drain-budget
and 10 explicit-loopback-site mutants, with no equivalents. The existing
21 HTTP helper/parser checks also pass. These unchanged gates are retained
rather than repeated. Ordinary native site hostnames still use verified
TLS; plaintext HTTP is admitted only for an explicitly configured
`http://127.0.0.1` development site.
Focused SDK clock probes also distinguish monotonic milliseconds from Unix
wall seconds and preserve the explicit deterministic test clock override.

The ordinary shared-realm packager now includes the same SDK closure as
the observer. `native-network-build.coax` emits the network/entropy units
and selects/copies the SDK sources; the native and remote Vulkan wrappers
only compile/link them with platform TLS. The resulting ordinary
Hold-the-Stair build links on Linux and executes 90 golden frames. The
same packaged transport passes the 79-row workload under ASan/UBSan with
leak checking. Loading the shared facade alone does not opt a legacy face
into this transport contract.

The native SDK additionally resolves the one exact `local:` URI against
an explicit immutable `VERSE_NATIVE_LOCAL_ROOM=ws://127.0.0.1:PORT`
startup binding. `local-socket-route.coax` owns the path/origin composition;
11 root mutants were killed, with no equivalents. A real loopback peer
reports the exact `/api/room/relay/everglade/local-1` request path, then
UTF-8 echo and deferred close. Its nine actual SDK rows are retained in
`vulkan-local-route.json`; every row word participates in the 20-mutant
local observer authority. The final mapping probe passes under ASan/UBSan
and leak checking. `native-packager-artifacts.sha256` binds these inputs,
generated units, executable and output. This echo probe qualifies route
projection, not the room simulation. The configured endpoint must be the
native Coax room process; its exact development/loopback/one-seat admission
remains `native-local-policy.coax` and the separate room drift qualification.

The complete frozen shared Everglade frame, all textures/media and actual
browser/Vulkan readbacks are recorded in [shared-frame.md](shared-frame.md).
That record preserves both the published G5 live comparison (53 values
beyond tolerance, within its unchanged 2,000 allowance) and the supplemental
zero-allowance comparison, which fails. The Metal owner reports the same
frame through its production interpreter; its final source/executable join
and interactive parity remain required.

`browser-core.json`, `browser-transport.json` and
`browser-replacement.json` retain the actual fixture event rows. Each row
is `[kind,a,b,c,d,e,byteCount,FNV32]`, as defined by `fixture.coax`.
The complete 230,400-byte readback is retained as
`~/.openagents/aethyrforge/host-contract-12869/browser-readback.json`.
`artifacts.sha256` identifies these outputs, source inputs, exact Wasm,
generated WGSL and both compiler images. These artifacts contain only
synthetic fixture data.

## Commands

From a worktree, select the existing qualified tool compiler with `COAX`
and keep `COAX_JOBS=2 COAX_CHECK_JOBS=2`:

```sh
coax/web/wasm.sh aethyr/host/fixture.coax aethyr/host/tests/fixture-face.c \
  "$HOME/.openagents/aethyrforge/host-contract-12869/fixture.wasm"

# The qualified graphics compiler emits both entries from the same Coax:
"$HOME/.openagents/aethyrforge/shader-qualification-12867/final-compiler/coax" \
  wgsl aethyr/host/tests/fixture-shader.coax \
  --vertices hfv_bg_vs,hfv_fx_vs --fragments hfv_bg_fs,hfv_fx_fs \
  --bindings aethyr/host/tests/fixture.bindings --strip hfv_ \
  > "$HOME/.openagents/aethyrforge/host-contract-12869/fixture.wgsl"

coax/web/tool.sh aethyr/host/check-transport.coax \
  "$HOME/.openagents/aethyrforge/host-contract-12869/fixture.wasm"
coax/web/tool.sh aethyr/host/check-browser.coax \
  "$HOME/.openagents/aethyrforge/host-contract-12869"
coax/web/tool.sh aethyr/host/check-observations.coax \
  "$HOME/.openagents/aethyrforge/host-contract-12869/browser-core.json" \
  "$HOME/.openagents/aethyrforge/host-contract-12869/browser-transport.json"
```

Each command takes seconds, not a whole-model or release gate. The browser
driver waits for its private services to listen and terminates them at exit.
The network fixture itself has a two-minute ceiling. The generated fixture
face accepts `COAXIAL_NATIVE_ADDRESS` for the native uintptr ABI; the same
observer and networking commands are available to the Metal/Vulkan adapter
checks. It is an effects fixture and does not need Everglade's render package.

On Linux, with an existing compiler selected explicitly:

```sh
coax/web/tool.sh aethyr/host/build-vulkan.coax \
  "$HOME/.openagents/aethyrforge/host-contract-12869/vulkan" "$COAX"
"$HOME/.openagents/aethyrforge/host-contract-12869/vulkan/check-vulkan" \
  "$HOME/.openagents/aethyrforge/host-contract-12869/vulkan/core.json"
coax/web/tool.sh aethyr/host/check-observations.coax \
  "$HOME/.openagents/aethyrforge/host-contract-12869/vulkan/core.json"
```

Add `--sanitize` to the builder and use a separate output directory for
the ASan/UBSan image. The Linux ABI retains the existing non-PIE/low-4-GiB
allocation rule; Metal uses the fixture's explicit native uintptr ABI.

For the real Linux transport workload, add `--transport` to the builder.
Build `verse/native/net.coax` with `coax/web/tool.sh` and pass that helper's
compiled executable, plus a private state directory, to the resulting
observer:

```sh
coax/web/tool.sh aethyr/host/build-vulkan.coax \
  "$HOME/.openagents/aethyrforge/host-contract-12869/vulkan-transport" "$COAX" --transport
"$HOME/.openagents/aethyrforge/host-contract-12869/vulkan-transport/check-vulkan" \
  "$HOME/.openagents/aethyrforge/host-contract-12869/vulkan-transport/transport.json" \
  /absolute/path/to/compiled-net-helper \
  "$HOME/.openagents/aethyrforge/host-contract-12869/vulkan-transport/state"
```

The observer starts its private Coax server, polls actual health before
work, then closes its sockets/helpers and server on exit.

## Required exits still open

The native Metal receipt/source record, shared lifecycle transitions and
device-loss/replacement receipts, interactive producer/GPU parity, and
muted/offline audio sample comparison remain to be recorded. Ordinary
Vulkan lifecycle/audio execution and HTTP URL normalization are still
required. The ordinary SDK packaging and local routing probes do not
substitute for complete interactive host qualification.
Only a complete three-host result plus #12868's interactive parity allows
native by default and the one-click web preview.

`tools-check` was run. It reports five existing human-asset Python tools
without Firm-reason headers (`face_rig.py`, `metahuman.py`, `outfits.py`,
`pack_human.py`, `shape.py`); none belongs to this change. New browser glue
carries reason 1 and the drivers/server are Coax.
