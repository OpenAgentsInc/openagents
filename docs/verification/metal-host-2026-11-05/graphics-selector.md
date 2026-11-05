# Native contract presentation and frame JPEG

The Metal SDK consumes `render_host_presentation` from the shared Coax
selector at `282235f088`. A native v1/profile2 face draws the checked FP16
graph and presents through the admitted quantized media product. Invalid
eligible surfaces refuse before the producer or clock advances. Pre-v1
faces retain their earlier path. The ordinary desktop default remains web.

The SDK retains the actual UNORM presentation texture, then copies identical
texels into the same-format drawable. JPEG and display readback use that
texture. The earlier JPEG refusal on the FP16 graph is retained under
`graphics-admitted-3f8/jpeg-red/`; no provider request is involved.

The [normal SDK receipt](graphics-selector-jpeg.json) observes two completed
frames, JPEG26261bytes at320x180, device recreation, pending0, old live0 and
closed ledger0/0. Both complete FP16 and UNORM outputs compare byte-identical
to the earlier admitted-package replay; its four-channel strict residual
therefore remains unchanged. Metal API and GPU validation are enabled and
report no errors. The [instrumented receipt](graphics-selector-asan.json)
passes the same replay, JPEG and recovery with AddressSanitizer on the native
producer, generated renderer/texture interpreter and SDK glue. No sanitizer
report occurs; this does not claim instrumentation of Apple frameworks or
the separately loaded socket library.

The actual generated renderer's85-source table has zero differences against
the frozen product snapshot plus the single facade import. Its admitted3f8
image and original receipt are the ones recorded in [graphics-admitted.md](graphics-admitted.md).
The generated C SHA256 is
`6fd3b1a650e9ef07ff3b676c41526ad872b83d6c66a882e29a42add2e9eb55ea`;
header `5933c6da89f33ec6dd1c994a7709c98e2dac478ebb02e6203856c9f7d9fc6325`;
normal SDK driver `88ac9ac4764af84d4c92228de4136e69b63acca3877f55439d924dcf685ec461`;
instrumented driver `054a0d24ec097d3958f360173e707af8412d6f983cbe2c8a0b101062fb80c584`.
Raw source snapshots, generated C/header/objects, plans, readbacks and logs
remain under `~/.openagents/aethyrforge/metal-host-12868/graphics-admitted-3f8/`.

The [common admitted matrix](../host-contract-2026-11-05/graphics-admitted.md)
records Web–Metal4 and Web/Metal–Vulkan1886 channels beyond0.002, with no
nonfinite values or masks, within the unchanged live allowance2000. Its
direct actual-native90→Vulkan(matchedWasm90) comparison is1942 channels;
these counts are separate measurements and are never added.

The first live ordinary-selector window remains a failed observation:
83.50fps during initial surface adaptation, no room seat, and later lost
occlusion/focus. Its old desktop URL requests a non-local pair room, which
the room refuses with404; the exact native-local endpoint instead upgrades
with101 and streams world data. The checked native-entry fix and subsequent
window qualification are separate. These SDK receipts do not close #12868
or establish display-rate, complete session or default qualification.
