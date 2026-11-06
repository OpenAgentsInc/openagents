# What Coax takes from Epic's Verse (proposal)

Written 2026-11-05. Status: proposal. Nothing here is implemented. Syntax in
this doc is proposed, not accepted by the bootstrap grammar.

"Verse" in this doc means **Epic's Verse language** for Unreal Engine 6 and
UEFN, not our Verse world. Sources: the
[Book of Verse](https://verselang.github.io/book/) (which tracks Epic's
development branch), Epic's
[road to UE6](https://www.unrealengine.com/news/the-road-to-ue-6) post, the
[AutoRTFM post](https://www.unrealengine.com/tech-blog/bringing-verse-transactional-memory-semantics-to-c),
and the [Verse Calculus](https://simon.peytonjones.org/verse-calculus/).
Facts about Epic's Verse are as described there on 2026-11-03.

The memory-safety plan this serves, and the options it chose between, are in
[memory-safety.md](memory-safety.md).

## Summary

Epic's Verse is memory-safe because it is managed: code runs on a VM with
collected objects and no pointers, and the only unsafe code is Epic's own
C++. That works because creators can't write native code. In OpenAgents,
agents write everything, including the C host, so Coax can't take that model.
It can take seven mechanisms:

| | Mechanism | From Epic's Verse | In Coax |
| --- | --- | --- | --- |
| A1 | Safe code and a native boundary | Managed code; native functions declared in `.native.verse` with generated C++ bindings | A safe profile; `#unsafe` blocks; foreign declarations only in boundary modules |
| A2 | Lookups that fail | `Array[I]` and map lookups are failable expressions | `Handle(T)` whose lookup returns `T?`; stale handles give `none` |
| A3 | Effects in signatures | `<computes>`, `<reads>`, `<writes>`, `<allocates>`, `<suspends>`, checked through calls | Enforced effect sets on procedure types, built on the effects inventory |
| A4 | Structured lifetimes | `sync`, `race`, `branch`, tasks owned by a parent, `defer` on cancellation | Structured jobs that own their scratch and borrows |
| A5 | Rollback | Failure contexts undo their writes; AutoRTFM makes C++ roll back | `#transaction` blocks for authority code, from a write log Coax emits |
| A6 | Compatibility of published code and data | Publication rules; persistable type rules | `coax compat` over exported ABIs, protocols and saved state |
| A7 | Owned lifetimes for world objects | Scene Graph components own their tasks; lifecycle stages | Entities own their handles, tasks and resources; despawn ends them |

And four things it doesn't take: garbage collection and a VM, failure as the
main control-flow form, single-threadedness as the answer to races, and
trusting the native layer.

## Threat model: why the line moves

| | Epic's Verse | Coax |
| --- | --- | --- |
| Who writes the untrusted code | Creators | Agents |
| Who writes native code | Epic engineers only | Agents |
| Where the safe/unsafe line sits | The language boundary (Verse vs. C++) | Inside the language (`#unsafe`), plus the C host |
| Runtime | VM, garbage collector | Native C11, no collector, GPU-shared memory |
| Frame budget | Server gameplay, cooperative tasks | 16.6 ms render frame, inference on the same GPU |

Epic can call native code trusted because one team writes it. We can't, so
safety has to reach the systems code, without a collector.

## A1. Safe code and a native boundary

**In Epic's Verse.** Creator code is all managed. Native functions are
declared in `.native.verse` files and implemented by generated C++ bindings;
`FNativeConverter` converts values across the line. A creator can call a
native function but can't write one.

**In Coax.**

- **Safe profile.** A unit is safe when it contains no `#unsafe` block and
  calls no procedure whose effects include `unsafe` or `foreign` (A3). The
  compiler reports which units are safe (`coax status --json`, `coax deps
  --effects`).
- **`#unsafe` blocks.** Raw pointers (`*T`, when they ship), unchecked casts,
  and calls to `#foreign` procedures are allowed only inside them. A block
  names its reason:

  ```jai
  frame_pixels :: (img: Image) -> []u8 #effects(unsafe) {
    #unsafe "the host keeps the mapped buffer until present" {
      return host_map(img.id, img.bytes);
    }
  }
  ```

- **Boundary modules.** `#foreign` declarations live only in modules marked
  `#boundary` (the `.native.verse` analog). Each foreign declaration states
  its effects, reviewed by a person or a reviewing agent and recorded with
  the declaration's source hash:

  ```jai
  #boundary;
  host_map :: (id: u32, bytes: int) -> []u8 #foreign #effects(foreign, reads, region(host));
  ```

- **Inventory.** Every `#unsafe` block and every boundary declaration shows
  in `coax deps --effects` output with its reason, so unsafe code can't
  spread unseen. A count per directory goes in `STATUS.md`.

**Migration.** The current ~1,100 `#foreign` declarations in 184 files move
into boundary modules, grouped by host (Vulkan, DRM, evdev, Metal, platform,
crypto). Callers don't change except to declare effects where A3 requires it.

## A2. Lookups that fail

**In Epic's Verse.** `Array[I]`, map lookups and casts are failable. They must
appear in a failure context (an `if` condition, a `<decides>` body), so a
missing element is handled where it happens, not by a crash.

**In Coax.** Coax keeps traps for indexing: a bad index is a bug, and the
frame path can't afford a branch for every access. But a stale **persistent
reference** is not a bug in a live world, because entities disappear all the
time. So handles fail instead of trapping.

- **`Handle(T)`** is a nominal type: a slot index and a generation, packed in
  a `u64`. Two handle types don't mix (`Handle(Ship)` isn't a
  `Handle(Planet)`), and a handle isn't an integer.
- **`Store(T)`** owns a fixed-capacity slot array, each slot's generation and
  a free list, as `aethyr/entity.coax`'s `EntityStore` does by hand.
- **Lookup returns an optional.**

  ```jai
  ship := store_get(ships, h);            // ([]Ship)? : a one-element view, or none
  switch ship {
    case none {}                          // despawned: nothing to do
    case some(s) { s[0].hull -= damage; }
  }
  hull := (try store_get(ships, h))[0].hull;   // in a procedure returning U?
  ```

- **The view a lookup returns is region-bound** to the store's current
  storage ([memory-safety.md](memory-safety.md), option 1), so it can't
  outlive a despawn or a resize.
- **Generation wrap retires the slot.** A slot whose generation would wrap
  is never reused, as `entity.coax` does today (`ENTITY_RETIRED`). This rule
  moves into the library and gets an `#authority` check.
- **A trap variant for proven-live code.** `store_get_live(s, h)` traps on a
  stale handle, for loops over handles the step itself just checked. It's
  safe (it traps, it never reads the wrong slot) and shows in the effects
  inventory as a trap site.

**Migration.** `aethyr/entity.coax` and its users move from packed `int`
handles to `Handle(Entity)`. The entity oracle (`aethyr/tests/entity-oracle.coax`)
checks both versions agree over the same spawn and despawn sequences before
the old one is removed.

## A3. Effects in signatures

**In Epic's Verse.** Effects are part of a function's type and checked
through calls: `<computes>`, `<reads>`, `<writes>`, `<allocates>`,
`<transacts>` (reads, writes and allocates), `<decides>` (may fail),
`<suspends>`, `<converges>`, `<predicts>`. A function may declare more than
it does, never less. Publication rules let a new version reduce its effects
but not add any.

**In Coax.** The effects inventory ([EFFECTS.md](EFFECTS.md)) already
computes may-effects for every procedure. A3 makes a declared set a contract.

- **The set.** Coax's effects map to the inventory's bits:

  | Effect | Meaning | Inventory bit |
  | --- | --- | --- |
  | `computes` | None of the below. Pure apart from traps | 0 |
  | `reads` | Reads storage through a borrowed view | 16 |
  | `writes` | Writes storage through a borrowed view | 32 |
  | `allocates` | Allocates (when Coax allocation ships) | new |
  | `io` | Host input or output | 1 |
  | `foreign` | Reaches a foreign procedure | 2 |
  | `unsafe` | Contains or reaches an `#unsafe` block | new |
  | `traps` | May trap (bounds, overflow, assertion, tag, stale-live handle) | 8 |
  | `clock` | Reads a clock or random source (for R13) | new |
  | `suspends` | May yield to a scheduler (for A4, later) | new |

  Coax has no `decides`: failure is a value (`T?`, `Result(T,E)`), and R2
  already makes handling it mandatory.

- **Where a declaration is required.** On `#export_c` procedures, on a
  module's public procedures, on every procedure in a `#authority` unit, on
  foreign declarations, and on procedure-typed parameters (callbacks).
  Elsewhere effects are inferred, as Epic's Verse defaults to `<transacts>`.
- **The check.** A procedure's inferred effects must be a subset of its
  declared set. A callback argument's effects must be a subset of the
  parameter type's. An error names the first call path that adds the
  effect, using the inventory's existing shortest-path witness.
- **Profiles use it.** "Safe" is "no `unsafe` or `foreign`". A job body
  (A4) can't have `io` or `suspends`. A `#transaction` block (A5) can't have
  `io`, `foreign` or `clock`. Deterministic code can't have `clock`. A GPU
  procedure can't have `allocates`, `io` or `foreign`.
- **`unknown` stays explicit.** The inventory's bit 4 (unknown effects)
  counts as every effect until a reviewed summary narrows it.

## A4. Structured lifetimes

**In Epic's Verse.** `sync` waits for all branches, `race` takes the first and
cancels the rest, `rush` takes the first and lets the others run within the
enclosing lifetime, `branch` starts work owned by the parent task, and
`spawn` starts unowned work. `defer` runs when a scope exits, including by
cancellation. On 2026-11-02 Epic clarified that a `branch` lives as long as
its parent task, not the function that started it. Verse is single-threaded
today; running transactions on many threads is roadmap work.

**In Coax.** Coax needs real parallel jobs (R15), so structure has to do two
jobs: own lifetimes and prevent races.

- **`#jobs` scopes.** A scope starts jobs and doesn't exit until every job
  in it has finished. There's no unowned spawn in safe code.

  ```jai
  #jobs {
    #job integrate(bodies[0..half), dt);
    #job integrate(bodies[half..n), dt);
  }   // both are done here
  ```

- **Scratch belongs to the job.** Each job gets its own scratch arena. Views
  into it can't leave the job (it's a region that ends with the job).
- **Writes must be disjoint.** Two jobs in one scope may write the same
  array only through ranges the compiler can show don't overlap: constant
  splits, or a split procedure whose `#forall` check proves disjointness. A
  shared read-only view is allowed. Anything else is an error, not a race.
- **Job bodies have restricted effects.** No `io`, no `suspends`, no
  `foreign` unless the foreign summary says it's thread-safe.
- **Cancellation runs `defer`.** A cancelled or failed job runs its defers;
  the scope reports the first error by value.
- **Determinism.** Results combine in index order, which R15 already
  requires, so replays match at 1, 2, 4 and 8 workers.
- **`race` later.** "First to finish wins" is useful for search and
  timeouts but makes results depend on timing. Add it outside
  deterministic code only, after the basic scope ships.

## A5. Rollback for authority code

**In Epic's Verse.** A failure context undoes its writes when it fails, so a
purchase that fails its last check leaves coins and stock as they were.
AutoRTFM extends this into C++: an LLVM pass clones functions, logs every
write's old value, and restores them on abort; frees wait until commit. It
can't undo external I/O.

**In Coax.** Rollback costs a write log on every store, so it doesn't belong
in the frame path. It does belong in code that moves money or grants
authority, where a half-applied update is the worst outcome.

- **`#transaction` blocks.** Writes inside the block are logged. If the block
  ends by returning `err` or `none`, or by `try` propagating one, the log is
  replayed backwards and the block's writes are undone before the error
  leaves. If the block completes, the log is dropped.

  ```jai
  pay :: (w: []Wallet, amount: Sats, to: Address) -> Result(void, Refusal) #effects(writes) {
    #transaction {
      try policy_allows(w[0].policy, amount, to);
      w[0].balance -= amount;
      try ledger_append(w[0].ledger, amount, to);   // if this fails, the balance is restored
    }
    return .{ok};
  }
  ```

- **Restrictions, enforced through A3.** No `io`, `foreign`, `clock` or
  `suspends` inside the block. Effects that can't be undone happen after it
  commits, from its result.
- **Traps don't roll back.** A trap is fatal, as today; the process state
  after a trap isn't trusted.
- **Coax writes the log itself.** Coax generates the C, so the emitter can
  log stores at the point it writes them. It doesn't need AutoRTFM's LLVM
  pass. Writes through `#unsafe` code inside a transaction are refused.
- **First consumers.** The signer's policy and the wallet (`verse/wallet.coax`)
  in the [Verse OS isolation proposal](../docs/os/2026-11-05-verse-os-isolation.md).

## A6. Compatibility of published code and data

**In Epic's Verse.** Published definitions can't be removed. A new version may
accept more (wider parameter types), return less (narrower result types) and
do less (fewer effects). Persistable types have strict rules: final classes,
no generic parameters, fields added only with defaults.

**In Coax.** Coax has published surfaces that outlive a build: `#export_c`
headers, proto schemas (already reviewed through `proto/`), saved realm
state, snapshots (R16) and state kept across hot reload (R26, R27).

- **`coax compat OLD NEW`.** Compares two versions of a module's published
  surface and refuses:
  - removed or renamed exported procedures and types;
  - changed C ABI layouts (`type_info` and `field_info` already give them);
  - narrowed parameter types or widened result types;
  - added effects on an exported procedure;
  - persisted record changes other than appended fields with defaults.
- **`#persisted` records.** Records that reach saved state, snapshots or the
  wire are marked. They can hold only values, fixed arrays, optionals and
  handles: no views, since a view can't outlive the process.
- **Handles across saves.** A persisted handle is saved as a stable ID, not a
  slot and generation, and resolved on load (the entity store already keeps
  ids beside handles).
- **Release gate.** `coax/RELEASE.md`'s release checks run `coax compat`
  against the last published build of each exported surface.

## A7. Owned lifetimes for world objects

**In Epic's Verse.** Scene Graph entities form a hierarchy, components attach
to entities, and a component's suspending `OnSimulate` work lives as long as
the component. Removing the entity ends it.

**In Coax.** Aethyr's entities already have handles, ids and a despawn that
waits for the end of the step (`entity_flush`). A7 makes ownership explicit:

- **An entity owns its resources.** Child entities, GPU resources, sounds and
  jobs registered against an entity end when it's despawned, in a defined
  order (children first, then the entity's own resources).
- **No dangling world references.** Anything that refers to another entity
  holds a `Handle(Entity)` (A2), never a view into its storage.
- **Owned tasks wait for A4.** Per-entity behavior over several steps is a
  later step, after structured jobs exist.

## What Coax doesn't take

- **Garbage collection and a VM.** Ruled out by spec §21, the frame budget
  and GPU-shared memory. Handles and regions replace them.
- **Failure as the main control flow.** Epic's `<decides>`, `[]` calls and
  speculative `if` conditions are a large new concept. Coax's `T?`,
  `Result`, `try` and `or_else` already make failure explicit as a value.
  A2 and A5 give the two benefits that matter (stale lookups that don't
  crash, failed updates that don't half-apply) without the new form.
- **Single-threadedness as the race answer.** Verse OS runs inference and
  rendering on one GPU from one process. A4 prevents races by structure.
- **Trusting the native layer.** Agents write our C host. A1 inventories it,
  and [memory-safety.md](memory-safety.md) hardens what can be hardened.
- **Arbitrary-precision integers and logic variables.** Coax's sized,
  checked integers suit engine code. The Verse Calculus' unification and
  lenient evaluation are research.

## Roadmap: suggested issues

Each issue ends on a check that runs on our own machines. "R" numbers are the
[roadmap](ROADMAP.md)'s phases each issue advances. Issues S0–S9 come from
[memory-safety.md](memory-safety.md); V1–V12 come from this doc. The order
below is the suggested filing and work order.

### Stage 1: decide and inventory

| Issue | Title | Scope | Done when | R |
| --- | --- | --- | --- | --- |
| S0 | Adopt "no raw pointers in ordinary Coax" | Add the rule to spec §3, ROADMAP R3 and `INVARIANTS.md`: `*T` ships only inside `#unsafe` | Docs merged; the grammar refuses `*T` outside `#unsafe` when pointers land | R3 |
| V1 | Boundary modules for foreign declarations | `#boundary` marker; `#foreign` refused outside boundary modules; move the ~1,100 declarations, grouped by host | Every `#foreign` is in a boundary module; all existing builds and checks pass | R11 |
| V2 | `#unsafe` blocks with reasons | Grammar, checker and emitter support; reasons recorded in `coax deps --effects`; per-directory counts in `STATUS.md` | A foreign call outside `#unsafe` or a boundary module is an error; the inventory lists every block and reason | R3, R11 |

### Stage 2: enforce effects

| Issue | Title | Scope | Done when | R |
| --- | --- | --- | --- | --- |
| V3 | Declared effect sets | `#effects(...)` on procedures and procedure types; subset check against the inventory with the shortest-path witness in the error | A procedure that declares `computes` and reaches `io` fails with the path; mutation gate on the checker at 0 survivors | R11 |
| V4 | Required declarations on published surfaces | Require `#effects` on `#export_c`, public module procedures, `#authority` units, foreign declarations and callbacks | All of them in the repository declare effects; the checker refuses a missing one | R11 |
| V5 | Safe-unit report | `coax status --json` and `coax deps --effects` report which units are safe; CI prints the count | The report matches a hand audit of five units | R0, R11 |

### Stage 3: references and lifetimes

| Issue | Title | Scope | Done when | R |
| --- | --- | --- | --- | --- |
| V6 | `Handle(T)` and `Store(T)` | Nominal handles, slot stores with generations and retirement, `store_get` returning an optional view, `store_get_live` trapping | Library with `#authority` checks for wrap and reuse; 0 surviving mutants | R3, R4 |
| V7 | Move Aethyr entities to `Handle(Entity)` | Port `aethyr/entity.coax` and its callers | The entity oracle agrees between old and new over its sequences; the realm frame's 99th percentile is within 2% of before | R3 |
| S1 | Regions for views | Every view carries a region; refuse returning or storing a view in a longer region, or keeping it across reset, growth or frame return | The existing E0383 cases pass under the new rule; new cases for arena reset and growth refuse | R3 |
| S2 | Regions for records that hold views | A record takes the shortest region of its views; refuse its escape | `EntityStore`-style records compile; a test that stores one in static storage refuses | R3 |
| S3 | Host-declared regions for foreign buffers | Boundary declarations name the region of a returned view (`region(host)`, `region(frame)`) | Vulkan mapped memory and camera frames go through declared regions; misuse refuses | R3, R9 |

### Stage 4: undefined behavior and hardening

| Issue | Title | Scope | Done when | R |
| --- | --- | --- | --- | --- |
| S4 | No undefined behavior in emitted C | Build generated C with `-fno-strict-aliasing` and `-fwrapv`; run the test suite's emitted C under UBSan and ASan in CI | CI runs clean under both sanitizers | R0 |
| S5 | Hardened C build profile | A build profile for generated C and the C host using a memory-safe C toolchain (Fil-C-style) on the NixOS hosts | The signer and one device model build and pass their tests under it; the slowdown is measured and recorded | R8 |
| S6 | Bounded model checking for restricted units | Run CBMC or ESBMC over emitted C for units with no allocator, bounded loops and no foreign calls | `lib/proto/wire.coax` and one virtio device model check clean to a stated bound | R0 |

### Stage 5: jobs, rollback, compatibility, ownership

| Issue | Title | Scope | Done when | R |
| --- | --- | --- | --- | --- |
| V8 | Structured `#jobs` scopes | Scopes that own their jobs, per-job scratch regions, effect limits on job bodies, `defer` on failure | An Aethyr solver step runs on 1, 2, 4 and 8 workers with identical results | R15 |
| V9 | Proven-disjoint writes | Constant splits and `#forall`-checked split procedures; overlapping writes refused | A test with overlapping ranges refuses at compile time; the solver's split passes | R15 |
| V10 | `#transaction` blocks | Emitter-generated write log; rollback on `err`, `none` and `try`; effect limits; refuse `#unsafe` writes inside | A wallet test that fails its ledger append restores the balance; mutation gate at 0 survivors | R2, R11 |
| V11 | `coax compat` and `#persisted` | Surface comparison for exported procedures, layouts, effects and persisted records; release gate | A removed export and an inserted persisted field both refuse; release checks run it | R5, R16 |
| V12 | Entities own their resources | Despawn ends children, GPU resources, sounds and registered jobs in a defined order | A despawn test leaves no live resource; the order is checked | R3, R27 |

### Stage 6: change the claim

| Issue | Title | Scope | Done when | R |
| --- | --- | --- | --- | --- |
| S7 | Safe profile on by default | New units are safe unless they declare otherwise; `#unsafe` needs a reason | All of `coax/lib`, `aethyr/` and `verse/` build; unsafe counts published | R3 |
| S8 | Change spec §3 | Replace "Do not promise general memory safety" with the guarantee in [memory-safety.md](memory-safety.md) and its "doesn't cover" list; update `INVARIANTS.md` | Merged in the same change as S7 | — |
| S9 | Measure the cost | Realm frame time, Aethyr step time and compile time before and after Stages 3–5 | A record in `coax/perf/` with the numbers | R0, R22 |

### Dependencies

- V1 and V2 come before V3, since effects need the boundary to exist.
- V3 comes before V8 and V10, which restrict effects inside their blocks.
- V6 comes before V7 and V12.
- S1 comes before S2, S3, V8 and V10, which all rely on regions.
- S7 and S8 come last and land together.
