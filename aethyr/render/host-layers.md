# Canonical host layer bytes, version 1

`host-layers.coax` composes the existing checked Coax PNG/baseline-JPEG
decoder, doubled base image, alpha-weighted box mip chain and one
premultiply/divide round trip. It does not opt any browser host into a new
asset path. Pre-contract browser image SDK behavior remains separate.

`htx_profile(profile: int) -> HostLayersProfile` selects the authored output
side: only qualified Everglade profile 2 is currently admitted (`valid=true`,
`side=512`). Every other catalog returns false/zero. SDKs consume this
descriptor instead of selecting a side from a decoded image or a constant.

`htx_plan(src: []u8, side: int) -> HostLayerPlan` reads the actual compressed
image bytes. Status zero admits a square image at `side` or `side/2`; all
other plans contain zero extents and status `IMG_BAD` (3). The source is at
most 8 MiB, side is 1..1024 and measured decoder work is at most 4 MiB.
`width/height` describe the decoded source; `side/levels/bytes` describe the
complete output chain; `work` is the required scratch capacity. The plan
also retains actual `source_bytes` and the internal `base_offset`.

`htx_decode(src, side, work: []u8, out: []u8) -> int` receives distinct,
caller-owned buffers. The immutable source remains alive for the call.
It returns zero after writing exactly `plan.bytes` output bytes. An
undersized work/output view returns `IMG_SMALL` (4) before output writes;
malformed compressed contents may fail during decode and must not be
uploaded. No view is retained and no memory is allocated by Coax.

Output is straight RGBA8, level-major for one layer, with each level tightly
packed and square `max(1, side >> level)`. Native `verse/gpu/layers.coax`
interleaves these per-layer chains level first then array slice. Browser
effects must upload these bytes directly with `writeTexture`; an image SDK
copy would apply an extra premultiplication conversion.

The authority's 29 checks and 71 root mutants pass with no survivors or
equivalent annotations. These checks include exact capacity boundaries,
known PNG pixels, doubled levels, translucent bytes and late decode errors.
This does not qualify actual browser/native image parity: that requires
all authored original inputs and actual GPU readbacks, preserving the
old/new visual regression rather than reusing historical goldens.

The native tool's actual 38 authored layers (512 square, ten levels) were
regenerated through this API. All 53,127,800 bytes are identical to the
previous native bank, SHA-256
`0b402736990723ee15869fa7c6d595dee1a99b7e29d34643f784cd892291872c`.
Raw test, mutation and bank observations are retained at
`~/.openagents/aethyrforge/metal-host-12868/layers-*.log` and
`layers-canonical.rgba`. The historical browser bank differs; its failing
actual frame comparison remains in `handoff-current-capacity/` and is not
qualified by this native byte identity.

The required tools check reports seven pre-existing Python files without a
`Firm reason` header (two Nostr relay and five human asset tools); this
checkpoint adds only Coax and Markdown. Its complete output is retained in
`layers-tools.log`.
