# Aethyr host contract v1

`aethyr-host-v1` is the boundary between a Coax realm and its platform
effects. WebGPU, Vulkan and Metal execute the same realm, input/session
rules, render plan, shaders, HUD and authored audio. A host transports
bytes, reports platform events and executes admitted effects. It does not
decide a second set of gameplay, layout, admission or recovery rules.

This document specifies observable behavior, including failure behavior.
An implementation reference is useful evidence, but changing that
implementation does not silently change this contract. Incompatible changes
need a new contract version. Existing network envelopes retain their own
versions in `proto/` and `docs/protocol/`.

## Calling and memory

The realm's scalar face is C11. `coaxial_*` names are identical on all
hosts; Wasm imports use module `env`. Integers declared `int32_t` or
`uint32_t` are exactly 32 bits; clocks declared `int64_t` are exactly 64
bits. Floating arguments and frame/store words are IEEE binary32 unless
their signature says otherwise. Pointers are unsigned 32-bit offsets into
the active Wasm memory on the web. A native address-capable face uses
`uintptr_t` for pointer arguments/results, consistently in the caller and
callee. A legacy 32-bit native face must keep every pointed-to allocation
below 4 GiB and refuse an unrepresentable pointer. Changing pointer width
does not change draw records, index words, storage strides or network bytes.

Calls into a realm are serialized. The host never calls the face again
from inside an import that the face is currently executing. A socket open
callback, a close callback and an HTTP completion are later calls. An
asynchronous operation belongs to its concrete engine and operation
generation; completion from a retired owner cannot answer a replacement.

Before the first frame the host calls module/global initializers, then
`coaxial_state_init(state)`. That function constructs the compiler's struct
defaults. Zero-filled memory is not a constructed state. The host allocates
nonoverlapping state, vertex and uniform regions, and keeps their measured
capacities. All additions/multiplications are checked before allocation or
copy. A new buffer or texture is zero initialized, or its initialized extent
is measured explicitly before it becomes a binding.

The required sizing/frame exports are:

| Export | Result / arguments |
| --- | --- |
| `coaxial_layout()` | State layout fingerprint; equality permits retaining state during a compatible reload |
| `coaxial_state_size()` | State bytes |
| `coaxial_state_init(state)` | Construct a fresh state |
| `coaxial_max_vertices()` | Vertex allocation count |
| `coaxial_vertex_floats()` | Binary32 words per vertex |
| `coaxial_uniform_floats()` | Binary32 words in the main uniform allocation |
| `coaxial_frame(state, vertices, uniforms, width, height, dpr, dt, still)` | Produce one frame; result is vertex count |

A face explicitly opts into this contract with
`coaxial_host_contract_version() -> uint32_t`, returning 1. The scalar
transport adapter additionally reports version 1. Missing/zero face version
retains the pre-contract adapter for older pinned engines; it is not a v1
conformance result. Merely loading the shared renderer does not migrate an
older realm's input/lifecycle/rebind protocol.

The host must not form a view past an actual allocation because the face
reported a larger logical count. The shared renderer's measured boundary
functions admit spans before copies/bindings. A refused frame submits no
render commands and acknowledges no uploads. Legacy clipping is not a
shared-renderer acceptance result. Source capacity, logical payload,
initialized high-water, physical GPU bytes, offset and element stride are
distinct observations; see `aethyr/render/resources.coax` and
`host-inputs.coax`. Storage allocation padding is at least 16 bytes, rounded
to 16; it is not the producer's logical element count.

## Frame and lifecycle

`width` and `height` are the positive integer **render target** dimensions
in device pixels. `dpr` is the scale relating input/layout units to that
target, including the chosen quality/supersampling scale. Presentation can
scale that target into a differently sized platform surface. The host
reports both dimensions in measurements; it does not report the window
size as a rendered size.

`dt` is elapsed monotonic seconds since the last admitted producer frame,
with zero on the first frame or after a clock restart. It is not an FPS
estimate, wall-clock time or a separately clamped simulation delta.
`aethyr/loop.coax` owns fixed-step refusal, clamp, budget, dropped backlog
and interpolation. A host never steps simulation to catch up with display
refresh. The room's authoritative 30 Hz clock continues while the local
world is covered or suspended.

Display callbacks offer work. They are not submitted frames. The shared
Coax frame admission consumes the observed number of unfinished
submissions; a refusal does not run an extra producer update or overwrite
resources still used by the GPU. Completion refers to this submission's
fence/command buffer. Readback and timestamps wait for that same owner.
The native desktop may use the display's refresh rate; a requested test
ceiling is common to all compared hosts.

All lifecycle policy is `aethyr/session/lifecycle.coax`, version 1. Hosts
report resize, focus/blur, shown/hidden, network/socket changes, device
loss/replacement, traps and accepted-command acknowledgments. They carry
out its returned `LifeActions`, including releasing held input immediately,
stopping/restarting the local clock, revoking stale pending commands,
asking for full state, replacing transport and reloading GPU resources.
They do not invent a native reconnect timer or replay policy. Long absence
is more than 900 room ticks; resend age is at most 90 ticks. GPU loss keeps
the session; a replacement device retires the old GPU handles. A trap
revokes pending commands and restarts from accepted state.

## Input

Key-down and key-up call `coaxial_key(state,key,mods)` and
`coaxial_key_up(state,key,mods)`. A named key is negative; an ordinary
character is its Unicode code point. Modifiers are bit 0 Shift, bit 1
Control, bit 2 Alt and bit 3 Meta. The face's result says whether it consumed the
event. Host platform shortcuts and text composition do not become duplicate
game events. Typed/pasted text arrives separately as inbox message 1,
UTF-8 bytes. IME composition does not emit intermediate key presses.

The named key table is stable:

| Key | Value | Key | Value |
| --- | ---: | --- | ---: |
| Escape | -1 | Enter | -2 |
| Backspace | -3 | Left | -4 |
| Right | -5 | Up | -6 |
| Down | -7 | Home | -8 |
| End | -9 | PageUp | -10 |
| PageDown | -11 | Delete | -12 |
| Shift | -13 | Control | -14 |
| Tab | -15 | | |

Pointer calls are `coaxial_pointer(state,x,y,phase,kind,id)`. Origin is the
top-left of the render target, positive x right and y down, in its device
pixels. `phase` is 0 down, 1 move, 2 up, 3 cancel. `kind` is 0 mouse or 1
touch/pen. A pointer's ID is stable through its gesture; simultaneous
touches have distinct IDs. Pointer capture retains the gesture when it
leaves the view. The browser's documented short, unmoved touch cancellation
normalization is applied before dispatch; synthetic mouse events following
a touch are suppressed. Hosts never turn both into two game presses.

`coaxial_wheel(state,dy)` receives CSS-pixel-equivalent vertical travel,
positive down: a line is 16 pixels and a page is 400. Optional
`coaxial_tilt` reports yaw/pitch radians. `coaxial_pad` receives the first
gamepad's left stick, x right/y forward and button 0 once per offered frame;
disconnect supplies one neutral sample. Blur/hidden invokes
`coaxial_blur` even when no render frame is running. The Coax input/session
layer decides dead zones, held controls and which fixed step takes a press.

## Inbox, actions and storage

`coaxial_inbox(state)` and `coaxial_inbox_cap()` identify writable bytes.
The host copies bytes before calling `coaxial_message(state,kind,length)`;
the face consumes or copies borrowed bytes during that call. Ordinary
message delivery truncates to that byte capacity; it does not count Unicode
characters or promise a complete final UTF-8 sequence. Sockets and bounded
HTTP have stricter overflow behavior below. Empty messages are delivered.

Standard message kinds are 1 typed text, 2 page `data-text`, 4 kept slot,
11 place, 12 spell bar, 600 page facts, 601 hidden, 602 shown, 603 leaving,
604 platform error, 605 pack progress, 606 viewer and 612 network status.
Network status is the text `1` online or `0` offline. Facts are UTF-8
`key=value\n` records; public player/account identifiers are permitted,
player keys are not. Required facts for a realm are its route/build/rules,
style/clock and actual viewport/quality. Browser-only observations are
absent when unavailable; a native host does not fabricate browser timings.

Before first frame, a host delivers its page selector/facts and available
initial assets, then restored place and bar in that order. Golden mode
restores neither place nor bar. Completion order of independent asynchronous
assets is not ordered; callbacks always retain their declared asset index.

Output is `coaxial_action(state)`, `coaxial_outbox(state)` and
`coaxial_outbox_len(state)`. The host copies the requested bytes, executes or
explicitly refuses the platform effect, then calls `coaxial_action_done`
once. Action 1 navigates, 2 copies text, 3 replaces the location hash, 4
persists the realm slot, 5 persists the playtest identity, 11 persists place
and 12 persists bar. Unavailable OS effects must not be reported as success.
Native persistent data is under `~/.openagents/`; reusable model data is
shared between apps. Runtime launch does no whole-model hash scan.

Opaque byte storage is a separate optional extension, exactly
`coax/web/STORAGE.md`. Unsupported imports refuse explicitly. A host must
not satisfy an unknown product import with an unconditional success/zero
stub. Known WASI compatibility stubs are not product capability evidence.

## Sockets and HTTP

`transport.coax` owns the slot/owner/deadline/byte decisions on both
adapters. The browser's scalar bridge is `coax/web/host-transport-face.c`;
native SDKs call the same `ht_*` functions. A `HostOwner` is its returned
slot, generation and engine epoch together. Do not reconstruct it from the
slot at callback time. `routes.coax` admits bounded root-relative HTTP
paths and the exact native-local URI/origin. `buffers.coax` admits the
complete measured destination before copying. These authorities are loaded
by the shared renderer carrier, without making transport depend on the GPU.

`coaxial_socket_open(url,url_len)` returns one of four slots, 0–3, or -1.
The UTF-8 URL must name supported `ws:`, `wss:` or `local:` transport.
Opening a fifth socket refuses without changing the four live owners.
`coaxial_socket_send(id,ptr,len)` sends one text frame only while open;
send before open/after close does nothing. Binary frames are not delivered
as text. A socket is a byte pipe; it parses no room, Nostr or gameplay frame.

Callbacks arrive through inbox kind `256 + 4*id + event`: 0 opened, 1 text
frame, 2 closed, 3 error. Events keep arrival order for one live owner.
An oversized UTF-8 frame is not truncated or partially applied: it produces
an error event instead. Local close invalidates the owner before returning,
closes platform transport and emits one deferred close event. Remote close
frees its slot when the close is delivered. A retired transport cannot
deliver into a reused slot. Event draining has a bounded per-frame work
budget and continues while hidden, so rendering cannot indefinitely block
room delivery. FIFO describes received transport order, not simulation
application: the replay/session gate owns the application tick.

`local:/api/room/relay/everglade/local-1` chooses the same Coax room simulation: a browser
Worker, or Aethyrforge's existing native room process on explicit development
IPv4 loopback. Ticketless native admission remains the authority in
`native-local-policy.coax`; the host cannot broaden it.

HTTP imports are `coaxial_http(url,len,body,body_len,flags)`,
`coaxial_http_bounded(...,timeout_ms:int64_t)` and
`coaxial_http_cancel(id)`. They share eight slots, returning 0–7 or -1 for
capacity/route refusal. Requests are same-site. Flags are 1 POST (else
GET), 2 beacon (no callback), 4 answer wanted, 8 attach the stored playtest
key, 16 consume the body's first line as Authorization. Credentials never
enter logs or public page facts.

Answers arrive as message `512+id`, ASCII `STATUS\n` followed by raw body
bytes. Bounded requests want an answer, cannot be beacons and fit the
**entire** response including status/newline inside the inbox. Deadline,
overflow and network failure produce exactly one `0\n`. Cancellation returns
1 when it cancels a live owner or 0 when absent, invalidates ownership before
aborting, and is silent: no later reply from that owner. Replacing the engine
cancels all its operations. `coaxial_now_ms()` is monotonic integer
milliseconds; `coaxial_now_seconds()` is Unix wall seconds. Neither is dt.

## Asset packs and graphics

An asset is its content identity, not its URL. A pack becomes visible only
after the complete bytes and table are admitted; failed/partial packs
publish no parts. `VPK1` has a 32-byte header, little-endian count at 4,
whole length at 8 and table offset at 16. Each 48-byte table row has kind
at 0, licence at 1, byte offset at 4, length at 8 and a zero-terminated
32-byte name at 16. Table/product/end arithmetic is checked against the
actual complete allocation before reading a row or payload.

Parts retain order: catalog kind 1, vertex bytes kind 2, index bytes kind 3,
texture layers kind 4 in table order, sounds kind 5 by name. One page pack
occupies four declared asset indices: catalog, vertices, indices, layers.
Catalog is copied through `coaxial_asset(state,index,bytes)` only after
measuring `coaxial_asset_capacity`. `coaxial_asset_ready` follows the copy;
vertex/index readiness gives byte length, layers readiness gives layer
count, and unavailable/failed asset readiness is zero. Other page assets
follow those four indices. Progress message 605 is `loaded total`, bytes,
and is not asset admission.

GPU admission is the shared `aethyr/render/` plan: package/interface,
resource views, passes, binding sources, texture formats/mips/sRGB,
samplers, blend/depth/raster state, uploads and media. Native SDKs execute
that admitted plan and exact draw offsets, including base instance. A new
GPU object first acquires its Coax registry handle; freeing it releases
that handle. A frame validates every handle before writes. Device loss
retires all handles, preserving generations so stale handles never revive.

WGSL, SPIR-V and MSL come from the same checked Coax shader tree. Metal
reflection version 1 uses separate buffer 0–28, texture 0–127 and sampler
0–15 namespaces; buffer 29 supplies element counts and buffer 30 is the
vertex stream. Storage `V3` retains portable packed stride. Build MSL 2.3+
with no fast math. Hosts do not rewrite shader arithmetic or repair
refused layouts. The G5 source/compiler/asset identities and existing
tolerances remain the graphics conformance inputs.

## Audio

Audio is authored in Coax. Optional `coaxial_sound_bank`/`_len` exposes
`NAME ADDRESS BYTES` lines for complete WAV bytes. The host copies those
bytes before asynchronous decoding. Pack sounds and declared sound-directory
assets share the same name lookup; missing/failed data produces silence,
not a substituted sound. Audio activation needs the platform's gesture or
explicit muted/offline test permission. Covering/pausing the world suspends
its audio graph. Tests never use the owner's speakers.

`coaxial_sounds_len(state)` drains the current command packet once;
`coaxial_sounds(state)` identifies its bytes. Hosts must not poll the
draining length twice and then play nothing. Lines are the existing
`aethyr/audio/mix.coax` protocol: `NAME GAIN PAN [DELAY LOWPASS]`, or the
eight-field mixed voice `NAME GAIN PAN DELAY LOWPASS RATE WET ID`;
`~NAME GAIN PAN` updates a looping bed; `!bus`, `!room`, `!fetch` and `-ID`
operate the shared bus/room/cache/voice identity. Decode promises retain a
voice slot, so decoding cannot bypass the 16 simple/48 mixed voice bounds.
Pan clamps to [-1,1], rate to [0.5,2] and wet/bus gain to [0,1]. Delay is
seconds on the audio clock. Lowpass is the declared one-pole filter
`y += (1-exp(-2*pi*f/rate))*(x-y)` when `1 <= f < rate/2`.

The bus gain eases with 0.03 s time constant; the room uses the admitted
normalized impulse. The compressor is -18 dB, knee 12 dB, ratio 3,
attack 0.01 s/release 0.25 s; the final limiter is -1 dB, knee 0, ratio 20,
attack 0.001 s/release 0.08 s. Stopped voice IDs fade at 0.015 s and stop
after 0.08 s. Native playback must qualify rendered samples against the
existing muted audio checks; consuming strings is not audio parity.

## Conformance and the desktop default

The fixture in this directory retains **actual delivered arguments and
bytes**, not a list of supported feature names. Its source is shared across
the three adapters. Core checks cover struct defaults, measured arena/output
extents, input normalization, raw UTF-8 inbox behavior, assets and action
acknowledgment. Transport checks additionally exercise real loopback
WebSocket/HTTP effects, exhaustion, cancellation and old-owner callbacks.
Lifecycle checks use the versioned shared authority. GPU checks compare
admitted draws and readbacks at the existing G5 tolerances. Audio checks
capture muted/offline PCM, including asynchronous decode/voice ownership.

`check-observations.coax CORE.json...` is the common core receipt checker.
Each JSON receipt has `schema: "aethyr.host.observations.v1"`, `scope:
"core"`, `host: "web" | "vulkan" | "metal"`, observed integer `version`,
`constructed`, `dropped` and `held` metadata, and `assets`, `frames` and
`events` arrays. Each row has exactly eight integers in the fixture's
`[kind,a,b,c,d,e,byteCount,FNV32]` order. The shared `conformance.coax`
checks the authored core sequence and `receipt.coax` adapts JSON. A core
receipt cannot substitute for any other section.
The same checker accepts `scope: "transport"` receipts with the 79 actual
rows from `check-browser.coax`'s authored loopback script plus observed
`dropped: 0`. It requires all 32 commands, 21 import results and 26 callback
positions. Different connections may complete in a different order; each
message-kind stream retains its exact payload order, and socket open may
only appear after the corresponding import result. The fixture uses
`ws://127.0.0.1:9170/echo` and `/api/host-contract/*` in every adapter so
command bytes and hashes identify the same workload.

A passing result must identify source, compiler, host executable/module,
fixture, shader and asset identities and retain raw observations. A missing
section or an explicitly refused required service is an incomplete result.
Three invocations of the pure Coax fixture checks do not constitute three
host passes. Offscreen shader qualification does not establish interactive
input/room/audio/lifecycle parity.

The current implementation record and commands are in
[`docs/verification/host-contract-2026-11-05/`](../../docs/verification/host-contract-2026-11-05/README.md).
Its browser core, real loopback transport and engine replacement sections
are qualified. Native adapter, lifecycle, production GPU and rendered-audio
sections remain separate required exits; this first record is incomplete.

At the start of #12869, the browser is the implementation reference, the
legacy Vulkan realm adapter explicitly refuses live sockets and has no
audio execution, and the interactive Metal host is #12868. These gaps must
be closed and measured before the desktop default changes. Until then,
WKWebView remains the default. When all three complete conformance and the
interactive Metal parity gates pass, Aethyrforge opens the native world and
offers one click, **Check as players see it**, to open the same world/build
in the web host. That preview uses the same saved document and room rules;
no native-only game control, HUD or gameplay is permitted. Setup remains
complete, and neither Coder nor the editor opens by default.
