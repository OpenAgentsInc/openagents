# Canonical still media bytes

`host-image.coax` is the version 1 portable PNG/baseline-JPEG base decoder.
It consumes original compressed bytes, preserving natural rectangular
dimensions, and returns straight RGBA8 after exactly one premultiplied byte
round trip. It creates no mip levels and performs no SDK resize or colour
conversion. Pre-contract browser media keeps its existing SDK path.

```coax
HostMediaPlan :: struct {
    status:int=IMG_BAD;
    width:int; height:int; bytes:int; work:int; source_bytes:int;
}
htm_plan(src:[]u8) -> HostMediaPlan
htm_decode(src:[]u8, work:[]u8, out:[]u8) -> int
```

Both functions export the typed native C ABI. The plan admits dimensions
1–2048 on each axis, at most 8 MiB of compressed source, at most 32 MiB of
workspace, and exactly `width*height*4` output bytes (at most 16 MiB). A
refused plan is status `IMG_BAD` (3) with every other field zero. The codec
may refuse a format within these size bounds; bounds alone do not promise
that every PNG/JPEG variant is supported.

Source, workspace and output belong to the caller and must be distinct.
Source remains immutable during both synchronous calls. No pointer is
retained. Allocate the measured workspace and complete output, call decode,
and publish the complete output prefix only after status `IMG_OK` (0).
Insufficient capacity returns `IMG_SMALL` (4) before writing output. A late
codec failure may alter scratch output but never publishes success; discard
the entire output on any failure. There is no platform-decoder fallback
after refusal in the canonical path.

The GPU adapter uploads these bytes directly to the shared media descriptor's
RGBA8 texture; it does not premultiply them again. The root mutation gate
kills 47/47 mutants with no equivalents. Checks cover independently generated
rectangular PNG/JPEG pixels, exact allocation limits, untouched short-buffer
output, malformed compressed input and the transparent-byte round trip.
