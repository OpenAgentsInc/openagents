# Admitted graphics image and exact product join

The original cached Mac compiler receipt now verifies against the protected
compiler inputs in `36d59fe390`. The image is SHA256
`3f8d0824d17ac7cc5c6ed843c755a7b8aafac741f02c21aeaa799f72cb9fac12`,
in cache `aab3c90f9d08d99cd4ad5f8d24986dfe96fb4db6911f3e65d48b7726e7367c56`.
The original `provenance.record` and helper are used unchanged. Verification
passes after shader regeneration. No receipt is reconstructed or replaced.

The unchanged normal `coax/build.sh` also installs that checked cache in the
separate compiler audit worktree at `d31d9e806a`. Its actual log reports
`installed the checked build aab3c90f9d08`. The normal installer produces
`coax/bin/source-digest`, value
`1fb12b56582dc2107199d2bbaa426701bce85703d1e2dd22c5485fed7ea5ecad`.
That sidecar is retained as `warm-source-digest`; it is not represented as a
sidecar that was present in the original cache. The host worktree's original
598 tool-runner symlink and its cached target are untouched.

## Actual regenerated products

At the frozen product source `36d59fe390`, the existing `metal-build.coax
ROOT shaders --coax IMAGE --out OUTPUT` completes normal portable-package
and Metal generation. The [portable manifest](graphics-render.json) names
the actual new compiler image, every source hash and all WGSL/SPIR-V/checked
tree products. The [Metal manifest](graphics-metal.json) binds its MSL and
reflection products to those exact portable manifest bytes.

The [byte observation](graphics-product-source.json) checks all 51 products:
three WGSL modules, twelve SPIR-V modules, twelve checked trees, twelve MSL
files and twelve Metal reflection files. All match the earlier frozen
products byte for byte. Nonmedia MSL/reflection match the existing Metal
resource set; entry13 matches the retained quantized-presentation experiment.
All 47 manifest source inputs also match the frozen current source tree.
The seven existing G5 pass qualifications and exhaustive actual three-SDK
presentation byte observations therefore retain their exact shader bytes;
their unchanged GPU gates are not rerun. Their original compiler scope stays
recorded, while this regeneration has its own actual admitted-image identity.

The new portable manifest SHA256 is
`b42d187ed9bcca4222fd2ab7b1e2b9d5f62a0eed0ef6be3e6ce2d41b327ad617`;
the Metal manifest SHA256 is
`45bbe4d7914ec0eb01fc7d8f15ea663fdb68d25eef14db525368d63661b0164a`.

## Integrated boundary

The first new-package replay intentionally retains the old browser RFI as
input. Its authenticated package identity differs, so the production SDK
refuses before rendering: `Frozen frame and authenticated product package
differ`. No configuration hash is rewritten. A fresh actual browser capture
against the new package is required for integrated replay/recovery.

That fresh browser capture subsequently completes with the source-qualified
current `17a742c08c` shared carrier and the exact new package. Its full RFI is
retained in the sibling's `graphics-admitted-3f8/browser/` directory. The
production Metal SDK replays it with API/GPU validation enabled,136 actual
draws per frame, two completed frames and pending0. Resource recreation
preserves both complete FP16 graph and actual UNORM display bytes exactly.
Old-generation live objects are0; final closed ledger objects/words are0/0.
The [actual SDK receipt](graphics-replay.json) retains those observations.

The [complete displayed comparison](graphics-replay-comparison.json) reports
four of230,400 channels beyond the unchanged0.002 bound, zero nonfinite values
and zero masked pixels. The strict comparator correctly reports FAIL; the
existing full-live allowance of2,000 is unchanged. This fresh frozen-frame
gate does not substitute for actual native producer composition or the
display-rate/window gate.

Raw products, original receipt, post-verification log, normal installer log,
actual observation tool, complete frozen product-source archive and this
initial refusal are retained under
`~/.openagents/aethyrforge/metal-host-12868/graphics-admitted-3f8/`.
The original historical CPU constructor qualification remains separate.
The subsequent [native selector/JPEG record](graphics-selector.md) joins
current native composition and the shared precision selector. Display-rate,
window/room and common host service exits remain separate; the desktop
default and issue states are unchanged.
