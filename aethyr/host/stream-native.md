# Native streaming observation profile

`tests/stream-walk-native-gpu.coax` runs the engine-owned corridor through
the existing walker, resource owners, canonical codecs and draw list.
`tests/stream-walk-vulkan.c` executes headless Vulkan effects with the
existing `verse/gpu/vkc` and `vks` SDK. It creates no window, input capture
or audio output. This is a bounded native observation carrier, separate
from the ordinary realm host and the published host-v1 ABI.

The Coax driver authenticates the pinned scene, Vulkan RCM1 companion and
actual local container/source bytes. Each normal load reads its source
after admission; the world is not eagerly made resident. Native HTTP and
asynchronous file workers are outside this profile. The browser consumer
retains its separate actual HTTP observations.

## Ownership and effects

Coax controls selection, the fixed-step clock, load/retirement commands,
render borrows, fixed grants and replacement order. The SDK binds admitted
48-byte vertices, u32 indices, decoded RGBA8 mip chains and the Coax shader.
It holds every render borrow through the actual submission fence, then
returns it. Cached pipeline descriptors close before a bound resource.

In this carrier only, `SW_VULKAN` makes the existing SDK allocation field 4
a mapped `VkcBuffer`. Its resource staging lease and a concrete renderer
buffer grant both precede creation. `vks_upload_buffer` consumes that same
allocation, without another transfer copy. `sw_host_finish` releases it
after the completed fence; the fixed buffer grant releases afterward.
The ordinary malloc/free and browser carrier retain their prior path.

Persistent uniforms and the 320×180 color/depth attachments have separate
existing renderer grants. Readback and replacement transfer buffers each
receive a temporary grant before allocation. `vks_read_buffer` reads into
the already owned mapped allocation. Source/decoded bytes remain retained
under their original resource tickets.

The observer records dynamic allocation/free events and per-stage timing,
logical GPU bytes, actual `VkMemoryRequirements.size` totals/peak, and
fixed core/SDK metadata sizes. Explicit Vulkan backing is measured; it is
not called opaque driver memory. Internal pipeline/driver allocations,
allocator overhead, process RSS and the observer's fixed stack/report
storage are separate. Current RCM1 category accounting uses logical GPU
bytes: this carrier does **not** qualify those caps as physical Vulkan
allocation budgets. Complete backend planning remains #12990 work.

## Controlled replacement

After 32 warmup frames the driver captures all 230,400 RGBA8 bytes, the
canonical simulation SHA-256 and a row/length/retained-source digest. It
drains physical effects, closes all old pipelines/resources and the old
device, invokes the existing atomic GPU-handle replacement, opens a new
device, and rebuilds from the retained CPU spans. No source read or paid
simulation frame occurs between the compared captures. Sources, queued
logical ownership and canonical state remain in their original carrier.

The complete image must match byte-for-byte and contain more than one
color. Every granted visual owner must upload again. A continued draw and
the optional full 768 m out-and-back route then use the replacement device.
Final cleanup requires all resource, inbox, decoded image/audio, renderer
and concrete SDK owners to reach zero. Automatic loss, interrupted uploads,
replacement failure and rescue are separate unfinished exits.

`stream-native-conformance.coax` admits sixteen independently supplied
observation words: version; nonempty initial GPU set; equal full image;
nonuniform raster; equal canonical state; retained CPU count/source identity
and no new reads; no paid frame; old device closed; all grants reuploaded;
epoch advanced; resumed draw; validation errors; validation warnings; final
SDK objects; final ledger objects/words; complete drain with validation
enabled. Its root mutation gate kills 23/23 without equivalents.

## Run and inspect

```sh
coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD [--sanitize]
NEW_BUILD/stream-native SCENE_DIR SCENE_SHA256 NEW_BUILD/stream-draw.spv NEW_OUTPUT TIER --route
python3 aethyr/tests/streaming_native_oracle.py SCENE_DIR NEW_OUTPUT NEW_BUILD/stream-draw.spv --fault-checks
```

The installed Vulkan SDK must be discoverable through pkg-config; the
receipt requires `VK_LAYER_KHRONOS_validation`. Each command has a bounded
deadline. The build uses the installed C compiler and Coax's own SPIR-V
emitter. No external shader compiler or source-asset tool runs.

The independent oracle reconstructs allocation lifetimes/costs, dependency
publication, exact binary32 walking, source identity and every recovery
readback byte. It rejects 22 altered histories. It retains paid-step
canonical hashes but does not independently serialize or replay this new
native GPU history; the earlier browser-to-native CPU replay qualification
has its own input and backend profile. It does not infer shader numerical
reference qualification from a nonuniform image.

Current hardware/software scope and exact pins are in the
[native receipt record](../../docs/verification/aethyr-streaming-12990/walk-native-gpu.md).
