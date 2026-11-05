# Original compiler provenance and new complete frame captures

This replaces the earlier **CPU compiler preparation**, using the native
observer's unchanged admission check. It does not qualify a current
compiler release or close the host/default exits.

Historical commit `c224561457` has the exact compiler-input fingerprint
`b9ab17ba5e4b6eb808fa4a93ac7e2d3db3c91cf2053519dce5a7b62923447e00`.
The original admitted compiler, original helper and original four-line
`provenance.record` verify against that checkout. Their SHA256s are
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`,
`9bab7b7fc990b0562aff10899e3bdfa5379e6f7a73f7ee83982fdce2e5fd3fdb`
and `334e90156db2e51e8999d3fdf60e5de4e74e8e9c35e0c3a9c4dd24b4f41913de`.
`qualified-original-provenance.record` is an unchanged copy of that original
record, rather than a newly authored provenance receipt.

`prepare-historical-root.coax` checks a clean historical worktree and
verifies the original receipt before adding separately pinned product
inputs from `2f88ce4f12`. It replaces product directories while preserving
every compiler/provenance input, including compiler/protobuf libraries and
the SHA kernel. The kernel is identical at both commits. The original
receipt verifies again afterward. Exact historical compiler inputs and
current product inputs are retained in separate archives. This is explicitly
a historical compiler plus current product snapshot.

The compiler's separate `source-digest` is
`386db301954b87b24d07ea2d175643563eb437c613467d3adfd70f249071b598`
before and after preparation. Its algorithm/scope differs from the build
fingerprint above. This value is observed only after the original receipt
verifies against actual compiler sources; it then satisfies the native
observer's existing matching-source check. No iteration compiler or
reconstructed digest supplies that admission.

The admitted compiler emits the current pure-Coax producer, shared renderer
and native texture policy. The dev/O1 realm Wasm is 3,585,961 bytes, SHA256
`af8a8f08cd4d2691ac27df554c68761730b37e87d8cab21861e76975260f1d75`.
Its bounded build took 420 seconds (409 Coax, 10 clang). Native C/header
emission profiles retain exact sources and generated hashes. Frozen
WGSL/SPIR-V/Metal products retain their separate #12867 shader qualification;
this preparation does not republish or newly qualify that graphics compiler.

Both actual browser captures pass the production observer with stable
source fences. They retain complete 320×180 producer words, eight textures
with mip chains and seven authored HUD images. The private surface copies
the pinned public page/assets, adds the shared renderer selector and
preserves the ordinary `/worlds` alias of `/realms`.

The native observer admits the complete snapshot and source-joined backend,
then executes 136 actual draw commands on the RTX 4080. Both formats have
zero validation errors/warnings. Source, payload, SDK and output fences
remain stable, and owned remote cleanup succeeds.

| Complete readback | Values past existing tolerance | Existing allowance | Result |
| --- | ---: | ---: | --- |
| Browser–Vulkan RGBA16Float | 53 / 230,400 | 2,000 | pass |
| Metal–Vulkan RGBA16Float | 53 / 230,400 | 2,000 | pass |
| Browser–Vulkan ordinary BGRA8/RGBA8 | 7,020 / 230,400 | 2,000 | **fail** |

All have zero nonfinite values and zero masked pixels. The 8-bit failure
is retained with the strict comparator: maximum absolute error 0.1294117645;
first differences are one 8-bit step. This remains a presentation conformance
gap. Half-float success cannot hide it or authorize a default change.
No tolerance, allowance, shader or mask was changed.

The subsequent optional [presentation experiment](presentation-experiment.md)
quantizes all three retained half-float frames once using the same existing
Coax conversion. It preserves every strict failure and measures the cost
of changing accumulation precision; it is not a production format switch.

The Metal owner separately records zero channels past tolerance against
the new half capture, exact pre/post recovery bytes and zero closed-resource
ledger. Its BGRA8 strict comparison retains eight channels beyond tolerance,
within the inherited live allowance. The final Metal SDK/source join remains
owned by #12868, separate from this CPU preparation.

Full immutable artifacts are under
`~/.openagents/aethyrforge/host-contract-12869/qualified-frame/`:
`compiler/`, `preparation/`, `surface/`, `native/`, `half/`, `rgba8/`,
`vulkan-half/`, `vulkan-rgba8/` and comparison receipts. The exact prepared
payload is `qualified-native-prepared.json`; `qualified-frame-artifacts.sha256`
joins provenance, preparation, emissions, complete readbacks and verdicts.
The earlier iteration records and withdrawn admission remain unchanged.

No release/deploy command or production access was used. Lifecycle adapters,
ordinary 8-bit presentation, Linux audio/output, interactive producer parity
and the final desktop/default requirements remain open.
