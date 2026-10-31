# 28. Reusing bounded libraries

A useful library names more than a procedure. Its caller needs to know who
owns the bytes, what a count measures, how refusal is reported, and which
platform execution was checked. The [selected library index](https://github.com/OpenAgentsInc/openagents/blob/main/coax/LIBRARIES.md)
records those contracts for a small set of current APIs. It also separates
Coax module visibility from the generated C interface and names the native
shims an effectful consumer needs.

This chapter calls those libraries from three short programs. All inputs
are local fixtures. No credentials, network connection, model download or
GPU is required. The programs keep storage ownership visible and inspect
failure results before using their payloads.

## Run only these examples

From the repository root:

```shell
coax/book/check-examples.sh --list
coax/book/check-examples.sh --only ch28_verse ch28_protocol ch28_psionic structure
```

`--list` names the available units without running a compiler, build or
fixture. `--only` matches whole names, accepts each name once even when it
is repeated, and rejects empty or unknown selections before execution.
It checks the entire requested set before starting anything. Selected source
units retain the usual exact output, exit-status and debug/release checks.
The runner labels a selected result **partial**: other examples, native hosts
and mutation gates have not thereby been run. With no arguments it keeps
the full book gate.

## A shared mutable sequence in Verse

The `coax.sequence` module exports `swap` and `reverse`. Their element type
is a generic procedure parameter. They operate on the caller's mutable view
and retain no descriptor. In the real code, Verse reverses digits while
formatting a GPU-pass diagnostic; Psionic uses the same swap in HTTP buffer
rotation. Those consumers keep their existing formatting and admission
rules.

The example also uses the public HUD module. Its exact output fixture and
authored checks live beside [ch28_verse.coax](examples/ch28_verse.coax).
The imported module names identify the API; physical source ownership and
`#authority` obligations still belong to the authored files.

<!-- checked-source: examples/ch28_verse.coax -->
```jai
// The same caller-owned byte operation used by Verse's pass formatter and
// Psionic's HTTP response rotation. No GPU execution or ownership is implied.
#import "Basic";
#load "../../lib/sequence.coax" as sequence;
#load "../../../verse/hud.coax" as hud;

#test "reversing trace digits keeps the caller's delimiters" {
  trace := [6] u8.['[', '8', '4', '0', '2', ']'];
  sequence.reverse(trace[1..<5]);
  #assert cast(string) trace[..] == "[2048]";
  sequence.swap(trace[..], 1, 4);
  #assert cast(string) trace[..] == "[8042]";
}

#test "the public HUD API places a slot and refuses an invalid slot" {
  slot := hud.verse_hud_slot(320, 700, 34, 0, 6);
  refused := hud.verse_hud_slot(320, 700, 34, 6, 6);
  point := hud.verse_hud_frame_point(0, 0, 100, 40, 7, 1);
  #assert slot.x == 13 && slot.y == 610 && slot.width == 44 && slot.height == 44;
  #assert refused.x == 0 && refused.y == 0 && refused.width == 0 && refused.height == 0;
  #assert point.x == 93 && point.y == 0;
}

main :: () {
  trace := [6] u8.['[', '8', '4', '0', '2', ']'];
  sequence.reverse(trace[1..<5]);
  print("%\n", cast(string) trace[..]);
  slot := hud.verse_hud_slot(320, 700, 34, 0, 6);
  print("HUD slot: % % % %\n", int(slot.x), int(slot.y), int(slot.width), int(slot.height));
}
```

<!-- checked-source: examples/ch28_verse.out -->
```text
[2048]
HUD slot: 13 610 44 44
```

The writable array and its delimiters belong to this call. Reversing a
subview preserves the other elements. `swap` uses normal bounds checks;
invalid indices are bugs that trap. Generic element copies are ordinary
value copies: copying a borrowed field does not retain its backing storage.
Keep that storage alive and exclude concurrent conflicting writes. These
operations supply neither general ownership checking nor an allocator.

## An offline protocol exchange

[The WebSocket example](examples/ch28_protocol.coax) uses the independent
RFC 6455 masked `Hello` client vector and a separately authored unmasked
server frame. The fixed mask is test data; a real client gets fresh masking
entropy from its transport contract.

<!-- checked-source: examples/ch28_protocol.coax -->
```jai
main :: () {
    mask := [4] u8.[0x37, 0xfa, 0x21, 0x3d]; client: [11] u8;
    written := ws_frame_write(WS_OP_TEXT, true, "Hello", mask[..], client[..]);
    #assert written == 11;
    print("client frame: % bytes\n", written);

    // Server input is an independent unmasked fixture, not the client output.
    server := [7] u8.[0x81, 5, 'H', 'e', 'l', 'l', 'o'];
    frame := ws_frame_read(cast(string) server[..], 5);
    #assert frame.status == WS_FRAME_OK;
    print("server payload: %\n", cast(string) server[frame.header..<frame.total]);

    partial := ws_frame_read(cast(string) server[0..<6], 5);
    #assert partial.status == WS_FRAME_MORE;
    print("partial frame needs more bytes\n");
    oversized := ws_frame_read(cast(string) server[..], 4);
    #assert oversized.status == WS_FRAME_TOO_BIG;
    print("oversized frame refused\n");

    short: [12] u8; for i: 0..<12 { short[i] = 0xa5; }
    #assert ws_frame_write(WS_OP_TEXT, true, "Hello", mask[..], short[0..<10]) == -1;
    for b: short { #assert b == 0xa5; }
    print("short output refused without writes\n");
}
```

<!-- checked-source: examples/ch28_protocol.out -->
```text
client frame: 11 bytes
server payload: Hello
partial frame needs more bytes
oversized frame refused
short output refused without writes
```

The writer returns a byte count or `-1` and checks output capacity before
writing. The reader distinguishes a complete server frame, incomplete bytes
and a frame larger than the caller's cap. Its offsets select a payload from
the original input; they do not own or retain it. Keep that input alive
while using the payload.

A masked client frame cannot be fed into this server reader as a round
trip. Frame parsing also does not establish TLS, authentication, complete
fragmented-message validity or permission to perform a command. The
[network contract](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/net/README.md)
describes the separate connection and admission layers.

## A tiny Psionic CPU consumer

[The Psionic example](examples/ch28_psionic.coax) loads the existing tensor
metadata and scalar-learning libraries. A 2×3 row-major `f32` tensor has
six elements, 24 bytes and final element index five. No tensor allocation
is hidden inside the metadata call.

<!-- checked-source: examples/ch28_psionic.coax -->
```jai
main :: () {
    shape := PsionicShape.{rank = 2, dims = [4] int.[2, 3, 0, 0]};
    layout := psionic_tensor_layout(shape);
    #assert layout.status == PsionicTensorStatus.ok;
    index := psionic_tensor_index(shape, [4] int.[1, 2, 0, 0]);
    #assert index.status == PsionicTensorStatus.ok;
    print("tensor: % elements, % bytes, index %\n", layout.elements, layout.bytes, index.offset);

    prediction := psionic_predict(1.25f32, 3.0f32);
    #assert prediction.status == PsionicStatus.ok;
    print("prediction: %\n", prediction.value);
    refused := psionic_predict(1.25f32, 17.0f32);
    #assert refused.status == PsionicStatus.number;
    print("out-of-profile input refused\n");
}
```

<!-- checked-source: examples/ch28_psionic.out -->
```text
tensor: 6 elements, 24 bytes, index 5
prediction: 3.75
out-of-profile input refused
```

Check each status before its value. The prediction call admits finite inputs
within its `[-16,16]` profile and multiplies two `f32` values; `17` is refused.
This example exercises a small CPU contract, not a model or device backend.
The [Psionic book](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/README.md)
contains the full tensor, training and GPU contracts.

The example does not manufacture an address-overlap observation. Native
copying uses the separate tensor-memory adapter, which observes real spans
before Coax decides whether to copy. Passing `false` to an internal
`*_checked` overlap parameter would not establish that two allocations are
disjoint.

## What these recipes establish

Each `.coax` source has an exact `.out` fixture and authored checks. Native
boundary tests additionally exercise the selected generated programs at
`O0`/`O2` under ASan/UBSan. The shared sequence's own checks and mutation gate
cover its stated operations; adoption records compare the affected Verse
and Psionic consumers. Reuse that evidence while those inputs remain
unchanged.

These checks cover the named contracts and executions. They do not turn all
of `coax/lib/` into a stable package, establish a new target matrix, or make
raw native pointers safe. Follow each entry's storage, errors, units and
link requirements. In particular, the tool JSON writer sets `bad` on an
exhausted buffer and can leave partial text; discard it rather than treating
it as an atomic replacement.

When adding a recipe, choose one existing public boundary, retain independent
expected output and refusal cases, and record its limits beside the code.
Use the existing books for broader systems instead of copying their internal
implementations into the example.
