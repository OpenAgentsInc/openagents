# Memory safety in Coax (proposal)

Written 2026-11-05. The general guarantee remains a proposal, and the
spec's current rule still holds: "Do not promise general memory safety"
([spec.md](spec.md) §3). This doc says why we should change that rule, which
approach to take, what the guarantee would cover, and what stays trusted.

The mechanisms borrowed from Epic's Verse language are specified in
[from-epic-verse.md](from-epic-verse.md), along with the issue roadmap.
The host-buffer question is decided below (2026-11-05): each host buffer
gets a region tied to one lease, starting with scoped `#lease` blocks.
Enforcement is still proposed.

## Why

- **Agents write almost all Coax.** Coax is the implementation target for
  every OpenAgents surface: Metaverse realms, the OpenAgents Metaverse Engine, the Metaverse OS
  session, the CLI, Psionic. Agents write most of it. A memory bug an agent
  writes is a bug nobody reviewed line by line.
- **Finding bugs got cheap.** On 2026-11-04 OpenAI reported about 1,000
  validated Linux local privilege escalations from an automated pipeline.
  Most were memory-safety bugs. Attackers can buy the same capability, and
  memory bugs are what it finds first.
- **The isolation plan depends on it.** The
  [Metaverse OS isolation proposal](../docs/os/2026-11-05-verse-os-isolation.md)
  puts the identity and wallet in a Coax signer and proposes Coax virtio
  device models. Both parse attacker-controlled bytes. "Small and checked"
  has to include "no memory corruption" for that plan to hold.
- **Checks need it.** `#forall`, `#authority` and the mutation gate check
  logic. A program that can corrupt its own memory can break that logic
  without any checked line being wrong.
- **It's cheap now.** The repository has about 1.2 million lines of Coax and
  no raw pointer declarations: `*T` is target syntax the bootstrap doesn't
  implement, and allocation (`New`, `free`, `destroy`, `[..] T`) is planned
  library work. Ordinary Coax is already in the safe subset. Deciding before
  pointers and allocators ship costs a design decision. Deciding after costs
  a migration.

## Where Coax stands

| Property | State |
| --- | --- |
| Bounds | Indexing and slicing trap in debug and release (E0410). No build mode removes the check. |
| Integer overflow | Checked arithmetic traps unless `--arithmetic wrapping` is chosen for a unit. |
| Casts | Checked. |
| Initialization | Scalars start at zero, structs use field defaults, uninitialized storage needs `---`. Deterministic and compile-time code reject reading it. |
| Sum types | `T?`, `Result(T,E)` and nominal unions with exhaustive `switch`; reading the wrong alternative is a fatal tag fault. |
| Raw pointers | Not implemented. |
| Allocation | Not implemented in Coax; native hosts own allocation. |
| View lifetimes | Partial. E0383 rejects known local backing returned directly or retained in caller or static storage ([verification-return-borrows.md](verification-return-borrows.md), [verification-retained-stores.md](verification-retained-stores.md)). Recursive and unmodeled helper relations, regions and transfer lifetimes are open (R3). |
| Views in records | Allowed and common (for example `EntityStore.gen: [] int` in `aethyr/core/entity.coax`). Their lifetime isn't tracked. |
| Persistent references | `Handle(T)` and `Store(T)` check generation and retire exhausted slots. The Metaverse Engine and Everglade use typed lifetimes; saved state uses stable IDs. Store provenance and view regions remain caller obligations. |
| Foreign calls | About 1,100 `#foreign` declarations in 184 files. Each is trusted. |
| Concurrency | No jobs, atomics or race checking yet (§12). |
| Effects | Procedure bounds are enforced, including transitive calls. Everglade's shared rules exclude clock, I/O and foreign effects. Native summaries remain trusted. See [checked-state.md](checked-state.md). |

Spatial safety is mostly in place. Temporal safety (dangling views), the
foreign boundary and future concurrency are the gaps.

## The guarantee we should aim for

Proposed wording for spec §3, to adopt with the default safe profile after
the required enforcement lands (recommendation 8):

> **Safe Coax is memory-safe.** A unit compiled under the safe profile, with
> no `#unsafe` block and no foreign call, cannot read or write memory outside
> a live object it was given, cannot use an object after its region ends, and
> cannot race with another job. Where the compiler can't prove an access
> safe, it inserts a check that fails (`none`, `err`) or traps. Unsafe code is
> explicit, inventoried and reviewed.

For a view supplied by a boundary, this guarantee depends on that boundary
honoring its declared extent, lifetime and access contract. A region annotation
does not verify the C host or the device; those remain outside the claim.

What that covers:

- Out-of-bounds reads and writes (already true).
- Use after free, use after arena reset and use after container growth.
- Stale persistent references.
- Data races between Coax jobs.
- Reading uninitialized memory (already true for deterministic code; extend
  to all safe code).

What it doesn't cover, stated in the spec next to the claim:

- **The C host.** Vulkan, DRM, evdev, PipeWire, sockets and dmabuf glue are
  hand-written C, roughly half a million lines of C and headers in the
  repository by a rough count.
- **Foreign libraries and drivers**, including NVIDIA's closed userspace.
- **GPU programs.** SPIR-V, WGSL, Metal and CUDA output follow each target's
  memory model and driver.
- **The toolchain.** The frontend, the C emitter and the host C compiler are
  trusted to preserve meaning ([spec.md](spec.md) §14 already says this for
  the proto importer).
- **Logic bugs, leaks and denial of service.** A safe program can still be
  wrong, run out of memory or loop.

## Options

### 1. Second-class views, regions and handles (recommended core)

- **Views are borrows tied to a region.** Every view carries the region of
  its backing storage: a procedure's frame, a named arena, a container's
  current storage, static data, or a lease on host-owned storage as decided
  below.
- **A view can't outlive its region.** Returning it, storing it in a longer
  region or keeping it across the event that ends its region (arena reset,
  container growth, frame return) is a compile error. E0383 is the first
  piece of this.
- **Records that hold views take the shortest region of their views.** This
  keeps `EntityStore`-style structs, which hold views of their own columns,
  while stopping them from escaping.
- **Long-lived references are handles.** `Handle(T)` is a nominal slot and
  generation. A lookup returns `T?` or a view: a stale handle gives `none`
  (see [from-epic-verse.md](from-epic-verse.md), A2).
- **No lifetime annotations in ordinary code.** Regions are inferred within a
  procedure; a procedure's signature names regions only when it returns a
  view (as the returned-borrow summaries already do internally).

Pros:

- It extends rules the compiler already has instead of adding a second
  system.
- It matches engine code: ECS slots, structure-of-arrays columns, per-frame
  and per-level arenas.
- No garbage collector or per-access region bookkeeping; handles and dynamic
  host leases account for ownership at lookup/acquire/release.

Cons:

- Each handle lookup costs a load and a compare.
- Graph-shaped data has to go through handles.
- Some APIs get clumsier (a function can't hand back a view into a
  temporary).

#### Host-owned buffers: decision (2026-11-05)

**A host buffer gets a region tied to one lease.** A lease is one
acquisition of one buffer: a Vulkan mapping, a dmabuf CPU mapping or a
camera frame. A region label for the whole host, such as `region(host)`,
isn't enough, because it can't tell one acquisition from the next.

- Acquiring and releasing a buffer happens in a reviewed `#boundary` module
  and carries `foreign` and `unsafe` effects. Wrappers keep those effects;
  a region annotation never hides them.
- The view a lease gives follows the ordinary region rules. Code that only
  receives the view (indexes it, passes it on) is safe and doesn't need
  `#unsafe`.

**Why a lease and not the mapping's lifetime.** A buffer can stay mapped long
after the CPU may safely touch it. Our hosts already show this:

- [The camera host](platform/camera-native.c) dequeues a V4L2 frame, copies
  it into caller storage and requeues it before returning. The mapping
  outlives the requeue; the frame's contents don't. A view without copying
  would have to end before the requeue. Today's copy is already safe.
- [The Vulkan helper](../verse/gpu/vkc.c) keeps buffers mapped for their
  whole life, but reads them only after a host-read barrier and a fence
  wait. The CPU may read between the fence and the next submission, not
  for as long as the buffer is mapped.
- [Psionic's tensor placement](../psionic/lib/tensor-placement.coax)
  counts pending work and readers and refuses release while any remain.
  Regions have to keep those counts, not replace them with a label.
  [Read-only views](readonly-views.md) likewise stop the reader from
  writing; they don't stop another alias from writing.

##### Scoped blocks first

The first API is a block. There is no lease value to store, copy or return:

```jai
#lease readback read {        // acquire: wait for the fence, then map for reading
  sum := checksum(readback);  // readback is [] const u8, valid only in here
}                             // release: the view ends, the GPU may write again
```

- The view, any slice of it and any record that holds it can't leave the
  block.
- Code inside the block can't release, requeue, unmap, resize or submit
  device work that writes the same bytes.

Coax can't stop a lease value from being copied yet: R3 hasn't built
ownership that can't be copied ([results.md](results.md) says a copied
ticket is "fenced", not prevented). A lease that outlives one block waits
for that. Blocks let S3 ship without it.

##### What's checked when

| Check | When | Who enforces it |
| --- | --- | --- |
| The view and anything holding it stay inside the block | Compile time | The region checker |
| No release, requeue or conflicting submission inside the block | Compile time | The region checker, with the boundary's effect declarations |
| The span (size, alignment, readable bytes) and access mode | Acquire | The adapter |
| Reader and writer counts per buffer range | Acquire and release | The adapter, in Coax, with the counts kept as Psionic's placement does |
| Fences, flushes, invalidates, dequeue and requeue | Acquire and release | The adapter's C |
| A stale acquisition can't return | Acquire | The adapter, by generation |

The compile-time rows are proven by the compiler. The other rows depend on
the adapter being right, and its C part is trusted, so each adapter is
tested on its own (S3b, S3c).

##### The adapter's rules

1. **One owner, one span.** The adapter keeps the allocation alive and states
   the span's type, size, alignment, readable bytes and access mode. Only a
   boundary can create a lease. Copying a view doesn't create a new owner or
   extend the lease.
2. **Access starts after synchronization.** The lease starts after the
   acquire and any fence, invalidate or dequeue it needs. It ends at the
   block's end.
3. **Readers and writers exclude each other.** Readers exclude writers; a
   writer excludes everyone else on the same bytes. Two imports of the same
   memory share one count. Coherent memory or a read-only mapping doesn't
   change this. A CPU lease ends before any device work that writes the
   same bytes is submitted. Device work keeps its own claim until it
   actually completes.
4. **Old leases can't come back.** Cancellation, device loss and host
   replacement stop new acquisitions. Outstanding leases and device work
   finish before the storage is released or reused. Cancelling work is not
   the same as the work completing. Generations retire instead of wrapping.
   A platform that can revoke access in the middle of a lease can't offer
   safe views.

**Fallback.** When an adapter can't meet these rules, copy into Coax-owned
storage while access is valid, or keep the access inside `#unsafe` with the
unmet rules written as its reason. The camera keeps its copy path until a
lease adapter passes S3c.

S3 is split into three issues (see the roadmap in
[from-epic-verse.md](from-epic-verse.md)):

- **S3a: scoped `#lease` blocks in the compiler.** Views can't escape;
  release and conflicting submission inside the block are refused.
- **S3b: the Vulkan readback adapter** in `verse/gpu`, the first real
  consumer. It already has the fence.
- **S3c: camera and dmabuf leases without copying.** Only after a
  measurement shows the copy costs too much.

### 2. A Rust-style borrow checker

Ownership, lifetime parameters, and "one writer or many readers" for every
reference.

- Pros: static, no runtime cost, proven at scale.
- Cons: the largest possible R3, which already blocks R4, R11, R14, R15 and
  R23; it fights structure-of-arrays, arena and zero-copy GPU patterns;
  lifetime syntax breaks the "one form for ordinary work" rule; slower
  compiles.

**Not recommended.** Option 1 covers the same bugs for the code we write.

### 3. A hardened build of emitted C and the C host

Build generated C, and the hand-written C host, with a memory-safe C
toolchain (Fil-C-style capability pointers) or for CHERI hardware.

- Pros: it reaches the C host and foreign glue, which language rules can't.
  Coax emitting C makes this a build profile, not a port.
- Cons: about 1.5–4× slower; no help on the GPU; closed drivers stay
  outside.

**Recommended for small, attacker-facing programs** (the signer, device
models, parsers, the update agent), not the frame path.

### 4. Bounded model checking of emitted C

Run CBMC or ESBMC over the C that Coax emits for restricted units (no
allocator, bounded loops, no foreign calls).

- Pros: proves "no out-of-bounds, no overflow, no assertion failure for all
  inputs up to N" for small code; no runtime cost; extends the `#authority`
  habit from checking logic to checking memory.
- Cons: only scales to small units; needs a profile the checker can digest.

**Recommended for the same small units as option 3.**

### 5. Garbage collection

Epic's Verse is memory-safe this way: a managed VM, collected objects, no
pointers. It's ruled out for Coax: "No mandatory GC" is a non-goal
([spec.md](spec.md) §21), collector pauses don't fit a 16.6 ms frame, and GPU-
shared memory can't be traced. See [from-epic-verse.md](from-epic-verse.md)
for what we take from Verse instead.

## Recommendation

Do all of these, in this order:

1. **Make it a rule now: ordinary Coax has no raw pointers.** When `*T`
   ships, it ships only inside `#unsafe`. Record this in spec §3 and in
   `INVARIANTS.md` as a design rule before any pointer work starts.
2. **`#unsafe` and enforced effects.** Foreign calls, raw pointers and
   unchecked casts are allowed only in `#unsafe` blocks or in boundary
   modules. The effects inventory becomes enforced, with `unsafe` and
   `foreign` as effects a caller must declare. Safe units can then be
   identified by the compiler, not by convention.
3. **Regions for views (option 1).** Finish R3's view lifetimes as regions,
   including records that hold views and the lease-scoped host regions
   decided above. Foreign acquisition stays at the unsafe boundary.
4. **`Handle(T)` with failable lookup.** Move `aethyr/core/entity.coax` from
   packed `int` handles to the nominal type.
5. **Structured jobs before any parallelism.** R15's jobs bind scratch and
   views to the job's scope and allow overlapping writes only on proven
   disjoint regions. No unstructured spawning in safe code.
6. **Close the undefined-behavior gap in emitted C.** Build generated C with
   `-fno-strict-aliasing` and `-fwrapv` (or prove no signed overflow
   reaches C, which checked arithmetic already mostly does), and run the
   Coax test suite's emitted C under UBSan and ASan in CI.
7. **Hardened build and bounded model checking (options 3 and 4)** for the
   signer, device models and parsers.
8. **Then change the claim.** Adopt the wording above in spec §3 and
   `INVARIANTS.md` in the same change that turns on the safe profile by
   default, with the "doesn't cover" list beside it.

### Costs to plan for

- **Speed.** Handle lookups and lost aliasing assumptions cost time in hot
  loops. Measure on the Metaverse Engine's step and the realm frame before and after.
  `#unsafe` opt-outs need a stated reason and show in the inventory, so they
  can't spread unseen.
- **Roadmap.** Regions add proof obligations to every later feature
  (containers, context, jobs, GPU buffers, device graphs). That's the price
  of making the claim at all; doing it before those features exist is
  cheaper than retrofitting.
- **The C host stays unsafe.** Shrink it over time: every effect the host
  does in C is a candidate to move behind a narrow, typed foreign interface
  with a reviewed effect summary.

## Open questions

1. What does a stale handle cost on the realm frame at the 99th percentile,
   and is a trap-on-stale variant needed for hot loops?
2. Which hardened C toolchain works on both macOS and the NixOS hosts?
3. Does `--arithmetic wrapping` stay allowed in safe units, given that
   wrapping is defined but often wrong?
