# Canonical CPU image cache

`host-image-cache.coax` owns 112 immutable source identities, root-relative
still-image admission and a 128 MiB aggregate retained decoded-byte budget.
Each decoded allocation has one slot owner. Reserve before allocation;
free its physical buffer before releasing the reservation. Unsupported,
missing, damaged, capacity-exhausted and allocation-failed images become
failed/not-ready observations. They do not freeze unrelated rendering or
fall back to a platform decoder. The source cap remains the canonical
decoder's 8 MiB, workspace 32 MiB and natural extent 2048×2048. A source
identity is immutable for an engine/cache owner, including failure.

The portable SDK is `metaverse/native/render-images.{c,h}`. `RhgiSource.size`
observes the physical encoded extent before the module allocates its source
buffer; `read` returns 1 only after filling exactly that buffer. Zero and
negative results refuse. The caller owns URL mounting, physical root
containment and actual file/transport effects. The module performs no URL
routing, decoding arithmetic or notice formatting in C.

`rhgi_get` returns borrowed canonical RGBA, dimensions, slot and decoded/
failed facts. Cached views last until close. A refused view borrows the input
URL for the synchronous caller; it creates no retained negative entry once
the cache is full. `decoded` does not assert GPU readiness. Upload and ledger
ownership remain with the renderer. Feed unique actual GPU ready/failed
observations to `rhgi_notice`, which forwards them unchanged to the shared
Coax `htn_format`. Failed notices use 0×0. `rhgi_close` frees physical pixels,
releases their owners and returns the actual remaining Coax byte total.

Emit `native-images.coax` as the SDK's `RenderTexture.c/.h` to obtain the
existing texture/decoder/notice exports plus the additive `hic_*` ownership
API. Replace the old native-texture object; do not link both copies. No
shader or large producer emission is required.

The component record is
`docs/verification/metal-host-2026-11-05/image-cache.md`. Ordinary Metal/
Vulkan adoption and actual GPU readiness remain separate from CPU cache
qualification.
