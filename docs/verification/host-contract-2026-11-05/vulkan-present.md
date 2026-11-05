# Vulkan display affordability

The actual Wayland/Vulkan SDK probe passes on coderos-4080's existing
`wayland-1`, using a private 320×180 window which closes at the end.
No VT is changed, no browser is opened, and no audio is used.

[The raw receipt](vulkan-present.json) records ten exact observations:

| Observation | Expected |
| --- | ---: |
| Offer while the actual frame fence is unsignaled | 0 |
| Producer event count after refusal | 0 |
| Admitted clock after refusal | -1 |
| Offer after the physical slot becomes available | 1 |
| Actual producer event count | 1 |
| First admitted frame time | 0 |
| Further unused image reservation before close | 1 |
| Validation errors after complete close | 0 |
| Validation warnings after complete close | 0 |
| Vulkan validation actually enabled | 1 |

The probe uses the real frame fence, swapchain acquire, production realm
SDK, and a zero-draw SDK clear/present pass. The Coax driver authors the
workload and result; its C file executes SDK calls. The semantic authority
kills 18/18 root mutants, without equivalents. An earlier strengthened
run correctly failed because the layer was absent from Vulkan's search
path; the final run names the concrete installed SDK layer directory and
requires the observed enabled fact. Zero counters alone cannot pass.

`vkp_reserve` checks the preceding submission fence and acquires an
available WSI image with zero timeout before producer advancement. An
unavailable resource returns immediately. Presentation consumes that exact
reservation. Resize and close consume an unused acquire semaphore before
retiring its swapchain; validation remains clear through complete teardown.
The existing blocking API remains available to explicit synchronous probes.

The ordinary Vulkan loop now pumps platform and network work before
attempting a frame. Contract-v1 faces use the shared Coax monotonic offer,
retain the unclipped requested vertex count for render admission, and have
no second host delta clamp. Their initial producer call waits for physical
affordability. Wayland focus and the versioned suspended-state event project
to the shared lifecycle; older faces retain their key-up fallback.
Both ordinary and shared-renderer C branches compile against their actual
SDK headers. The changed realm SDK also passes the common ten-row
lifecycle trace under ASan/UBSan with leak checking.

The executable, generated C/headers, SDK protocol products and authored
sources are identified by [the artifact list](vulkan-present-artifacts.sha256).
The durable Linux directory is
`~/.openagents/aethyrforge/host-contract-12869/present-final/`;
the changed sanitized realm SDK is retained separately in
`lifecycle-display-sdk/`. The existing Linux compiler identity is unchanged
from the earlier native SDK record.

This is a physical resource/clock probe. It does not qualify Everglade
multiplayer room continuity, actual minimize/occlusion on every compositor,
device replacement, input-method behavior, or the full production window.
Those remain separate exits, together with ordinary 8-bit GPU parity and
Linux audio. The desktop default remains web.
