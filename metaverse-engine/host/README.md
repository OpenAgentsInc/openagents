# Metaverse Engine host contract v1

`aethyr-host-v1` is the boundary between a Coax realm and its platform
effects. WebGPU, Vulkan and Metal execute the same realm, input/session
rules, render plan, shaders, HUD and authored audio. A host transports
bytes, reports platform events and executes admitted effects. It does not
decide a second set of gameplay, layout, admission or recovery rules.

The additive corridor [HTP1 carrier](transport-manifest.md) binds complete
source/container plans to actual request generations and world epochs before
reader construction/fetch. Its original-ticket and allocation histories have
a separate [adoption receipt](../../docs/verification/aethyr-streaming-12990/walk-transport-binding.md).
It preserves the v1 host ABI and the earlier unknown-size/private profiles.
Its additive v2 diagnostic profile reports exhaustion without interrupting
effect cleanup and prepays capture copy/text temporaries. The
[adoption inventory](stream-adoption.md) names retained adapters and separate
compiler/SDK/GC/driver memory domains.

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
distinct observations; see `metaverse-engine/render/resources.coax` and
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
`metaverse-engine/core/loop.coax` owns fixed-step refusal, clamp, budget, dropped backlog
and interpolation. A host never steps simulation to catch up with display
refresh. The room's authoritative 30 Hz clock continues while the local
world is covered or suspended.

`metaverse-engine/host/lifecycle.coax` composes focus and visibility around that
published session policy and records an admitted-frame clock in monotonic
microseconds. A hidden surface cannot restart its clock on focus alone.
Repeated timestamps admit zero dt; negative/backward time, a full submission
budget and unavailable physical frame resources refuse without consuming
elapsed time. The returned delta has no
host clamp. `htl_event` returns the existing `LifeActions`, `htl_offer`
returns admission/dt, and `htl_tick` converts the host clock to 30 Hz ticks
without multiplication overflow. Adapter execution is recorded separately
from these component checks.

Display callbacks offer work. They are not submitted frames. The shared
Coax frame admission consumes the observed number of unfinished
submissions; a refusal does not run an extra producer update or overwrite
resources still used by the GPU. Completion refers to this submission's
fence/command buffer. Readback and timestamps wait for that same owner.
The native desktop may use the display's refresh rate; a requested test
ceiling is common to all compared hosts.

All lifecycle policy is `metaverse-engine/session/lifecycle.coax`, version 1. Hosts
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

The v1 Wayland SDK reads the compositor's complete XKB keymap (at most
1 MiB), and updates its client state from serialized modifier masks. The
separate `wayland-input.coax` projects actual keysyms/Unicode and modifier
facts; it never assumes a physical US layout. Text-field local compose
publishes a complete UTF-8 commit (at most 1,024 bytes), with no intermediate
or duplicate printable key-down. Physical modifier transitions include the
key event before the following serialized mask update. Key-up releases the
same delivered key even if the modifier snapshot changed.

Scroll offers coalesce one protocol frame: value120 takes precedence over
discrete notches, with 100 CSS pixels per notch; finger/continuous sources
retain pixel travel. The existing old-protocol wheel fallback remains ten
CSS pixels per axis unit. Captured drags retain motion/release outside the
view. The SDK's real keymap/listener and ownership receipts are in
[`wayland-input.md`](../../docs/verification/host-contract-2026-11-05/wayland-input.md).
The text-input-v3 companion now handles compositor commits as atomic UTF-8
transactions of at most 4,096 bytes. Preedit is held outside the producer;
only committed text reaches inbox 1. It advertises neither surrounding text
nor surrounding deletion, and refuses a transaction requesting unsupported
deletion. A `done` for an older client state within the same field remains
valid; a reply from before the current enable is discarded. Cursor rectangles
use surface-local coordinates. Missing text-input support retains local XKB
compose.

Clipboard copy and Ctrl-V/Shift-Insert paste use the seat's data device.
Pastes admit a complete UTF-8/no-NUL transaction at EOF, at most 2 MiB, with
a five-second deadline. No partial prefix reaches the producer. A checked
input epoch plus the host's engine identity owns each request; pointer-down,
Tab/Enter/Escape, field/geometry changes, focus loss, hiding and engine
replacement invalidate pending text and local compose. Paste also cancels
older composition before starting its own transaction. Four retained sources
(at most 8 MiB total) and four nonblocking send descriptors bound outgoing
clipboard ownership. Cancelled sources retain their bytes through actual
send completion; a closed reader cannot terminate the host with SIGPIPE.
Unavailable/refused clipboard effects report the existing 604 error path.

Actual protocol-object, descriptor and window-callback receipts are in
[`wayland-text.md`](../../docs/verification/host-contract-2026-11-05/wayland-text.md).
These component checks do not claim an interactive third-party IME session
or replace the ordinary window/three-host/default gates.

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
`coaxial_outbox_len(state)`. A nonempty version 1 packet also supplies
`coaxial_outbox_capacity(state)`, the physical byte extent of the selected
allocation. `action-packet.coax` admits the complete measured span before a
byte view, decode or effect. Empty actions need no pointer/capacity export.
A refused packet executes no effect; it acknowledges the action once and
then delivers platform-error message 604 outside that producer call.
The host copies admitted requested bytes, executes or
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
inputs and the exact native-local URI/origin. `http-target.coax` normalizes
the HTTP target. `buffers.coax` admits the
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

Native application payload storage is admitted before allocation/copy by
`inbox-queue.coax` and `socket-queue.coax`. The process has four workers;
each socket slot has at most one executing and one pending immutable job.
An engine context leases at most 64 inbound event nodes and four inboxes of
payload bytes. Four separate terminal-owner cells retain only scalar
error/close outcomes. Outbound retention is at most 64 commands/32 MiB,
at most 8 MiB each. A native reader waits on a full inbound lease queue;
command/SDK allocation refusal yields explicit error/close. Close/replacement
wakes retired readers and keeps already executing bytes alive only for
their concrete owner. These are application storage limits, not a claim
about kernel, browser or SDK hidden buffers. Browser adapters that cannot
apply native read backpressure must refuse/close explicitly instead of
retaining an unbounded callback/encoding queue. Current Linux observations
and remaining integration scope are in [socket-linux.md](socket-linux.md).

`local:/api/room/relay/everglade/local-1` chooses the same Coax room simulation: a browser
Worker, or the OpenAgents Metaverse Engine's existing native room process on explicit development
IPv4 loopback. Ticketless native admission remains the authority in
`native-local-policy.coax`; the host cannot broaden it.
The producer's existing reconnect spelling appends exactly `?re=1` to that
URI. The shared local route authority admits that spelling and preserves
the query; adjacent paths, other queries and additional parameters refuse.

HTTP imports are `coaxial_http(url,len,body,body_len,flags)`,
`coaxial_http_bounded(...,timeout_ms:int64_t)` and
`coaxial_http_cancel(id)`. They share eight slots, returning 0–7 or -1 for
capacity/route refusal. Requests are same-site. Flags are 1 POST (else
GET), 2 beacon (no callback), 4 answer wanted, 8 attach the stored playtest
key, 16 consume the body's first line as Authorization. Credentials never
enter logs or public page facts.

The URL is valid UTF-8, 1–4096 bytes, begins with one slash, and contains no
backslash, control or DEL byte. Coax removes the fragment, retains the query
(including an empty `?`), removes literal and percent-encoded dot segments,
and applies the browser's path/special-query percent encoding. Trailing
ASCII spaces are stripped before parsing. Existing percent escapes keep
their spelling. A normalized target has at most 12288 bytes. Normalization
does not select an origin: `/.//b` becomes the pathname `//b` on the
already bound site. Browser/Apple URL effects concatenate the bound origin
and target; a native HTTP request writes the target with its separately
bound Host header. A refused or short destination receives no prefix.

Answers arrive as message `512+id`, ASCII `STATUS\n` followed by raw body
bytes, including a leading byte-order mark or malformed UTF-8 in an ordinary
unbounded response. Ordinary delivery clips the byte sequence at the inbox
capacity. The browser v1 adapter does not decode/re-encode those body bytes.
Bounded requests require valid UTF-8, want an answer, cannot be beacons and fit the
**entire** response including status/newline inside the inbox. Deadline,
overflow and network failure produce exactly one `0\n`. Cancellation returns
1 when it cancels a live owner or 0 when absent, invalidates ownership before
aborting, and is silent: no later reply from that owner. Replacing the engine
cancels all its operations. `coaxial_now_ms()` is monotonic integer
milliseconds; `coaxial_now_seconds()` is Unix wall seconds. Neither is dt.
The physical browser byte/prefix regression and unchanged legacy helper
observations are in
[`http-bytes.md`](../../docs/verification/host-contract-2026-11-05/http-bytes.md).

Version 1 refuses HTTP redirects. `http-redirect.coax` selects no-follow
before the request and refuses status 301, 302, 303, 307 or 308 before
publishing any response bytes, including a redirect without a Location.
An answer-bearing request receives `0\n`; a beacon remains callback-free.
The host sends no request to the redirect destination and never replays
the original method, body, Authorization or player header there. Browser
fetch uses `redirect: 'error'`; its beacon path uses keepalive fetch because
`sendBeacon` cannot select redirect behavior. Pre-v1 browser callers keep
their prior behavior. This explicit v1 rule does not alter other statuses.
The actual browser workload and remaining native integration are recorded
in [the redirect qualification](../../docs/verification/host-contract-2026-11-05/http-redirect.md).

## Asset packs and graphics

Version 1 media packet readers measure `coaxial_media_capacity(state)` before
forming a view of `coaxial_media(state)` and its logical length. The shared
[`media-packet.coax`](media-packet.md) policy admits empty packets without
storage; nonempty packets require the complete physical allocation. A refused
packet becomes an empty media request set, with no parser call or readiness
notice from those bytes. Unrelated world rendering continues. This is separate
from per-image decode, cache and GPU readiness admission.

An asset is its content identity, not its URL. A pack becomes visible only
after the complete bytes and table are admitted; failed/partial packs
publish no parts. `VPK1` has a 32-byte header, little-endian count at 4,
whole length at 8 and table offset at 16. Each 48-byte table row has kind
at 0, licence at 1, byte offset at 4, length at 8 and a zero-terminated
32-byte name at 16. Table/product/end arithmetic is checked against the
actual complete allocation before reading a row or payload.

A host never reads that header or table itself (GitHub #13304). The one
reader is `pack_table` in [`pack-table.coax`](pack-table.coax), with
`pack_stated` for a loader that holds only a pack's first 12 bytes. That file
states the layout once; [`pack-table-face.coax`](pack-table-face.coax) is its C face. The Metaverse Engine's
[`metaverse-engine/editor/pack.coax`](../editor/pack.coax) loads it and adds the full
admission, `pack_read` (order, catalog and licences), and the writer:

| Host | How it reaches the reader |
| --- | --- |
| Browser (`coax/web/host.js`, `character-host.js`, `pack-fetch.js`) | `openPackTable()` in `coax/web/pack-host.js`: the pack reactor's `pack_table_*` exports (`coax/web/pack-face.c`) |
| iOS and the Mac (`apple/PageHost.m`, `apps/everglade-desktop/Sources/Engine.swift`) | [`pack-table.h`](pack-table.h) over the unit the app's build compiles from `pack-table-face.coax` |
| Android (`apps/everglade-mobile/android/host.c`) | `pack-table.h`, the same way |
| Coax tools (`coax/web/room/room-catalog.coax`, `verse/native/wasm-run.coax`) | `#load` of `pack-table.coax` |
| Linux (`verse/native/realm-host.c`) | `verse/native/pack-table-gen.h`, the face's emitted C (below) |

The Linux realm host (`verse/native/realm-host.c`) includes
`verse/native/pack-table-gen.h`: the C the compiler writes for
`pack-table-face.coax`, committed because nine builds compile that file and
most take no Coax step. `coax/remote/check.sh reports` (its `pack-reader-c`
step) fails when the committed file isn't what the compiler writes. After a
change to `pack-table.coax`, its face or the compiler's C, write it again from
the repository root:

```sh
coax/bin/coax c metaverse-engine/host/pack-table-face.coax --library > verse/native/pack-table-gen.h
```

### The frame plan

A page's engine may export `uint32_t coaxial_frame_plan_v1(state, uint32_t features, uint32_t *out, uint32_t cap)`
(GitHub #13303). It writes the frame's operations as records of 8 words; record 0 is the version
record. [`metaverse-engine/render/frame-plan.coax`](../render/frame-plan.coax) states the layout, writes the
records from `passes.coax`'s plan and reads them back. A host walks them and maps a pass id to its
pipeline and bindings; it decides no order, and it refuses a plan whose version record is not the
one it was built for. A page without the export keeps the host's built-in order. No page exports it
yet. C hosts read the numbers from [`frame-plan.h`](frame-plan.h), which that module writes:

```sh
coax/web/tool.sh metaverse-engine/tools/frame-plan-header.coax > metaverse-engine/host/frame-plan.h
```

`check.sh reports` has a `frame-plan-h` step that fails when the committed header is stale.

### Packs paged by world cell

A pack may say which bytes of its image each world cell's content is, so a
host fetches a cell's pages when the cell controller
(`metaverse-engine/world/cells.coax`) asks for that cell, and not the whole pack at the
start (GitHub #13304, for #13280). The catalog has a line a cell:

```text
Z CX CZ DIGEST AT N [AT N ...]
```

[`pack-cells.coax`](pack-cells.coax) is the one reader and the one writer of
that line: `pack_cell_line` writes it (the pack tool calls it),
`pack_cell_find` reads a cell by key, and `pack_cell_key` is the page's key,
`(cx + 32768) * 65536 + (cz + 32768)`. The asset-pack profile
(`proto/verse/asset-pack-v2.json`, `cell_lines`) states the line and holds
fixtures to this module. A host does three things, and decides nothing:

1. Asks for the cell: `aethyr_pack_cell(image, key)` on the reader's C face
   (`aethyr_pack_cell_ranges` in `pack-table.h`), or `result.cell(key)` on
   what `loadPaged(pack, {demand: true})` returns in `coax/web/pack-host.js`.
   It gets the cell's digest and up to eight byte ranges, each inside the
   image. A cell the catalog lists twice, or whose line is damaged, is not
   found; the host then answers the controller with a failed load.
2. Fetches what the ranges need: `hpg_cover(l, at, n, out)` in
   [`page-stream.coax`](page-stream.coax) names the pages not yet held that
   cover a range. The host fetches those pages and calls `hpg_admit` for
   each (`result.more(ranges)` in the browser). No page is fetched twice.
3. Answers the controller with the digest from step 1 once `hpg_ready` says
   every range is held.

`coax/web/tool.sh metaverse-engine/host/check-pack-cells.coax ROOT OUT` runs the three
steps end to end through the browser's module on a pack it writes, and is the
`pack-cells` step of `coax/remote/check.sh reports`.

The v1 browser consumes `pack-stream.coax` and `private-pack.coax` through
the separate pack reactor. Each observed stream admits at most 64 MiB;
actual retained allocation pages share a 256 MiB/64-ticket owner. The twelve
header bytes may arrive separately. Private metadata is at most 4,096 bytes;
exact encoded/plaintext extents and plaintext identity must precede whole
VPK1 admission. SDK cancellation or retirement keeps ownership through actual
reader/crypto/decoder completion. Completed immutable bytes can transfer to a
matching renderer; a retired pending load cannot publish into it. Pack sound
descriptors retain source bytes until awaited audio installation releases
them. The page's measured 8 MiB master workspace remains separately retained.
These application bounds do not measure hidden browser/network queues or GC
RSS. Actual ordinary world and physical failure/retirement observations are
in `docs/verification/host-contract-2026-11-05/browser-pack-loading.md`.

Parts retain order: catalog kind 1, vertex bytes kind 2, index bytes kind 3,
texture layers kind 4 in table order, sounds kind 5 by name. One page pack
occupies four declared asset indices: catalog, vertices, indices, layers.
Catalog is copied through `coaxial_asset(state,index,bytes)` only after
measuring `coaxial_asset_capacity`. `coaxial_asset_ready` follows the copy;
vertex/index readiness gives byte length, layers readiness gives layer
count, and unavailable/failed asset readiness is zero. Other page assets
follow those four indices. Progress message 605 is `loaded total`, bytes,
and is not asset admission.

GPU admission is the shared `metaverse-engine/render/` plan: package/interface,
resource views, passes, binding sources, texture formats/mips/sRGB,
samplers, blend/depth/raster state, uploads and media. Native SDKs execute
that admitted plan and exact draw offsets, including base instance. A new
GPU object first acquires its Coax registry handle; freeing it releases
that handle. A frame validates every handle before writes. Device loss
retires all handles, preserving generations so stale handles never revive.

For v1 Everglade layers, `render/host-layers.coax` owns compressed PNG/JPEG
decode, the profile's 512-pixel side, doubling, alpha-weighted mips and the
canonical premultiplication round trip. Every host uploads the resulting
complete per-layer level-major straight RGBA8 bytes directly; SDK image
resizing or another premultiplication conversion changes the asset identity.
The browser's separate `host-layers.wasm` reactor uses that same authority.
Pinned pre-v1 browser engines keep their established SDK path. Complete
bank and actual GPU byte receipts are in
[`layers-canonical.md`](../../docs/verification/host-contract-2026-11-05/layers-canonical.md);
the new v1 path deliberately has its own visual reference.

Version 1 still media uses `render/host-image.coax` for PNG/baseline-JPEG
decode at natural rectangular dimensions, each axis at most 2048. Its
measured plan bounds compressed input to 8 MiB, workspace to 32 MiB and
complete straight RGBA8 output to 16 MiB. It performs one premultiplied byte
round trip, no mip generation and no SDK resize/colour conversion. Hosts
upload those exact bytes directly to the shared media texture descriptor.
Unsupported or malformed input refuses explicitly, with no SDK fallback
or published partial output. That image becomes failed/not-ready and its
607 notice is `URL 0 0`; unrelated world rendering continues. Successful
notices are `URL WIDTH HEIGHT`, pending images are omitted and rows retain
request order without a final newline. Retired asynchronous work cannot
allocate a texture or notify the replacement engine. Pre-v1 media retains
its established SDK path. Complete authored-image and actual GPU receipts
are in [`media-canonical.md`](../../docs/verification/host-contract-2026-11-05/media-canonical.md).

`host/media-notice.coax` owns the 607 payload on every v1 adapter: at most
112 unique root-relative whitespace-free URLs, successful dimensions in
1..2048 and failed dimensions forced to zero. Formatting validates the
complete input and destination capacity before writing any byte. A host
never truncates a media notice to fit the inbox; refusal publishes no
partial message. Browser formatting is synchronous after its separate
reactor is ready, and asynchronous image completion retains the concrete
engine/media owner. Actual delivered inbox bytes, continued frames and
recovery observations are recorded in
[`media-notices.md`](../../docs/verification/host-contract-2026-11-05/media-notices.md).

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
bytes before asynchronous decoding. A v1 bank also exposes
`coaxial_sound_bank_capacity(state)` for the actual metadata allocation and
`coaxial_sound_bank_storage(state)`/`_storage_bytes(state)` for the complete
immutable WAV allocation. The host admits the metadata view before forming
it, then uses `host/audio-bank.coax` to admit every complete name/source span
before copying. Decoded allocation additionally needs the canonical WAV
plan and a shared `audio-memory.coax` ticket. An admitted name alone does
not admit a source view or decoded allocation. See
[`audio-bank.md`](audio-bank.md) and the Linux SDK's
[`audio-linux.md`](audio-linux.md) for measured ownership APIs.
The bank's aggregate copied source budget is 8 MiB across 128 identities;
overlapping spans under different names count separately. Refusal changes
no existing row or byte total.
The v1 browser loads the separate `host-services.wasm` reactor before it
admits any producer view. Its bank registry consumes bounded metadata rows
through the same `hab_*` authority; a refused row sets aggregate failure
while later independently admitted assets remain available. A refused
metadata allocation produces no bank view. Pre-contract pages retain their
existing adapter. Browser PCM retention and queue admission have separate
integration receipts; this bank-span check does not qualify them.
Pack sounds and declared sound-directory
assets share the same name lookup; missing/failed data produces silence,
not a substituted sound. Audio activation needs the platform's gesture or
explicit muted/offline test permission. Covering/pausing the world suspends
its audio graph. Tests never use the owner's speakers.

`coaxial_sounds_len(state)` drains the current command packet once;
`coaxial_sounds(state)` identifies its bytes. Hosts must not poll the
draining length twice and then play nothing. Version 1 packet readers use
`coaxial_sounds_capacity(state)` and the measured-span authority before
forming that view; the reported length alone is insufficient. This export
does not drain or mutate the packet. Version 1 consumes the packet once after each completed
producer handoff, including messages and input while drawing is covered;
packet delivery does not wait for another GPU frame. Invalid packet spans
are refused before a byte view is formed and do not consume unrelated
producer work. Lines are the existing
`metaverse-engine/audio/mix.coax` protocol: `NAME GAIN PAN [DELAY LOWPASS]`, or the
eight-field mixed voice `NAME GAIN PAN DELAY LOWPASS RATE WET ID`;
`~NAME GAIN PAN` updates a looping bed; `!bus`, `!room`, `!fetch` and `-ID`
operate the shared bus/room/cache/voice identity. Decode promises retain a
concrete engine owner and command order. The shared PCM interpreter admits
at most 16 simple, 48 mixed and 64 bed voices when complete decoded data is
available. Retiring the engine disconnects its audio node and fences pending
decodes. The gesture fact is captured when the command is issued: controls,
room/cache and beds can initialize while suspended; earlier one-shot cues
are refused rather than replayed after a later gesture.
Pan clamps to [-1,1], rate to [0.5,2] and wet/bus gain to [0,1]. Delay is
seconds on the audio clock. Lowpass is the declared one-pole filter
`y += (1-exp(-2*pi*f/rate))*(x-y)` when `1 <= f < rate/2`.

The bus gain eases with 0.03 s time constant; the room uses the admitted
normalized impulse. The compressor is -18 dB, knee 12 dB, ratio 3,
attack 0.01 s/release 0.25 s; the final limiter is -1 dB, knee 0, ratio 20,
attack 0.001 s/release 0.08 s. Stopped voice IDs fade at 0.015 s and stop
after 0.08 s. Native playback must qualify rendered samples against the
existing muted audio checks; consuming strings is not audio parity.

Version 1 executes `metaverse-engine/audio/host-pcm.coax` at 48 kHz on every host;
the browser runs that exact interpreter in an AudioWorklet. SDK callbacks
copy its 1024-frame stereo blocks without implementing DSP. Both shared
dynamics stages have 288 frames of lookahead, giving mixed voices 576
frames of authored bus delay; simple and bed voices bypass that bus.
The final linked peak ceiling is -1 dB. Bed gain starts with a 0.5 s time
constant and updates gain/pan with 0.25 s. The precise shared algorithm and
borrowed immutable PCM ABI are in `metaverse-engine/audio/host-pcm.md`.

Generated bank WAVs use `metaverse-engine/audio/host-wav.coax`: exact 44-byte PCM16
RIFF, mono/stereo, 22,050 or 48,000 Hz, through the shared zero-phase 64-tap
rational resampler into 48 kHz PCM. Its measured output extent precedes
allocation. Everglade's ordinary default instead selects the picked
`NAME-CAND.ogg` assets from `everglade-bank.coax`; the 48 generated WAVs
are the `?sfx=synth` profile. The complete authored catalogue in
`audio/everglade-pcm-assets.coax` preserves all 153 Ogg candidates. Its 32
required APC1 companions bind exact source and finite PCM bytes/SHA/shape;
missing or corrupt required data refuses without SDK fallback. The other
121 retain SDK decoding after complete metadata/shape admission. Unlisted
safe names keep their metadata-admitted SDK path. This does not substitute
synth sounds or erase the recorded original codec failures. The exact
versioned format and APIs are in `audio/host-pcm-asset.md`. Browser immutable PCM
has 128 allocation tickets and 8,388,608 binary32 words (32 MiB); allocation
refusal cannot publish a partial asset. Pre-contract faces retain the
existing Web Audio graph. Its independent retained observations are not a
sample-parity claim for the version 1 interpreter.

Before an authored Opus SDK decode, `audio/host-opus.coax` measures complete
Ogg framing and packet-derived decoded extent as well as trimmed output.
Reserve each observable compressed scratch, decoded plane, transfer and
retained allocation separately. A short EOS granule cannot reduce the
decoder reservation. Validate observed 48 kHz frame/channel shape against
that plan before publishing PCM; codec refusal publishes no prefix. The
exact supported framing profile is documented in `audio/host-opus.md`.
Its metadata gate is separate from actual codec-output conformance.
The Linux synchronous SDK adapter additionally executes `audio-decode.coax`:
partial reads advance frame offsets, premature/error/extra output refuses,
and exact output requires an independent EOF probe. Complete coded and probe
planes precede SDK opening; finite-word admission precedes the separately
reserved retained copy. All 153 actual Linux outputs pass the unchanged
sample bound, with exact required companions and zero per-row cleanup. This
component is separate from ordinary Linux default demand and realm/device
composition; its record is `docs/verification/host-contract-2026-11-05/linux-opus.md`.

The browser now accepts raw source descriptors at its v1 audio boundary.
`audio-source.coax` admits streamed application-owned copies before retention;
each encoded input, coded decoder extent, transfer and retained Worklet plane
has a distinct shared memory ticket before allocation. The actual SDK output
must pass `hpo_output` before copying. Source plans precede SDK decode, and
required companions execute the same Coax reader in the Worklet. Dynamic
directory descriptors are transient, preventing an ever-growing failed-name
cache. Retiring an engine aborts its concrete readers, waits for owned SDK
decode completion and Worklet cleanup acknowledgment, releases all PCM
ownership and closes that concrete context before replacement. SDK hidden
workspace/platform buffers remain separate from measured application copies.

## Browser recording

A version 1 muted clip uses the same 48 kHz Coax PCM interpreter, canonical
asset decoder and AudioWorklet as live playback. It retains commands from
clock zero, including bed/room initialization before the recording mark,
renders through the complete ending block, then crops to the admitted
recording interval. Microseconds round to 48 kHz samples; commands enter at
the next 1,024-frame mixer boundary. The recorder never creates a live
AudioContext. Pre-contract recordings retain their earlier adapter.

`audio-timeline.coax` bounds complete encoded lines to 1,024 bytes, 8,192
commands and 8 MiB total. Times are monotonic integer microseconds in
[0, 2147483647]. One bad offer poisons the recording; one successful seal
freezes its frontier. Invalid crop extents, missing required assets or
allocation refusal reject the whole WAV. Decoder/source PCM and the complete
offline output plus crop use separate instances of the existing 32 MiB
allocation authority. This keeps recording duration from consuming the
ordinary decoded-bank allowance without making capture output unbounded.

Retirement fences and waits for the actual pending descriptor/decode and
Worklet cleanup before releasing timeline bytes or PCM tickets. A replacement
owner cannot inherit the old recording. The actual ordinary-host PCM,
refusal and held-retirement receipts, exact reactor identities and commands
are in [`browser-clip-audio.md`](../../docs/verification/host-contract-2026-11-05/browser-clip-audio.md).

## Conformance and the desktop default

The separate bounded streamed browser fixture has an additive exact-build
[whole-reactor rescue profile](../../docs/verification/aethyr-streaming-12990/walk-persistent-rescue.md).
`rescue-policy.coax` owns compatibility and crash-loop admission; IndexedDB,
WebCrypto and WebGPU remain SDK effects. This fixture does not change host-v1
or qualify its native adapters.

The standalone `render_host_presentation` selector and its small shared
`hrm_presentation` mode admit only version 1/profile 2. An eligible invalid
display/extent returns an enabled but refused typed plan, never a legacy
fallback. Unsupported profiles and pre-contract faces retain their existing
path. The ordinary browser adapter projects that scalar through the services
reactor. The public Everglade face declares v1 on every host and selects the
complete path without overrides. Actual mode, held-completion and
device-recovery observations pass. See
`docs/verification/host-contract-2026-11-05/presentation-v1.md`; the admitted
graphics/current-realm observations and remaining integration exits are in
`docs/verification/host-contract-2026-11-05/graphics-admitted.md`.

The fixture in this directory retains **actual delivered arguments and
bytes**, not a list of supported feature names. Its source is shared across
the three adapters. Core checks cover struct defaults, measured arena/output
extents, input normalization, raw UTF-8 inbox behavior, assets and action
acknowledgment. Transport checks additionally exercise real loopback
WebSocket/HTTP effects, exhaustion, cancellation and old-owner callbacks.
Lifecycle checks use the versioned shared authority. GPU checks compare
admitted draws and readbacks at the existing G5 tolerances. Audio checks
capture muted/offline PCM, including asynchronous decode/voice ownership.

`check-observations.coax RECEIPT.json...` is the common receipt checker.
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

The checker also accepts `scope: "lifecycle"`: all ten actual frame/key/blur
rows and the five final focus/visibility/clock words are required. Negative,
backward and hidden frame offers produce no row; hidden releases the held
key, focus behind hidden cannot restart the clock, and first/resumed deltas
are zero. These normalized adapter observations are separate from physical
window visibility, GPU recovery and session replay qualification.

A passing result must identify source, compiler, host executable/module,
fixture, shader and asset identities and retain raw observations. A missing
section or an explicitly refused required service is an incomplete result.
Three invocations of the pure Coax fixture checks do not constitute three
host passes. Offscreen shader qualification does not establish interactive
input/room/audio/lifecycle parity.

The [current qualification and commands](../../docs/verification/host-contract-2026-11-06/current/README.md)
combine actual web, Vulkan and Metal section receipts, current admitted
compiler/source joins, physical GPU comparisons, audio and native-window
integration. The earlier November 5 records retain their failures and
component scopes.

The Metaverse Engine opens the native Metal world by default and offers one click,
**Check as Players See It**, to open the same world/build in the web host.
`--world web` also selects that host. The preview uses the same saved document and room rules;
no native-only game control, HUD or gameplay is permitted. Setup remains
complete, and neither Coder nor the editor opens by default.
