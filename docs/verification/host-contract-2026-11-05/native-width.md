# Linux native-width producer ABI

The Linux realm dispatcher now uses the existing `cx_face_addr` projection
when built with `COAXIAL_NATIVE_ADDRESS`. State, vertex, uniform and borrowed
data pointers retain native width in the face, dispatcher and imports.
Counts, draw/storage words, indices and message lengths remain u32. The
unselected branch retains its original below-4-GiB ABI and allocator setup.
`verse/native/face-address.h` refers to the existing shared face header.

The check-first core fixture requires both its actual state allocation and
producer-owned asset allocation to exceed `UINT32_MAX`. The old dispatcher
refuses that workload. After pointer plumbing changes the same common core
event/asset/default/frame checker passes normally and under ASan/UBSan/LSan.
The instrumented run records addresses 132,066,183,192,576 and
132,066,183,192,744 with a 64-bit projection. The unchanged legacy branch
passes the common trace with addresses 1,106,518,016 and 1,106,518,184 and a
32-bit projection. These are observed allocations, not invented pointer
sentinels. Small actual receipts are in `records/native-width/`.

The pinned 85c producer reuses the four original unedited b587 C/H cache
units already joined in `current-85c.md`. GCC compiles the core with PIC,
strict floating arithmetic and hidden visibility, then links the matching
native-width face as `libeverglade.so`. Only the additive physical
`media_capacity` export is overlaid for the later media consumer. Its relink
reuses the measured unchanged core objects. Source, object and library
identities plus the actual dynamic symbol table are retained. No generated
C is rewritten and no duplicate-definition waiver is used.

Optional face defaults are compiled inside the producer DSO. The executable
omits its own copies under `COAXIAL_DYNAMIC_FACE`, so an executable's weak
fallback cannot preempt the producer's real export. The core's shared
renderer functions remain private; the executable owns its separate checked
render facade. This preserves one concrete owner per exported SDK boundary.
The static path still includes the same optional defaults, and packaging
copies their header alongside the existing face headers.

The ordinary Vulkan executable successfully loads this DSO and executes 90
golden frames on the RTX with validation enabled, zero errors/warnings and
zero ledger objects after close. That initial workload omits page facts and
does not qualify live media, ordinary room/audio services, current display
composition or device/session recovery. Its 49.07-ms median includes
validation and is not a frame-rate acceptance result. Full integration
continues separately.

Durable C/H, original cache/source joins, matching face snapshots, compilation
records, symbols, libraries and actual runs are under:

```
~/.openagents/aethyrforge/host-contract-12869/native-width/
~/.openagents/aethyrforge/host-contract-12869/current-85c/linux-producer/
~/.openagents/aethyrforge/host-contract-12869/current-85c/world-build/
```
