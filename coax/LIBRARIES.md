# Selected Coax library contracts

Start here when reusing code in `coax/lib/`, Verse or Psionic. This index
covers the entry points below and their current limits. Other names in those
source files remain implementation details; this is a curated set of
contracts, not a package registry or a promise that every internal module is
stable.

The [language status](STATUS.md) records implemented compiler capabilities.
Each library still needs its own target, storage, error and verification
contract. A successful Coax check does not establish native pointer validity,
GPU execution or runtime authority. [Book chapter 26](book/ch26-library-recipes.md)
shows three small offline consumers; the [Psionic book](../psionic/book/README.md)
and [network notes](lib/net/README.md) describe the larger systems.

| Selected entry | Current use and boundary | Native link inputs |
| --- | --- | --- |
| [`coax.sequence`](lib/sequence.coax): `swap`, `reverse` | Small generic procedures over caller-owned mutable views; used by Verse's pass formatting and Psionic's HTTP buffer rotation | Generated C; no platform shim |
| [`verse.hud`](../verse/hud.coax): the listed module exports | Shared CPU HUD geometry, used by native and Coaxial consumers | Generated C and the host's ordinary C/math libraries; no HUD-specific shim |
| [URI syntax](lib/net/uri.coax) and [WebSocket framing](lib/net/ws.coax): `uri_parse`, `ws_frame_write`, `ws_frame_read` | Pure bounded protocol pieces; transport and authentication are separate consumers | Generated C; WebSocket's loaded SHA-1 source is included; no socket/TLS shim for these functions |
| [Tool JSON](lib/json/json.coax): selected `jv_*` reads and `jo_*` writes | Caller-owned node tree and output buffer, used by Coax tools | Generated C; no platform shim |
| [Descriptor-generated protobuf](proto.md): selected message `new`, `set_*`, `get_*`, `encode`, `decode` | Versioned schema representation; demonstrated by the existing HUD invocation capstone | Generated self-contained C; checked-in descriptor is a build input; no protobuf runtime or `protoc` required |
| [Psionic CPU metadata](../psionic/lib/tensor.coax) and [scalar prediction](../psionic/lib/learn.coax): `psionic_tensor_layout`, `psionic_tensor_index`, `psionic_predict` | Small CPU contracts from chapters 1–2; no model artifact or device required | Generated C and ordinary C/math libraries; these functions need no address-observation shim |

The recipes qualify selected native C executions. Existing consumers and
linked evidence name any additional platform coverage. This index does not
extend a native result to every Wasm, Metal, CUDA, SPIR-V or WGSL target.
`#export` controls Coax module visibility; `#export_c` is a separate C ABI
choice. Several older libraries still use a flat `#load` graph, so an index
entry alone does not create a namespace or private boundary.

## Mutable sequences

**Maturity and targets:** new experimental shared helper, adopted by two real
repository consumers. This change qualifies the selected native C calls and
consumer behavior; it supplies no GPU implementation or general target promise.
**Verification:** `coax/bin/coax test coax/lib/sequence.coax --library` and
`coax/bin/coax mutate coax/lib/sequence.coax --library`. The affected adoption
record retains native output and generated-size/cost observations.

Load `lib/sequence.coax` under an alias and call `sequence.swap(items, a, b)`
or `sequence.reverse(items)`. The logical module is `coax.sequence`; its
export list contains those two names. Type parameters specialize ordinary
value copies, including shallow copies when an element contains a borrow.

`items.count` measures elements; `a` and `b` are element indices, not byte
offsets. The caller owns the view's backing allocation, keeps any element borrows
alive, and excludes concurrent conflicting access. Neither procedure retains
or returns a view. `swap` uses one element temporary; `reverse` makes
`items.count / 2` swaps. Copy cost depends on the element type. Neither
procedure allocates a heap buffer or changes a caller's resource policy.
An empty `reverse` is valid. Invalid `swap` indices trigger ordinary Coax
bounds traps; they are not an expected-error return channel.

Actual adoption is in [Verse's pass formatter](../verse/gpu/everglade-pass.coax)
and [Psionic's serving engine](../psionic/lib/serve-engine.coax). The
[Verse recipe](book/examples/ch26_verse.coax) also uses the alias directly.
These are synchronous storage operations, not general ownership or lifetime
checking.

## HUD geometry

**Maturity and targets:** production-bounded exported CPU geometry, with existing
iOS and Coaxial consumers. The recipe qualifies selected native C API results;
it does not add a new renderer, GPU target or physical-device gate.
**Verification:** `coax/bin/coax test verse/hud.coax --library`; the chapter 26
`ch26_verse` unit checks the actual public API and refusal cases.

The module exports `HudRect`, `HudPoint`, `verse_hud_slot`,
`verse_hud_frame_point`, `verse_hud_frame_arm`, `verse_hud_tone`,
`verse_hud_cut`, `verse_hud_outline` and `verse_hud_arm_down`. Use the
[authored export list](../verse/hud.coax) as the boundary; `HUD_*` constants
remain private.

Sizes and positions use the caller's consistent UI coordinate units;
`bottom` is the bottom safe-area inset. Returned rectangles and points are
owned values. No input is retained and no GPU resource is created.
`verse_hud_slot` returns a zero rectangle for an invalid slot/count or width
below 100; it admits counts 1–12 and indices `0..<count`. It does not claim
general NaN/infinity admission. The caller supplies sensible finite geometry
and owns conversion to platform pixels and GPU lifetimes.

The actual iOS bridge and Coaxial HUD consumer exercise this source. The
chapter 26 example checks exact geometry and refusal through the public API;
it does not establish rendering or input behavior on a new platform.

## URI syntax and WebSocket frames

**Maturity and targets:** existing bounded protocol policies. URI has recorded
native and Wasm qualification in the linked network notes; framing here is
qualified by the selected native C fixture, not by opening a network connection.
**Verification:** `coax/bin/coax test coax/lib/net/uri.coax --library`,
`coax/bin/coax test coax/lib/net/ws.coax --library`, and the `ch26_protocol`
book unit. Retained URI evidence is in
[uri-evidence-2026-10-28.json](tests/net/uri-evidence-2026-10-28.json).

`uri_parse(text)` returns `UriParts`. It preserves the input and returns
byte ranges into it, rather than allocating or normalizing a URL. Keep the
text alive while using those ranges. Refusal has `ok=false` and zero
metadata. The supported subset has a 2048-byte total limit and exact
lowercase `https://`, `http://` and `wss://` schemes with bounded ASCII DNS
hosts. See the [network contract](lib/net/README.md) for the complete syntax
and retained native/Wasm evidence. DNS resolution, TLS verification, SSRF
policy and identity authorization are separate obligations.

`ws_frame_write(op, fin, payload, mask, out)` writes a masked client frame,
returning its byte count or `-1`. The mask has exactly four bytes; control
frames are final and at most 125 payload bytes; output capacity is checked
before writing. The caller owns disjoint payload, mask and writable output
storage. A live client obtains fresh mask/handshake entropy through its
actual transport contract. The fixed mask in the book is an offline RFC
fixture.

`ws_frame_read(bytes, limit)` reads an **unmasked server** frame. Its status
separates incomplete input, success and protocol/size refusal. On success,
`header`, `length` and `total` identify the payload in the caller's bytes.
Keep those bytes alive and unchanged while using the payload. A frame header
result alone does not establish complete text-message UTF-8 or fragmented
message validity; the larger reader/connection supplies those checks.
Never feed a masked client frame into this server-frame reader as a round
trip test.

The [offline recipe](book/examples/ch26_protocol.coax) uses independent
client and server vectors. The full `relay_conn` interface additionally
requires `socket-native.c`, `tls-native.c` and entropy effects; its [network
notes](lib/net/README.md) name the exact platform flags and qualification.
Those effects are not needed by this recipe.

## Tool JSON

**Maturity and targets:** existing tool-oriented parser/writer, used by native
Coax tools. This entry does not qualify every protocol admission profile or GPU
backend, and this book change does not rerun its unchanged full gate.
**Verification:** `coax/bin/coax test coax/lib/json/json.coax --library` and
`coax/bin/coax mutate coax/lib/json/json.coax --library` exercise its authored
pure checks and mutation boundary.

`jv_parse(document, text, nodes)` parses one complete value into a
caller-provided `JvNode` array. The document borrows both `text` and `nodes`;
keep both alive and unchanged through all reads. Failure returns `false`
and sets `ok=false`, `count=0` and `err`. Read with `jv_get`, `jv_at`,
`jv_path`, `jv_kind` and `jv_raw`; absent lookup is `-1`. `jv_str` unescapes
into a separate caller-owned byte buffer and returns its length or `-1`.

Depth is at most `JV_DEPTH` (64), and node/output capacities are supplied by
the caller. Duplicate keys are not rejected: `jv_get` finds the first.
Escapes are checked, but raw string UTF-8 is not generally validated.
`jv_int` accepts plain integers within ±2^53 and otherwise returns its caller's
fallback. This is the tool parser's contract; security-sensitive Nostr
admission has its own parser/profile and must not be silently substituted.

For output, use `jo_start`, `jo_obj`/`jo_arr`, `jo_key`, the typed value
writers, `jo_end` and `jo_text`. `Jo` borrows its output buffer. Always check
`bad` before publishing the text: capacity/depth failure can leave **partial
output**, which must be discarded. This writer does not promise atomic
replacement. `jo_raw` expects already-valid JSON from its caller.

## Descriptor-generated protobuf

**Maturity and targets:** implemented selected proto3 descriptor API, with
native C ABI/sanitizer evidence documented in [proto.md](proto.md). This is the
supported schema/runtime subset, not all protobuf features or a new GPU target.
**Verification:** `coax/book/examples/ch24_workflow/run.sh` runs the existing
credential-free native capstone; set `BOOK_SANITIZE=1` for its sanitizer path.
The runner's `PROTOC` oracle is optional. Reuse its retained result when inputs
are unchanged rather than rerunning it for this index edit.

Import a pinned descriptor with `#import_proto` and use the generated
message API. The [capstone source](book/examples/ch24_workflow/workflow.coax)
shows `Book_HudInvocation_new`, the selected `set_*` procedures,
`Book_HudInvocation_encode`, `Book_HudInvocation_decode` and
`Book_HudInvocation_get_revision`. Prefixes and message fields come from
the imported descriptor; the internal `PB_*` runtime helpers are not a
separate public package API.

Decoded messages and string/byte getters borrow the original wire storage.
Setters and encoding borrow their supplied destination; keep active storage
alive and use disjoint input/output buffers. Getters report error and
presence separately. Preserve full-width numeric types such as `u64`
revision; a decoded record grants no permission to invoke an action.

The shared wire limits are `PB_MAX_BYTES=1048576`, `PB_MAX_DEPTH=64` and
`PB_MAX_FIELDS=524288`, with descriptor-specific rules in addition. The
[protobuf contract](proto.md) defines error precedence, unknown-field
handling and alias refusal. Physical overlap observation is a native
boundary; it cannot run in the compile-time evaluator. The existing
[capstone host](book/examples/ch24_workflow/host.c) checks real short buffers
and wire bytes. Its runner has an optional independent `protoc` oracle;
ordinary generation and use need neither `protoc` nor a protobuf runtime.

## Psionic CPU metadata and scalar prediction

**Maturity and targets:** bounded CPU teaching APIs used by the Psionic book,
with selected native C qualification. No device, original-model weights or
full inference-serving qualification is implied.
**Verification:** `psionic/check.sh --only tensor learn` is the existing scoped
unit gate (these exact unit names appear in `psionic/check.sh`); the new
`ch26_psionic` book unit is the small offline consumer. The unchanged broader
unit results should be cited, not rerun for a documentation-only change.

`psionic_tensor_layout(shape)` returns status, element/byte counts and
row-major strides. `psionic_tensor_index(shape, coordinates)` returns
status and an element offset. Shape rank is 0–4, active dimensions are
positive, inactive dimensions/coordinates are zero, and at most 1,048,576
contiguous `f32` elements are admitted. A scalar has one element and four
bytes. These procedures return owned metadata and retain no buffers.
Check `PsionicTensorStatus.ok` before using the payload.

`psionic_predict(weight, x)` is the scalar example from
[Psionic chapter 1](../psionic/book/ch001-learning-a-number.md). Both inputs must
be in its finite `[-16,16]` profile; refusal returns
`PsionicStatus.number`. On success, `value` is the `f32` product. This is
one scalar operation, not full model inference or GPU qualification. The
[book recipe](book/examples/ch26_psionic.coax) checks statuses before
reading values and requires no weights file.

The optional native tensor-copy boundary is different:
`psionic_tensor_copy` in [tensor-memory.c](../psionic/platform/tensor-memory.c)
links generated tensor C and that adapter, with the inline
[spans.h](../psionic/platform/spans.h) observations. It observes physical overlap and
calls Coax. The internal `psionic_tensor_copy_checked(..., overlap)` trusts
that observation; passing `false` does not validate native addresses.
Native callers own pointer validity, actual extents and lifetime through
the synchronous call. Its existing tensor gate covers finite-data,
capacity, alias and spare-tail cases. The chapter 26 recipe deliberately
uses only metadata and scalar prediction, requiring no such observation.

## Verification and adding a consumer

Use [chapter 26](book/ch26-library-recipes.md) to run exactly the new recipes.
`coax/book/check-examples.sh --list` names individual source units and special
host/mutation/structure units. `--only` selects exact names, deduplicates them
and refuses an empty or unknown selection before any compiler or fixture
runs. A selected result is labelled partial. With no arguments the existing
full book gate still runs.

The [recipe boundary tests](tests/library_recipes.py) check selected native
`O0`/`O2` and ASan/UBSan executions. The sequence module has its own authored
checks and mutation gate. Those finite results qualify the recorded cases;
they do not certify arbitrary callers or new target backends. Follow the
repository rule to reuse passing evidence on unchanged inputs.

For a new public entry, record its exact source/export names, link inputs,
units, bounds, borrowing/retention and error behavior, maturity, claimed
targets, and the command/evidence that supports each claim. Add a small
credential-free example with exact output/exit fixtures. Keep platform
effects outside the library's decision logic and retain unavailable or
unsupported qualification explicitly.
