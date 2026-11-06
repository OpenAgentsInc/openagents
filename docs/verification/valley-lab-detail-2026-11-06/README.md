# Valley World: first original full-source GPU page

This continues `2dd5aa90133ffea7f0fc1631a2610b6b15116feb`. The complete
27,501-placement world, original 334-joint Echo, nine cells and bounded walking
surface remain available. One nearby instance of the original sandstone rock
can now use **1,997,616 original source triangles**, replacing its **23,562
fallback triangles** (84.8 times the triangle count). This is an ordinary
indexed WebGPU draw in both camera and shadow passes, with the same original
placement, material and component override.

## Source and scope

The read-only Unreal SDK inventory finds 253,525,214 Nanite input triangles
and 129,821,687 input vertices across all 130 mesh types, versus 1,705,175
unique triangles in the existing fallback pack. No separate HiResSourceModel
is available in these assets. GLTF Exporter's `export_source_model=true`
exports the original source LOD0 mesh description instead of render fallback
data. This does not decode Unreal's cooked Nanite cluster format.

The first full-source mesh is
`/Game/AncientContent/Megascans/3DAssets/RockSandstone/SM_RockSandstone_026.SM_RockSandstone_026`.
The SDK export has 5,992,848 corners; deduplicating complete, exact F32
position/normal/UV tuples yields 1,012,930 vertices. Both pages preserve every
triangle and original corner order. The independent Python oracle compares
all 5,992,848 corners with the original glTF binary, including every F32 bit
in the reference page. Neither source geometry nor the private pages enter Git.

Original data stays under
`~/.openagents/aethyrforge/valley-lab/world-export/Saved/world-detail/`.
The two pages are staged by symlinks in
`~/.openagents/verse/private-build/web/`. The existing 1,565,719,372-byte
world pack and its SHA-256 remain unchanged:
`2066386e413e390eadc06aa9ae1b22fe115d645fe400fbf1393790c0930d699a`.

| Page | Bytes | SHA-256 |
| --- | ---: | --- |
| `valley-detail.page`, Q16 | 40,178,400 | `8f7f401b6acf1dbf7a2a4e69ffff75bd03a4ca1e7340f164f2ef11008c45c1bd` |
| `valley-detail-reference.page`, F32 | 56,385,280 | `652031658f97cc103a590f8c5aafa4cfa838f3626fe85517d09d1c5743576b17` |

## Page, compression and ownership

VDP1 version 1 has a 128-byte little-endian header: magic/version/encoding,
vertex/index counts, vertex/index/total byte offsets; F32 position lower/upper
bounds at32/44, UV lower/upper at56/64; position/normal/UV error bounds at72/76/80;
fallback first/count at84/88, original material ordinal at92, and the 32-byte
source-binary SHA-256 at96. Encoding1 uses 16-byte vertices (XYZ Q16, octahedral
normal Q16×2, UV Q16×2); encoding2 uses eight exact F32 words. Indices are U32.
Logical vertex IDs start at8,000,000 and12,000,000 respectively. Exact artifact
identity pins the complete header, payload and index namespace before publication.

The Q16 page is 28.7% smaller overall, with half-sized vertex records. Its
measured maximum local position error is **0.0000271670374 metres**, below
the published **0.0000339668004-metre** half-cell Euclidean bound. Maximum
normal component error is0.000056483832 and UV error0.00000762517772.
World-position error also depends on the original placement's affine scale.

Coax owns complete-byte identity, per-chunk stream extent admission, fallback
and profile selection. The browser executes fetch, SHA-256 and GPU upload.
Each page is bounded by64MiB. Source bytes remain outside the unchanged64MiB
Wasm memory; they are immutable index/storage GPU buffers. The legacy host's
optional version1 index companion selects a GPU asset and physical first-index
word separately from the existing draw record. Existing realms retain the
original index buffer ABI.

Both optional pages may remain pinned for comparison. The explicit legacy
8,000,000,000-byte profile accounts7,522,905,088 bytes at the tested framebuffer,
including five64MiB reservations for the combined retained page bytes, stream
chunks and GPU uploads. Aligned immutable pages do not allocate a second
upload copy. SDK/driver/GC internals are not measured process memory. This
does not qualify the shared renderer's separate1GiB ledger or native hosts.

K cycles fallback, Q16(default) and F32 reference. One eligible original
instance is selected inside18m of the camera and retained to22m while its
centre remains in the viewport. New choices prefer projected radius; all
other instances keep complete fallback geometry. This is a bounded distance
prototype, not a screen-error-qualified LOD cut. L shows profile and instance.
G retains the independent reference/pass-cut/cluster diagnostics.

## Actual GPU observations

Every comparison reads all2560×1600 RGBA bytes with simulation time held.
The frozen Q16/F32 bounds remain mean RGB error≤0.25 levels/channel and at
most0.001 of pixels differing by more than8 levels in any channel.

| View | Mean RGB error | Fraction over8 | Pixels changed by original detail |
| --- | ---: | ---: | ---: |
| Basin | 0.0010590820 | 0.0000085449 | 23,845 |
| West | 0.0016097005 | 0.0000190430 | 39,658 |
| East | 0.0034130046 | 0.0000307617 | 69,987 |
| North | 0 | 0 | 0 |

The initial visibility check failed at north: its selected instance is
occluded by other original scenery. That red observation is retained. North
is an occlusion control, **not evidence of added visible source detail**.
The three visible-detail views each require at least1,000 changed pixels;
all four keep the same compression error thresholds. No Hi-Z selection or
source-detail coverage of the northern rocks is claimed.

Physical browser checks cover missing pages, one-byte corrupt responses,
both HTTP requests held while the complete root renders, subsequent complete
arrival, and device replacement with requests held. Corrupt completions from
the retired device neither publish nor increment replacement refusals;
only the new device's complete pages become ready. Already loaded Q16 and
F32 profiles each recover with an identical state digest and every full-frame
RGBA byte identical. Frames remain bounded to two unfinished submissions.

The shadow cull also now matches the shader's actual depth projection:
`z=0.5-dot/(4*range)` admits±2range on its depth axis, rather than±range.
Camera and shadow still have independent cuts. The fallback-only unculled
reference remains the pixel oracle for both accelerated cuts.

Coax authority gates kill **72/72** page-policy and **147/147** runtime mutants,
with no equivalents. Focused World checks pass24/24; codec checks pass2/2.
The source oracle, emitted shader check and browser receipts are beside this
file. Reproduction entry points are `aethyr/tools/valley-world-detail-sdk.py`
(SDK only, inventory batches of16), `valley-world-detail-pack.coax`,
`valley_detail_oracle.py` and `coax/web/valley-detail-check.coax`.

A separate broad audit of the existing, non-authority fallback selector
reports371 mutants,230 killed and141 survivors in its older traversal,
interval-padding and plane helpers. That report is retained; it is not a
zero-survivor qualification and no mutant was marked equivalent. The changed
shadow-depth extent has red/green checks and the complete GPU image oracle.
The two new admission authorities have their separate zero-survivor gates
above. The first Slay regression used an outdated compiled page after its
private source pack changed concurrently; it correctly refused the foreign
SHA-256. Its red receipt is retained before rebuilding against merged main.

The merged release Wasm is120,975 bytes within the124,000-byte limit;
its original64MiB memory maximum is unchanged. The budget increase from
120,000 accommodates the optional page/index ABI and checked detail selection.
The shared host merges main's retained original-pack cache and source storage
limit request, and keeps Valley's own groom extent when sharing Slay's frame.
The fresh [source-detail inspector](source-detail.png) and `preview.log`
also verify the retained local runtime at
`http://127.0.0.1:8895/worlds/valley-world?pack=valley-world`, outside the
implementation worktree. `*-first-host.log` observations precede the merge;
unqualified filenames record the combined host and regenerated shader.

The full-groom Slay regression is **not a passing recovery gate**: both
concurrent and isolated runs miss its unchanged GPU deadline. A separate
server running the unmodified `origin/main` host at
`e5f853cc73b03b821c5990fc4bfbeef70efae470` reproduces the same failure and
the same four pre-loss frame hashes. See `slay-main-control-red.log`,
`slay-isolated-deadline-red.log` and `slay-concurrent-deadline-red.log`.
This retains the already recorded Slay exit at `4edd12daa4`; no deadline was
raised and no Slay recovery parity is claimed. Valley's separate loaded-page,
held-request and desktop/phone-viewport navigation recovery gates pass.

## Remaining Nanite work

Only one mesh type and one instance at a time use the new full-source page.
The other129 types remain render fallbacks. Fine geometry has no cluster
hierarchy, simplified parents, measured projected error, adaptive source LOD,
GPU-driven traversal, Hi-Z occlusion, demand paging or eviction. Both full
pages are retained rather than managed by a general GPU page cache.
Unreal lighting, material and hair parity, exact triangle collision and
overhang traversal also remain separate work. This is a real source-detail
milestone within the navigable world, not Nanite or full Unreal parity.
