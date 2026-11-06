# Linux bundled live images

`render-live-media.c` connects the ordinary Vulkan adapter to the shared
`rhgi` canonical image cache. Coax owns URL mapping and physical containment
admission, source/work/output bounds, 112 cache identities, the 128 MiB decoded
owner, media composition, GPU ledger and 607 formatting. The SDK opens the
file, checks the actual opened descriptor's canonical path, reads its exact
observed extent, and uploads the returned raw RGBA bytes without SDK decoding.
Regular-file identity, size and timestamps must remain unchanged across read.
Failed images remain failed/not-ready while unrelated images continue.

The actual RTX workload reads all seven authored JPEGs through this adapter,
uploads and reads back every one of their 64,512 RGBA bytes, replaces the
physical Vulkan device, then uploads the retained CPU cache again. Both
readbacks are exact. The old ledger is empty after destruction; the seven CPU
identities and 64,512 bytes survive device replacement. The resent notice is
byte-identical. CPU and GPU owners are zero after close.

Four further physical workloads replace the first image with a missing file,
malformed metadata, a PNG that fails only during decode, and a symlink to a
valid JPEG outside the physical mount. Each produces the exact zero-dimension
failed notice, retains seven identities/55,296 decoded bytes, and leaves all
other six images drawable. The escaped file is refused through the actual
opened-descriptor containment check. The final observation authority kills
48/48 root mutants with no survivors or equivalents.

The same 58 observations pass with generated Coax, SDK and platform C compiled
under ASan/UBSan, including leak detection, on actual Lavapipe. NVIDIA's ICD
refuses ASan instance initialization before adapter work (`vkCreateInstance`,
error -9); its failed attempt is retained and is not an instrumented RTX pass.
The normal RTX and instrumented Lavapipe runs both enable Vulkan validation
and report zero errors/warnings. No sanitizer suppression is used.

This workload exposed two retained failures. An empty-directory listing called
`qsort(NULL, 0, ...)`, which UBSan rejects; the SDK now sorts only when more
than one entry exists. The first observer counted all media receipt records
instead of their actual `draw` flags, wrongly reporting seven drawable images
in the failure cases. Its corrected SDK observation leaves policy unchanged.
The final normal and instrumented receipts pass after both corrections.

One physical generated C closure owns the codec, renderer and observation
exports. Its successful profile and actual C source table join all 73 inputs
to a closed source snapshot with zero differences. Linux admitted 0.50 image
`5cc82481bf6db529e919a404134b6d18b02c984f443809af7426767af869af9c`
retains its original verified 9b1 compiler root/receipt; product overlays and
SDK snapshots remain separate. Small receipts, build/executable hashes and the
SDK snapshot manifest are in `records/linux-live-media/`. Full C/header/profile,
sources, binaries, raw failures and outputs are retained on both machines at:

```
~/.openagents/aethyrforge/host-contract-12869/linux-live-media/
```

This qualifies physical file/cache/GPU image effects and replacement-device
ownership. It does not claim ordinary world frames, rendered media quads,
producer/network continuity or a Vulkan default. Those integration gates
remain separate.
