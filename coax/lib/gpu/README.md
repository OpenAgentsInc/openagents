# Shared GPU admission

The generic GPU ledger and frame/range checks live here so the engine and
Coax Native use one implementation without a framework dependency on a world
or engine host. They moved from `metaverse-engine/render/` in #13645.

`frame.coax`, `types.coax` and `registry.coax` keep their original logical
module IDs (`metaverse.engine.render.*`), declaration names, field layouts and
behavior. Canonical `#load` imports now point here. The old source paths remain
thin compatibility loads; bind module aliases to these canonical files.
`frame-checks.coax` moved with the implementation. Engine-specific resource
descriptors, passes and SDKs stay with the engine.

The generic texture-accounting codes in `formats.coax` have a distinct type.
The engine's existing public `RenderTextureFormat` enum retains its identity;
its boundary checks prove the numeric mapping remains unchanged.

Resource creation follows `rr_acquire`; release waits for the last GPU use.
Keep canonical CPU data for recovery. A fresh SDK host/device invalidates its
physical objects and replays the retained data. Handles, generations and
word limits are ledger decisions, rather than a second SDK allocator.
