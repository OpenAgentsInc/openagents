# Material-role mip checks

`gpu.log` and `gpu.json` retain a headless Vulkan check on an NVIDIA RTX 4080,
driver 595.71.05. The fixture generates its own chamber and replaces one image
with a 4×4 black/white RGBA checker. The same image serves color, normal,
metallic-roughness, occlusion, emission, and masked material roles.

Actual level-1 GPU readbacks are `[188, 188, 188, 128]` for sRGB color,
`[128, 128, 128, 128]` for linear scalar data, and `[128, 128, 255, 128]` for
opposed normals. The mask keeps two of four texels. A shader samples all five
actual imported material bindings into a linear RGBA8 target; color and emission
return about 0.5 linear RGB, and the data maps retain their expected values.
A 64×64 bilinear material probe records 50% masked coverage. The test deletes
its source directory, destroys the device, and verifies identical channel and
mask pixels after recovery from retained CPU data.

`receipt.json` records the public source base, exact implementation hashes,
executable hash, and commands. The Git commit containing these files is the
publication revision. `library-tests.log` retains the edited crates' tests;
`consumers.log` checks desktop/mobile Rust consumers, and `web-check.log` checks
the browser target. GPU tests are ignored by the ordinary library run and this
one is run explicitly. The library run passes 579 Verse and 136 engine tests,
with 14 GPU/platform tests ignored. No owner host, display, relay, credentials, phone suite,
release gate, or Clippy run is involved.

Reproduce with the pinned Rust toolchain and a warm target directory:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1"
cargo test -p verse -p verse-engine --no-default-features --lib
cargo test -p verse --no-default-features --lib \
  uploaded_roles_and_material_bindings_match_linear_light_and_mask_recipes \
  -- --ignored --nocapture
cargo check -p verse --no-default-features --features web \
  --target wasm32-unknown-unknown
```

Run the GPU test with a temporary `HOME` and `XDG_RUNTIME_DIR`, unset `DISPLAY`
and `WAYLAND_DISPLAY`, and set `WGPU_BACKEND=vulkan`. A machine with a native
adapter is required. Library tests cover odd-sized images, quantized normalized
normals, thin stems, oversized-mask fitting, material opacity, role separation,
and variant byte reservations.

These are correctness checks, not frame-time measurements or production foliage
sign-off. RGBA8 quantization and a fixed bilinear calibration grid approximate
coverage. One texel cannot hold a covered fraction; nonempty masks retain one
visible texel. Tied alpha ranks can alter silhouette shape. Trilinear transitions,
anisotropy, vertex alpha, extreme cutoffs, and oblique views need art acceptance.
The portable cooker runs during upload; persisted compressed or authored mip
chains remain V20. The physical glTF loader also applies material recipes when
fitting oversized images. Phone and browser image quality has not been measured.

The hardware and full library evidence precedes a rebase onto main's movement
storage-backpressure fixes. Implementation fingerprints remain identical after
that rebase; `rebased-tests.log` records 13 textured-material tests, four imported
material tests, and the variant resource test passing on the integrated tree.
