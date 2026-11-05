# Canonical layer bytes and actual GPU upload

All 38 original compressed Everglade pack layers now decode through the
same Coax image authority in a separate browser Wasm reactor. Every byte
of every complete layer/mip chain matches the native bank: 53,127,800 bytes,
zero differences. A separate actual normal-browser WebGPU capture compares
every logical texel after upload and readback, again with zero differences
across all 38 slices and ten mip levels. The physical readback is 53,533,184
bytes because WebGPU row copies align small mips to 256 bytes; all 38,874
logical rows are compared without omitting a logical byte. The row padding
is transfer storage, not an image mask.

`layers-browser.json` retains each original input and complete decoded
output comparison. `layers-gpu.json` joins the actual complete GPU file
to the native bank and metadata. Both source fences are unchanged. The
authored pack SHA256 is
`1c3fcbd6af5bac6b05639cab360725b798c3eb2ff16df0f71512ea68cf7433bc`;
the canonical native bank SHA256 is
`0b402736990723ee15869fa7c6d595dee1a99b7e29d34643f784cd892291872c`.

`htx_profile`, `htx_plan` and `htx_decode` own profile extent, compressed
PNG/JPEG admission, image arithmetic, doubling, alpha-weighted mip chains
and the established premultiplication round trip. The browser performs
allocation and direct raw `writeTexture` effects. It performs no additional
SDK resizing, decoding or premultiplication. The reactor exports scalar
projections and measured capacities; each decoded result is copied before
another layer can reuse the workspace. The normal v1 adapter selects this
path; pinned pre-v1 engines retain their existing image-bitmap path. A
capture-only override permits a separately identified frozen experiment.

This is an intentional new v1 asset identity. Historical browser assets
used SDK high-quality scaling and decoding and differ materially from the
existing native bank, including base-level bytes. The old complete-frame
failure is preserved by the native host record; these new receipts do not
reinterpret it as passing or claim identical historical browser visuals.
The native bank itself is unchanged. Full frame comparisons and an explicit
old-to-new visual record remain separate from byte identity.

The saved same-Metal-interpreter FP16 old/new frame comparison measures
14,025 of 230,400 channels beyond 0.002, maximum absolute error
0.138671875, with zero nonfinite values or masks. Both retain the same
producer uniform/HUD/draw/index/vertex/storage/puff byte identities; only
the layer bank changes. This is recorded as a failed historical comparison,
not as a parity pass. The supplied 320×180 images are retained as
`layers-old.png` and `layers-canonical.png` for direct visual review.

The new observer authority has checks written before implementation and
kills all 38 root mutants, with no survivors or equivalent annotations.
The shared layer authority/profile's already recorded 75-mutant gate is
reused without repeating it. The reactor is emitted by the original
admitted historical compiler SHA256
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`.
This is current pure-Coax host input through the historical compiler,
not a current compiler release. Its measured release size is 55,132 bytes
against the explicit 70,000-byte web budget. Browser modules and C scalar
facade are SDK effects; both observers and the build tool are Coax.
Shell/module syntax, the explicit size budget and whitespace checks pass.
The required repository tools check reports two unrelated pre-existing
Nostr relay Python files without `Firm reason` headers; all new files in
this checkpoint have the required Coax or SDK-effect form.

Artifacts are retained under
`~/.openagents/aethyrforge/host-contract-12869/layers-bridge/`:
`browser-third/` contains all decoded outputs, `canonical-frame/half/`
contains the actual complete producer, buffers, GPU textures and frame,
and `emitted/`/`release-emitted/` retain generated C and headers. Initial
failed browser starts remain separately recorded; the successful bridge
supplies the reactor's required WASI clock effect. The full new canonical
HALF frame also passes the actual Metal interpreter and GPU recovery:
zero of 230,400 channels exceed the unchanged 0.002 bound, zero nonfinite
or masked values, maximum error 0.0013427734375, exact pre/post recovery
bytes and closed ledger zero. Source/binary and validation details remain
in the Metal owner's `replay-canonical-half/` artifacts.

The same complete canonical HALF RFI also executes on the RTX 4080 Vulkan
SDK with all local/remote/SDK source fences unchanged, successful owned
cleanup and validation enabled with zero errors or warnings. Both the
browser–Vulkan and Metal–Vulkan comparisons retain 27 of 230,400 channels
beyond 0.002, zero nonfinite or masked values, within the inherited live
allowance of 2,000. These are FP16 graph results. They do not qualify the
ordinary eight-bit display path or supersede its separately retained
failure. `layers-browser-vulkan-half.json` and
`layers-metal-vulkan-half.json` preserve the exact comparisons.

Reproduction, with a new output directory each time:

```sh
nice -n 10 coax/web/tool.sh coax/web/build-layers.coax ROOT OUT.wasm
nice -n 10 coax/web/tool.sh aethyr/host/check-browser-layers.coax \
  OUT.wasm NATIVE_LAYER_FIXTURE_DIR NEW_BROWSER_OUT
nice -n 10 coax/web/tool.sh aethyr/host/check-layer-readback.coax \
  BROWSER_CAPTURE_DIR NATIVE_LAYER_FIXTURE_DIR NEW_REPORT.json
```

Production web opt-in, complete Vulkan/Metal frame parity, remaining host
services and the desktop default decision remain open. This component
checkpoint closes neither issue and changes no desktop default.
