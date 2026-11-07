# Explicit backend allocation admission

`backing-budget.coax` charges an observed backend allocation before it occurs.
It uses the existing `HostInboxQueue` and exact slot/serial/epoch leases;
it does not allocate memory, create GPU objects or replace logical RCM1 costs.
The bounded native stream carrier now adopts it through
[device-bound plans and observed allocation lifetimes](native-backing.md).
The original component/SDK qualification remains separately scoped.

Eight named lines retain the existing resource categories 0–6 and add persistent
renderer storage at 7. The caller declares each line's byte cap, aggregate cap
and per-allocation cap. A grant reserves exactly the observed allocation size,
counts once until physical destruction, and records per-line/aggregate peaks
and the earliest lease that established each peak. Equal later peaks retain
the original attribution. Concurrent allocation pressure uses the existing
64-ticket table. The supported bound is 8 MiB per allocation and 32 MiB total;
it is not a claim about the machine's available physical memory.
These counters and peaks describe reservations. An admitted attempt can fail
before the SDK allocates memory; the actual allocation trace remains separate.

`BackingNeed` holds bytes, alignment, memory-type mask and selected memory type.
Its typed validation distinguishes size, alignment range, non-power-of-two
alignment, mask range, index range and unsupported type. Vulkan alignment is
an offset constraint; it need not be smaller than the allocation. Whole
dedicated allocations bind at offset zero. The observations come from the
actual SDK object, not an author's estimated payload size.

`backing_hold` returns invalid, category, aggregate or inbox refusal before
publishing a lease. Named-category and aggregate refusal leave all existing
owners and serials unchanged; only the refusal count advances. Release reads
the byte size and category from canonical live storage, so a caller cannot
forge another size/category or spend a duplicate, superseded or old-epoch
lease. `backing_reset` advances the existing inbox epoch only when the ledger
is empty; exhausted epochs retire instead of wrapping. Construct a budget
once, then reset it after full physical teardown, rather than replacing it
with a fresh epoch-1 value during device replacement.

The additive `vks_buffer_try_admitted` / `vks_image_try_admitted` factories
create a private unbound resource, query its real memory requirements, select
a supported memory type, then call the supplied `VksMemoryGate` before
`vkAllocateMemory`. Only `VK_SUCCESS` authorizes allocation. Other callback
results return unchanged after destroying the unbound object. A missing gate
refuses without SDK effects. Original factory entry points retain their prior
behavior through the same implementation, with no gate.

The gate must reserve its caller's Coax budget and retain the exact lease.
If a later SDK phase fails, the caller releases after the factory returns
and has unwound its private objects. On success, release follows final GPU
use and actual destruction. The SDK never silently releases caller leases.
The callback may inspect requirements and make its admission decision; it
must not mutate the device or private resource being built.

The [verification record](../../docs/verification/aethyr-streaming-12990/walk-native-backing-gate.md)
pins the 148-mutant policy gate, independent counted model and actual
hardware/software Vulkan cases. These checks do not establish corridor
adoption, a complete device-bound allocation-plan companion, renderer/readback
coverage, browser transport planning or opaque SDK/driver memory accounting.
