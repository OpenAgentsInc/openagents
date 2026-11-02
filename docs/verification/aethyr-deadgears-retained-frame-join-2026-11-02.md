# Dead Gears: retained frame and geometry joins

The input/source review passes. This record does not change the retained full
image failures or assert an interior-comparison result. It joins the separately
derived geometric envelope to the actual recorded frame; the new comparator
must still admit the pinned profile and inspect every image value.

The retained [runtime record](data/aethyr-ch11-2026-11-01/canonical-box36/runtime/qualification.json)
uses profile 4, 320×180, two indexed scene records and 4,500 HUD vertices.
The scene records contain 36 vertices × 293 instances, base instance 96, then
240 vertices × 42 instances, base instance 0. These are the 20,628 indexed
vertex invocations and 6,876 triangles in the input-only geometry census.
The background and HUD remain part of the complete image comparison.

The actual files were hashed again for this read-only review. Their values
match the vertex-generation preflight, both CPU measurement input maps and the
native execution input map. The [observations](data/aethyr-ch11-2026-11-02/deadgears-frame-join/observed-sha256.txt)
retain full paths and SHA-256 values.

| Input | SHA-256 |
| --- | --- |
| Uniforms | `2869fce687e2aea32c87adb365cd5b66e4b6df0215a35d0706c7043d1e1a9a03` |
| Object storage | `f837e11b94fe44e63e352fedcf0cc8f2a9d13888374a11f5426b198ae76f050c` |
| Draw records | `e843237a265a3c33f073d05086c705e3718530215ad60cb2396219ee56ab1157` |
| Indices | `c89db7222126863309183fc023c7091fb18392d16a397dac76a96a022cd62cef` |
| HUD | `8921a58764557f6985bbe962ae23174c27149cd603f42a7918b28e2f1f1bfc24` |
| Shader package | `def947afb78d14e9320f0f7c74a612e105b81d8cee4fd82bbfd03596bb8d4e7f` |
| Nominal vertex packets | `62823c676088a5296d1ca46bc265aafe93b1946dec1f60268ee05818474d3302` |

The package embedded in the actual browser capture matches that manifest.
Its scene entry names `deadgears_scene_vs` and `deadgears_scene_fs`, storage
slots `u` and `objects`, checked tree
`22f1044355279e5a87772f9c71dc5ea71377f85c39690fc4d5cff5387efae6d1`,
SPIR-V `c8f21a5b4ed893857ab6a7f6e7b154a0e86489b151779436a288aec53811af04`,
and WGSL module
`60a7f653b4c7c10e4367a376211673d0a2b3c0899fd1b59fcf8175a88baeaaa8`.
Both CPU input maps contain that scene tree. The native execution map contains
the same manifest, scene tree, SPIR-V and all producer inputs, with every
recorded source fence unchanged.

The actual package's source observations also match the current files for
`apps/openagents-web/gpu/deadgears.coax`, `verse/gpu/shader.coax`,
`verse/gpu/wgsl.coax` and `aethyr/render/ring8.coax`, byte for byte. Their full
hashes are in the observations. The checked-in convenience `deadgears.wgsl`
is not used as a source identity. The nominal packet's case list, generated
native kernel, header and driver match their retained preflight hashes too.

The browser CPU profile is nearest-positive, eight subpixel bits, guard-band
65,536; the native CPU profile is nearest-even with the same bits and guard.
Those match the envelope's two declared profiles. The GPU capture reports
`rgba16float`, tight 2,560-byte rows and successful frame validation. Native
Vulkan reports the RTX 4080, eight subpixel bits, zero validation errors and
zero warnings. There are no physical texture or media-image resources in
this captured frame; its media text is empty.

| Complete image | SHA-256 |
| --- | --- |
| CPU, both declared profiles | `c4825c7daaccb36b839b637b143c9d677bf7afa8069f6afa21c766632d510ccc` |
| WebGPU | `eea19f17c7756c4880d63995deee07aa6781d36de19a46aeea203b128247ac59` |
| Vulkan | `08fe3735b8646336168abe0d0c035efb72a7e23a98dfc05c734bac44832765e0` |

The original unmasked comparisons still fail three RGB channels at (42,76),
maximum normalized error 0.005706787109375, with zero nonfinite values. The
geometry-only envelope was generated independently from the pinned nominal
packets and profiles, with no image argument. It contains 159 complete pixels;
all 636 RGBA channels in those packets remain unqualified. This is a declared
conservative boundary exclusion, not a theorem about legal hardware depth
arithmetic, a visible-edge detector or a GPU vertex-output bit-identity claim.

The CPU and native execution artifacts retain their actual older 308 compiler
and reference-geometry epoch. This review does not promote them to newly
emitted current-main artifacts or attribute the later Everglade water repair
to them. The exact frame archive remains SHA-256
`71dee4fa4847f624301c966f15cd29bfb734d88531376dad93bfa724e92b6fe9`;
the separate precision-research archive retains the nominal packets and their
generation inputs. No producer, GPU frame or unchanged numerical gate was
rerun for this review.
