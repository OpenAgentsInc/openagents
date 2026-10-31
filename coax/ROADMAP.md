# Coax implementation roadmap

<!-- coax-release:current:start -->
Current compiler: **coax0 0.38.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->
Updated 2026-10-31. Baseline: the current compiler above, including source composition,
CPU layout reflection and the first R0 benchmark/source-debugging slice. This
revision incorporates the follow-up technical review while retaining the
evidence and compatibility limits of what has actually shipped. It adds the
physical-world foundations (R11–R19) and numbers every phase with a single
integer; [section 29](#29-earlier-phase-labels) maps the earlier labels.

The goal is a systems language in which OpenAgents and Verse can express
their computation, data, protocols, tools and build policy in Coax. Runtime
costs, ownership and generated behavior must remain visible. Native adapters
continue to own platform effects, storage, graphics resources and foreign SDKs.
Checks should produce inspectable evidence for humans and agents without
pretending that a successful build grants authority or proves arbitrary code.

This is the **implementation order**, not a new language specification or a
claim that planned syntax works. [The spec](spec.md) defines intended
semantics; [the bootstrap grammar](grammar.ebnf), implementation and tests
define what is accepted now. [The book](book/README.md) teaches both, marking
the difference. Milestones below have dependencies and exit evidence, not
calendar estimates. A milestone is complete only when its consumer and gate
are complete.

## 1. Starting point

The initial review described 0.10.0/0.11.0; the follow-up used a 0.12.0
snapshot. Preserve their criticism of missing evidence, but do not schedule
source composition, CPU reflection or the shipped 0.13.0 debugger/benchmark
slice as new work. The README's obsolete paynet prerequisite is already fixed.

| Area | Shipped | Still missing |
| --- | --- | --- |
| Language core | Scalars, strings, structs/defaults, enums, fixed arrays/views, loops, cleanup, checked casts and ordered evaluation | Tagged results/unions, exhaustive switching, richer procedures, pointers, owned containers and generics |
| Compile-time work | Constants, bounded `#run`, assertions, finite checks, execution coverage for authority units, mutation testing | Typed quotation/insertion, general declaration generation, plugins and a dependency scheduler |
| Source graph | Checked `#load`, physical-file ownership, bounded graphs, original-file diagnostics/traps, `coax deps` | Namespaces/visibility, parameterized modules, content manifests, packages and incremental invalidation |
| Reflection | Schema-1 CPU type/field/enum records; constant projections and `#run` tables; a real Haunted House layout consumer | First-class types, stable declaration identity, richer metadata, alternate-target layouts and derivation |
| Compiler | Coax lexer/parser/checker/source policy, descriptor importer, wire runtime and fingerprinting; explicit reproducible seeds | Coax evaluator, verifier/mutator, emitters and driver; complete debugger/type integration and a resident compiler |
| Native product | Actual iOS scene/HUD, Grid, movement, animation, wallet rules, Haunted House and voice logic | Measured whole-frame parity against a frozen native reference, broader platform/ABI evidence and complete product migration |
| Protocols | Selected proto3 descriptor imports, bounded borrowed codecs and real app consumers | Remaining consumer/domain admission paths and consumer-justified schema features |
| GPU/platforms | Scalar visual functions emitted as Metal; native stage wrappers; Mac/iOS integration; narrow Coaxial Wasm/WebGPU splash | Shared GPU resource declarations, general shader profiles, full engine browser vertical slice and evidence for other targets |
| Builds/iteration | Single-shot C11 output, shell/Xcode integration, JSON diagnostics, original-source line/name maps and phase timings | Capability-aware `build.coax`, safe editor analysis, cache, transactional reload and additional backends |
| Physical-world data | Plain scalar fields with units stated in comments and docs; the simulated robot of Psionic chapter 167 | Units and frames, typed clocks, bounded control, replay and snapshots, robot-model import, stamped sensors, supervised simulated actuators and fault scenarios |

The [source-unit evidence](../docs/verification/coax-source-units-2026-10-28.md)
and [reflection evidence](../docs/verification/coax-layout-reflection-2026-10-28.md)
record recent results and their limits. Native runtime traps already retain
source ownership; native debugger stepping and name maps now reach Coax files. The pulse
example establishes native behavior parity, not a measured full-engine speed
claim. No cross-platform floating-point, race-freedom or general memory-safety
guarantee follows from the current tests.

## 2. Order and dependency rules

R0 remains the adoption gate for substantial engine migrations and expansion.
Small conformance, evidence and tooling deliveries can proceed while helping
close that gate; a missed timing budget must not be relabeled as a pass.
R1–R8 build the language, compiler and tool infrastructure for the larger
vision. Prove native GPU and browser integration in R9–R10 before committing to
a new backend. R11–R19 make computation about the physical world checkable:
transitive effects, then units and frames, typed clocks, bounded control,
strict math and jobs, replay, robot-model import, stamped sensors with
supervised actuators, and fault scenarios. Complete compiler migration in R20
and resident compilation/reload in R21–R22 on their own evidence. R23 remains
conditional on measurements.

Every phase depends only on lower-numbered phases. The numbers give the default
priority, not an instruction to block an independent experiment behind
unrelated features. In particular, a browser feasibility test belongs in R1;
the first GPU/browser slice does not require parallel jobs, full macros or a
self-hosted compiler. Effects work starts with ownership in R3. Coax compiler
migration can proceed in small pieces before R20 once each piece has its
prerequisites. Restricted body replacement (R21) needs only R1's first
delivery and can run early as an experiment; resident compilation also does
not require completed self-hosting, so R20–R22 may overlap once their own
dependencies and trust boundaries are satisfied.

The physical-world phases share two continuous consumers: one running Verse
world, and one small simulated robot, the closed loop of Psionic chapters
[167](../psionic/book/ch167-imitation-before-robotics-scale.md)–169 (see the [plan](../psionic/book/PLAN.md)), built on its deterministic and
differentiable solvers (chapters 109 and 111). Each of R12–R19 must improve one
of them. These phases produce language, library and checking work for
simulation. They do not authorize, drive or qualify real hardware; a hardware
bridge, driver or real-time deployment profile needs its own authorization,
safety review and target qualification outside this roadmap.

Every milestone must name its smallest useful delivery, exact prerequisites,
real consumer and acceptance evidence. Unrelated work grouped under the same
number is not a prerequisite. The slices use the existing typed graph and
shared lowering/report formats introduced below, rather than separate compiler
frameworks. Where a phase has a first delivery and an expansion, the expansion
can follow in smaller deliveries without holding later phases that need only
the first.

| Phase | First delivery and prerequisites | Expansion and additional prerequisites |
| --- | --- | --- |
| R0 — Evidence and debugging | Real benchmark/debugger baseline, generated release/status and bounded differential probes shipped; extend scoped evidence | Close measured adoption, device/tail/full-loop and independent review gates |
| R1 — Compiler graph, targets and modules | Existing typed graph and R0 baseline; declaration identities, targets, spans, basic namespaces, content identities and minimal procedure lowering (delivered) | Generated-work scheduling, richer module configuration and conditional dependencies |
| R2 — Results and complete control flow | R1 first delivery and lowering; typed results/optionals, exhaustive switching, propagation and cleanup in a real adapter | Argument forms, overloads, variadics and other consumer-driven ergonomics |
| R3 — Ownership, memory and context | Current typed graph; diagnose simple local-view escapes (started) and define borrow/transfer rules | R1 targets and R2 errors; owning storage, allocators, containers and context |
| R4 — Generics and foundational libraries | R1 first delivery and the existing borrowed-value ABI; generic copyable values and non-owning algorithms | R2 errors and R3 ownership/invalidation before generic owning containers |
| R5 — Source-aware metadata and derivation | R1 identities/targets and shipped metadata; narrow derivation for concrete checked types | R4 type parameters for generic derivation and richer first-class type manipulation |
| R6 — Typed code and storage transformations | R1 scheduling and relevant R5 metadata; hygienic checked generation for one consumer | Explicit AoS/SoA transformations with measured conversion and runtime costs |
| R7 — Useful tools before privileged builds | Current syntax/check APIs for formatting/evidence; R1 for typed queries and revision-bound fixes | Effect and expansion queries as R6 and R11 expose them |
| R8 — Workspaces, plugins and build programming | R1 scheduling, R2 errors and R7 safe analysis; R5/R6 where privileged plugins need checked generation | `build.coax`, offline locks, verified local caching and compiler-service operation |
| R9 — Target-specific GPU interfaces | R1 target ABI, R2/R3 host lifetimes and R5 layouts for the selected pipeline | Further resource kinds and shader profiles per consumer |
| R10 — Browser vertical slice | R9 interfaces for the same pinned workload | R8 input manifests for reproducibility and broader target evidence |
| R11 — Transitive effects | R1 lowering, R2 callbacks/cleanup, R3 regions and R4 instantiations | Reviewed foreign summaries and hot-replacement effect checks |
| R12 — Units and coordinate frames | R4 type parameters and R5 annotations; built-in quantity, point, vector and transform operators | User operator overloads after R11 proves them pure, nonallocating and nonfallible |
| R13 — Typed clocks and multirate ticks | R12 durations, R2 typed staleness errors and R11 clock effects | Recorded clock boundaries consumed by R16 |
| R14 — Bounded control | R3 scratch/allocators, R4 generics and R11 effect summaries | Per-target stack evidence and bounded solver building blocks |
| R15 — Strict math and deterministic jobs | R3 regions, R4 libraries, R11 effects and R1 target rules | Checked parallel execution and replay at 1/2/4/8 workers |
| R16 — Deterministic replay and snapshots | R5 stable identities, R13 recorded clocks and R15 strict profiles | Rollback with committed effects and qualified replay profiles per target |
| R17 — Robot-model import | R2 errors, R5 derivation and R12 frames/units; a published URDF subset | Further formats qualified separately |
| R18 — Stamped sensors and actuator supervision | R12 frames, R13 freshness and R14 bounded control; simulated devices only | Calibration identities and richer supervision policies |
| R19 — Reproducible fault scenarios | R16 replay and R18 supervision | Larger scenario corpora on the same recorded format |
| R20 — Complete compiler migration and distribution | The facilities each individual pass needs; retain independent oracles and seeds | Coax semantic passes/emitters, documented trust boundary and supported distributions |
| R21 — Restricted procedure-body replacement | R1 interface/layout identities and a quiescent, restricted Mac host; unchanged-ABI procedure-body replacement | Measured edit-to-result value before R22 |
| R22 — Resident semantics and transactional migration | R1/R8 cache inputs, R3 ownership and R5 stable state identities; R15 quiescence when workers exist | Migration and rollback using R16's snapshot inventory |
| R23 — Additional backends | Earlier adoption/target evidence and a measured backend bottleneck | Faster debug or optimized output conforming to the same semantic profiles |

### How this relates to spec section 20

The spec's existing stage numbers remain useful topic identifiers. Stages 0/1
are the bootstrap foundation; unfinished stage 2 evidence is R0/R2/R3. Stage 3
expands through R1/R4/R5/R6. Stage 4 is R7/R8. Stage 5 is R3/R11/R15. Stages
6/7 are R5/R9/R10 and ongoing protocol adoption. Stage 8 is R21/R22, and stage
9 is R23. Stage 10, physical-world foundations, is R12–R14 and R16–R19. R20 is
an explicit compiler migration track across those topics.

The coarse stage numbers are not a global dependency chain: the protocol
importer already shipped before the complete metadata/build/effects systems.
Similarly, a browser proof should expose target mistakes before a large
parallel runtime or a custom backend makes them expensive to change.

## 3. R0 — Establish the adoption gate now

**Started in 0.13.0.** The real cast pose, robots/body capsules and player
collision have a repeatable C ABI harness. A pinned historical native pose
evaluator supplies a separate algorithmic reference over the actual app asset;
the Debug iOS capture exercises the real Grid and Metal command buffers.
Original-source breakpoints, stepping, values, stacks, diagnostics and sanitizer
traces are tested on Mac LLDB, with an actual Simulator app attachment too.
See [methodology](../verse/benchmarks/README.md), [debugging](DEBUGGING.md) and
[retained results](../docs/verification/coax-r0-2026-10-28.md).

**Pose median gap closed; R0 remains open.** The follow-up Coax kernel rewrite
passes the unchanged proposed 5% median gate in all three profiles: 1.034×
native at checked `-O0`, 0.807× in the app's checked `-O2` profile and 0.740×
in wrapping release. Output tolerance and zero observed kernel allocations
still pass. The reference, reader adapter, asset, workload, flags and threshold
are unchanged. See [the follow-up evidence](../docs/verification/coax-r0-pose-2026-10-28.md).

These remain Mac/Simulator observations, not phone performance or whole-engine
parity. Debug variance includes a retained large outlier; a passing median
is not a frame-tail guarantee. Finish physical-device/tail budgets, complete
asset/install/launch/GPU-preparation attribution and
independent reference/gate review before closing R0.

1. Retain the frozen live pose kernel in `HomeRenderer.swift`/`verse/cast.coax`,
   corpus, tolerance, reference revision and algorithms. Independently review
   its reader adaptation and assumptions before adoption sign-off. Add a new
   reference or representative kernel only with a documented reason and its own
   baseline; do not replace the existing comparison to hide a regression.
   The archived paynet pulse is a later candidate until its real source/data/host path is available. A deleted
   `src/paynet` directory must not be a fictional prerequisite.
2. Run the Coax kernel through the actual C ABI and Metal frame path. Measure
   debug/release CPU time, frame tails, copies, allocations and GPU upload cost.
   Keep equivalence tests separate from benchmarks. Use identical workloads,
   flags and named hardware; retain raw results and dispersion over repeated
   runs. Agree the performance budget before optimizing. An initial proposal
   is at most 5% median update regression against the frozen reference, with
   a separately agreed frame-tail budget; this is a proposed gate, not a result.
3. Attribute the complete clean/warm development loop: load/parse/check,
   compile-time work, generation, C compilation, link, assets, launch and GPU
   preparation. Record generated C size and peak compiler memory. Optimize the
   actual bottleneck before assuming the C backend needs replacement.
4. Retain the shipped original-source maps and regression evidence for native
   compiler errors, sanitizer reports and LLDB stepping/values/stacks across
   loaded units. Expand generated-code/type inspection and target coverage as
   consumers require it; line directives alone do not complete those workflows.
5. **Shipped in 0.14.0:** [`release.coax`](release.coax) generates the CLI
   version/status, accepted-feature tables and current book status. Bootstrap,
   iOS generation and book publication check source/snapshot/binary drift.
   Implementation version, inventory schema and historical introduction versions
   remain separate. See [workflow](RELEASE.md) and
   [evidence](../docs/verification/coax-release-status-2026-10-28.md). Status is
   reviewed metadata, not independent semantic proof; unmanaged historical prose
   still needs review.
6. Preserve independent checks and frozen compatibility inputs. Define who
   reviews changes to invariants, oracles and verification policy, with the
   enforcement outside the compiler on owned infrastructure. An author who can
   weaken both code and checks cannot certify their own authority by compiling.
   Review the relevant dependency graph too: moving a decision into an unmarked
   helper must not evade review. This strengthens the external gate without
   changing physical-file `#authority` semantics or making them transitive.

**Exit:** reproducible kernel and whole-loop reports; zero unexpected output or
allocation regressions; usable original-source native debugging; a versioned
baseline and an independently enforced review/gate policy. If Coax worsens
reliability or maintenance, stop feature expansion and fix that first.

### Early semantic conformance delivery

Version 0.25.0 separates arithmetic, native optimization, the `BUILD_DEBUG`
selector and native debug information through a checked Coax profile policy.
Explicit axis flags override the legacy preset independently of argument order.
The no-flag and `--release` defaults retain their previous behavior; compile-time
and check arithmetic remain checked, and authority checks remain mandatory.
An optional schema-1 profile report records the selection without changing
ordinary emitted output or the existing timing/dependency/source-map schemas.
This makes optimized checked native builds explicit instead of requiring the
app's external C flags. It does not qualify new GPU devices, supply a semantic
cache or enforce arbitrary independently linked C objects.

Prefer checked ordinary integer arithmetic with explicit wrapping operations
as the eventual design, subject to compatibility review and a documented
migration. Preserve existing programs' named legacy behavior during that
migration. Choosing optimization must not silently select wrapping arithmetic
for an authority component. Record all profile dimensions in module/artifact
metadata, compatibility decisions, cache keys and evidence; define mixed-profile
call boundaries explicitly. Verification selection must never bypass existing
authority requirements.

Add same-profile unoptimized/optimized conformance for overflow, casts, shifts,
division edges, evaluation order and cleanup. Specify typed `f32` decimal
rounding, including whether it rounds directly to binary32 or through binary64.
Keep `1.0000000596046448`, exact midpoints and adjacent decimal values as
independent-oracle fixture candidates. The current binary64-plus-f32-hint path
is documented; this is an unresolved conformance concern, not a confirmed
compiler bug or permission to change literal semantics silently.

Generate an **optional separate native check executable** for eligible checks.
Run pinned inputs/assertions through the evaluator and emitted unoptimized and
optimized procedures under matching semantic policies. Compare values, specified
traps and caller-buffer writes; record intentional compile-time/runtime profile
differences instead of misclassifying them. The current evaluator checks remain
required and test code stays out of shipping programs. Extend to Wasm only as
that target's execution contract is proved.

**First bounded differential slice shipped:** optional
[`tools/differential.py`](tools/differential.py) generates supported checked
integer/state programs, compares a separate model with evaluator and native
`-O0`/`-O2` results, and reduces mismatches. The retained sample covers arithmetic,
branches, aggregates, caller-owned view writes and reverse cleanup. A deliberate
emitter defect is detected and reduced. See [evidence and limits](verification-differential.md).
Overflow/trap edges, floating point, `#run` materialization, arbitrary native
check lowering and other targets remain outside this slice.

Extend valid-program differential generation and reduction across those
remaining expressions and profiles.
Generate only defined, supported behavior so mismatches are meaningful; malformed
input fuzzing remains a separate activity. Deliberately seeded evaluator/emitter
defects must be caught, and minimized mismatches retained as regressions. Keep
independent numeric, layout and protocol oracles: agreement is not a proof.

### Early evidence and authoring tools

Start one versioned evidence record using existing commands before waiting for
plugins or a resident compiler. Bind each claim to source/dependency content,
compiler/toolchain, target, semantic profile, check/domain selection, execution
mode, budgets, mutation policy and artifact identities. Keep distinct outcomes:
**passed**, **counterexample**, **inconclusive**, **infrastructure error** and
**not checked**. Missing execution or exhausted analysis is not a pass.

Mutation records distinguish assertion-detected changes, specified traps,
rejected/ill-typed candidates, resource exhaustion, infrastructure failures,
survivors and any historical reviewed-equivalence annotations. A resource-limit
failure is a semantic kill only when a resource/termination bound is itself the
stated property. Equivalence is not an assertion kill or permission to add
exemptions; retain the repository's zero-survivor authority gates. Classifications
must expose uncertainty rather than weaken the existing gate to improve a score.

Add a small frozen authoring-task corpus: change a kernel, add a field, repair a
type error and update a protocol adapter; add invalid-lifetime tasks when that
diagnostic exists. Pin starting revisions, tools/models and budgets. Measure
independently accepted completion, repair iterations, unintended edits, review
effort and elapsed edit-to-result time. Agents must not edit away the task's
oracle. This evaluates the authoring loop alongside compiler/runtime cost.
R7 can consume these records and existing diagnostics immediately.

## 4. R1 — Build the compiler substrate once

**First delivery:** declaration identities, target descriptions, source spans,
basic namespaces/content identities and the procedure lowering contract below.
**Expansion later:** general generated-work scheduling and richer module configuration.
Do not make basic symbol tools wait for the entire scheduler.

**First delivery shipped (0.26.0–0.32.0).** Logical `#module` owners and nominal
declaration IDs, separate content revisions, qualified `alias.Name` lookup
through one checked graph, `#export` visibility with the `verse.hud` module
migrated, the native-lp64 target record with refusal of other targets, exact
authored spans, and ordered native C lowering from the checked graph's effects
with its `coax deps --lowering` inspection record. Each is pinned by
`tests/modules.py` or `tests/lowering.py` against C or fixture oracles. Other
targets, reexports, packages and the expansion's scheduling remain separate.

Replace repeated whole-program assumptions with explicit declaration work:
names/signatures, constant/type dependencies, layouts, bodies, generated work
and completion. Diagnose cycles and stalled work finitely. Keep the current
single-threaded implementation until profiling justifies parallel compilation.

Introduce scoped declaration handles, logical module identities and original
source spans. A logical ID must not depend on an absolute checkout path or a
line number. Define which edits preserve internal identity; persistent engine
and protocol identities are separate contracts completed in R5.

Add explicit target descriptions: scalar widths, pointer/count ABI, alignment,
calling conventions, platform/runtime capabilities and compiler options. Extend
CPU layout queries through versioned interfaces; do not silently reinterpret
schema-1 LP64 metadata as wasm32 or GPU layout. Test target layout against a
native compiler oracle for every target claimed.

Build namespaces/visibility, `module.coax`, explicit exports and module
parameters on this graph. Preserve physical-unit authority. Implement `#if`
only after conditional dependencies are represented: both branches parse,
only the selected branch resolves/checks/emits, and self-dependent conditions
fail. No automatic package downloads. Extend `deps` with content identities
without mislabeling today's path inventory as a lockfile or cache key.

Define an inspectable semantic lowering shared by the evaluator, analyses and
emitters: ordered operations, control-flow edges, places/storage writes,
cleanup, recoverable exits, fatal traps and source provenance. Start with the
minimum necessary structure, not a new frontend or a mandatory optimizing SSA
framework. Retain the existing checked graph as the source of truth.

Pin left-to-right operands, destination-before-value assignment and exactly-once
compound-assignment destinations. Include two potentially trapping operands
with no explicit calls, returned-value computation before deferred mutation,
nested cleanup/early exits and, once available, `try` propagation. The lowered
form must explain native behavior under each supported optimization profile.
These are conformance requirements, not assertions of existing miscompilation.

**Consumer/gate:** split one real engine subsystem and the compiler graph into
modules without weakening authority or changing behavior. Test cross-module
cycles, private names, configuration invalidation and original diagnostics.
Extend the existing Coaxial splash's wasm32 feasibility path rather than
rebuilding its host: add target type/layout evidence, a host import, memory
access and a useful source location. The current `coax/web/wasm.sh` plus
WebGPU host is a narrow consumer, not complete browser support.

## 5. R2 — Make fallible application code expressible

**First delivery:** tagged results/optionals, exhaustive switching, propagation and
cleanup in one real adapter. **Expansion later:** richer procedures and adjacent forms
only as consumers require them.

Implement tagged unions, optionals, exhaustive `switch`, typed results,
ignored-error diagnostics, `try` and `or_else`. Propagation must execute active
`defer` cleanup exactly once in reverse order. Keep fatal traps distinct from
recoverable domain errors; foreign error domains/codes must survive adapters.
Define that distinction normatively with the implementation: recoverable errors
execute normal cleanup, whereas fatal traps terminate according to the runtime
contract and promise neither normal `defer` cleanup nor safe continuation.
Expected malformed input should produce a typed error before an invariant trap.
Do not resume execution after invalid memory access by translating traps into
recoverable exceptions.

Add multiple returns, named/default arguments, typed variadics, procedure
values/callbacks and well-defined overload resolution in small steps. Preserve
immutable value parameters and evaluation order; an optimization must not
introduce mutation through a copied parameter. Callback registration/release and context lifetime
must be explicit, and owning callback use waits for R3's lifetime contract.

Complete adjacent basic forms needed by consumers: explicit flags enums,
distinct types, composition/`using`, separately declared `#as` conversions,
documentation attachment and raw/Unicode string handling. Specify UTF-8 versus
byte operations, invalid input and foreign zero termination rather than
changing today's byte-string semantics implicitly. Reverse/removing iteration
needs invalidation rules before it is accepted.

**Consumer/gate:** migrate one existing native resource/file or protocol-adapter
error path to typed results. Test all alternatives, missing/ignored errors,
deferred release on early exits and callbacks ending at the host boundary.
Unsafe raw unions are not a prerequisite; admit them later only with explicit
discriminator/layout rules and a concrete foreign-interface need.

### Shared R2/R3 asynchronous host contract

Define created, submitted, cancellation-requested, completed and retired states
before adding language-level async syntax. Cancellation requests do not imply
callback suppression or resource retirement: the adapter must establish when
completion can no longer access input storage or callback context. Specify
ownership while pending, owner disappearance, completion thread/queue, exactly
once committed effects, release responsibility and generation-checked reuse.

Use pure sequence checks for cancellation then late completion, duplicate
completion, pending-owner destruction and handle reuse. A real native adapter
must separately show no stale access, double release or duplicate committed
effect under those orderings. No `async/await` feature is a prerequisite.
Protocol validation binds particular bytes and relevant state revisions;
mutating backing bytes or changing domain state can require revalidation.

## 6. R3 — Make ownership and allocation policies explicit

**First delivery started:** E0383 now rejects straightforward local-array view/string
returns, simple aliases/slices/casts and conditional arms while preserving
borrowed parameter returns. [Evidence and limits](verification-local-borrows.md)
record 553 checker checks and 3,762 killed mutants with zero survivors.
Interprocedural, aggregate-contained and general region lifetimes remain
unchecked; the complete borrow/transfer discipline is still work to do.
**Expansion later:** owning allocations, containers and context after error/cleanup
and target contracts exist. Early escape diagnostics do not establish general
memory safety.

Specify non-null/nullable pointers, address/dereference operations, initialization,
borrows and invalidation before adding an allocator-shaped convenience API.
Preserve existing value copies for copyable structs/arrays and shallow borrowed
view descriptors. Reconcile ownership before containers by defining four cases:

| Category | Assignment/pass/return contract |
| --- | --- |
| Copyable value | Copy the value without duplicating an ownership obligation |
| Borrowed view/reference | Copy the borrow; never extend its owner's lifetime |
| Owning value | Transfer ownership under explicit move rules; duplication requires an explicit clone |
| Unchecked pointer/foreign reference | State the unchecked boundary; no inferred ownership guarantee |

A struct containing an owner must not become freely copyable. Specify transfer
into fields, parameters and returns, use after transfer, and double destruction.
Resolve `defer destroy(...)` before permitting an owner to move: reject the
transfer, transfer/disarm the cleanup obligation explicitly, or use another
defined rule. Never leave two apparent owners. Explicit destruction and
`defer` can remain; automatic destructors are not required.

Distinguish read-only access from immutable backing storage: another writable
alias can invalidate decoded-view assumptions. Define writable/read-only borrows
and retained-return rules. Define a conservative region discipline
for admitted APIs, including subranges, overlapping views and escaping scratch.
Unchecked pointer/foreign operations stay explicit and do not acquire a memory
safety guarantee from an annotation.

Build allocators, arenas, pools, generation-checked handles and dynamic arrays.
Growth/reset/recycle must invalidate the right references. Add allocation
provenance, poisoning and checked debug paths. `destroy` releases only the
ownership its contract declares; no implicit recursive destructor or GC.

Add implicit context only after its dependencies are modeled. Allocator,
scratch and logging effects remain visible transitively; `push_context` changes
a bounded policy scope. `#no_context` and C entry points retain explicit
conventions. Keep runtime context layouts private/versioned, not an ABI shared
accidentally with Swift or independently built modules.

**Consumer/gate:** move one native-owned scratch/storage part of the animation,
world or codec workflow into an owning Coax abstraction. Test allocator failure,
growth, stale handles, reset, aliasing, background/foreground and cancellation
under sanitizers. No hidden per-frame allocation or escaped temporary survives.
Include transfers with pending cleanup, attempted double destruction, borrowed
returns, stale views after growth/reset and writes through other aliases.
For each violation state whether it is rejected statically, detected dynamically
or remains unchecked; do not claim the last category is covered by annotations.
Start effect summaries here; R11 completes them, and parallel execution waits
for R15.

## 7. R4 — Generics, constraints and foundational libraries

**First delivery:** generic copyable values and non-owning operations over fixed
arrays/views, using R1 identities/targets and the current borrowed ABI. They
need neither dynamic allocation nor implicit context. Preserve the stated
lifetime limits; this slice does not infer ownership safety. **Expansion later:**
generic owning containers require R2 errors and R3 transfer/invalidation rules.

Implement `$T`/`$N` parameters, generic procedures/types and constraint checking
with source-level instantiation traces. Specialization keys include resolved
types, values, targets and relevant options/effects. Bound recursive instances,
cache repeated work and report instance counts, time and code size.

Add explicit argument baking through the same mechanism. Do not specialize
every constant call silently. Operator overloads require pure, nonallocating,
nonfallible behavior; no allocation or I/O behind math syntax. Do not overload
assignment, field lookup, short-circuiting or calls.
Admit overloads only when R11's transitive effect checks enforce these
restrictions; until then keep the operators built in.

Grow the standard library with actual consumers: strings/UTF-8 and byte views,
builders, arrays, arenas, pools, hash tables with coherent equality/hash rules,
ordered tables, sorting and math. Separate iteration policy from persistence
identity. Keep platform file/process/network effects behind explicit adapters.

**Consumer/gate:** replace a real duplicated bounded container or algorithm in
the compiler/engine. Compare outputs, memory and iteration cost; test rejected
constraints, specialization explosions and ownership/error paths. Only add a
result-type customization hook after ordinary constraints prove insufficient;
the hook must be pure, typed and bounded, not a mutable compiler escape hatch.

## 8. R5 — Source-aware reflection and useful derivation

**First delivery:** narrow derivation over concrete, already-checked types using
R1 identities/targets and the shipped layout queries. It does not require
generic owning containers, general macros or full first-class type manipulation.
**Expansion later:** generic derivation needs R4's first delivery; richer reflection grows by consumer.

Build on the shipped layout queries. Later add first-class `Type`, borrowed `Any`,
declaration/field annotations, defaults, procedure signatures and later effect
metadata through versioned records. Keep compile-time host values distinct
from target values. Expose source identity and provenance to tools without
exposing mutable compiler pointers.

Define four identities separately: compiler declarations, engine component
registrations, target layout fingerprints and public schema names/field numbers.
Persistent IDs require explicit allocation and migration rules; do not hash a
field's current index or transient compiler type number into a save format.

Implement a narrow checked `#derive` path for equality/hash/printing and one
registry/inspector. Every generated declaration re-enters normal checking.
Record generator identity/version, declaration inputs and emitted ranges.
Missing semantics for pointers/units/ownership must be annotations or explicit
adapters, never guessed from field type alone.

**Consumer/gate:** replace a hand-maintained Verse registration or inspector
table, rather than adding an unused language demo. Test collisions, reorder,
rename, migration and cross-build identity. Inspect expanded code; reject bad
generators and check independent native layouts. `#derive(proto)` can consume
approved descriptors or propose a reviewed schema change; root `proto/` remains
the wire source of truth, not a competing generated Coax schema language.

## 9. R6 — Hygienic typed code and explicit data transformations

Implement phase/category-typed `Code`, quotation, `#insert` and `#expand` on the
declaration scheduler. Generated expressions, statements, declarations and types
must match their insertion sites. Use hygienic local bindings, explicit caller
captures/permissions and explicit evaluation duplication. Preserve invocation,
generator and generated-node provenance through diagnostics and debugger maps.

Make expansion and specialization budgets deterministic work/node limits, with
finite cycle/convergence failures. Do not substitute a wall-time cutoff or
silently raise limits to hide non-convergence. Keep ordinary `#run` sealed from
host capabilities.

Implement AoS, SoA and blocked-SoA transformations as inspectable generated
storage/accessors. Copy/gather/scatter semantics stay explicit; a gathered value
is not a pointer to an interleaved object. Include generated metadata in layout
checks and count conversion costs.

**Consumer/gate:** one real component/update or inspector generator with name
collision, capture, double-evaluation, wrong-category and budget fixtures.
Measure the transformed engine loop and generated C growth. Keep the simpler
representation unless the workload supports the transformation.

## 10. R7 — Useful tools before privileged builds

Ship a formatter and the R0 evidence format from current syntax/check APIs.
Add restricted typed symbol/layout queries and structured fixes as R1 exposes
their inputs; effect and expansion queries wait for those features, not the
other way around. Extend existing JSON diagnostics, `layout`, `deps` and source
maps instead of making another semantic frontend.

Fixes carry exact source ranges and expected document/content identities;
reject stale edits and evidence rather than guessing at the current buffer.
Offer bounded, credential-free editor/agent analysis that does not implicitly
run privileged build plugins. Distinguish analysis performed from checks not
selected; missing generated inputs remain explicit, never invented symbols.
This does not wait for macros, generic owning containers or `build.coax`.

**Gate:** a real authoring workflow; formatter idempotence and semantic
preservation, stale-fix/evidence rejection, useful structured diagnostics and
restricted queries without host side effects. Use the frozen task corpus to
measure accepted completion and repair/review effort, not a selected demo alone.

## 11. R8 — Workspaces, plugins and build programming

Define isolated workspaces, staged typed events and an input manifest on R1's
scheduling and R2's typed results. Add plugins through scoped handles and checked proposals, taking on
R5/R6 dependencies only where their generation actually requires them.
Generated work must converge before `complete`; a plugin cannot mark unchecked code valid.
Its identity/version/options/inputs participate in the dependency graph.

Make file/process/environment/network capabilities explicit and deny them in
pure computation. Record content hashes, sorted directory enumeration,
allowlisted environment, target/toolchain versions and pinned fetched artifacts.
Keep clock/randomness absent by default. A permission label alone is not an OS
sandbox; privileged builds and untrusted editor analysis need separate execution
policies, with enforcement demonstrated.

Implement `build.coax` against that API, initially generating current Xcode
source manifests and any restored CMake manifests. Compare generated and
existing builds before replacing shell or project lists. Add offline package
locks/tool provisioning and verified local caching; remote caching and signing
need their own trust/authenticity gates. Imports never implicitly fetch code.

Extend R7 into compiler-service/LSP operation, richer effect/expansion queries
and workspace-aware evidence as their semantics become available. Keep its safe
analysis path independent of privileged execution.

**Consumer/gate:** generate and build one real asset/registry/app workflow.
Test deterministic outputs, denied effects, changed-input invalidation,
plugin failure, stale cache rejection and editor operation without credentials
or host side effects. Pause if conventional generation is simpler and delivers
the same useful workflow.

## 12. R9 — Target-specific GPU interfaces

Separate CPU storage, C ABI, GPU resources and protocol encoding in reports
and fingerprints. Introduce the vector/matrix/SIMD lane types and resource
access/capability declarations required by a concrete pipeline. Compare shared
schema generation feeding existing shader tools with a restricted Coax shader
backend before expanding the latter.

The shader profile must exclude allocator/context use, general pointers and
recursion; validate bindings, offsets, strides, lengths, stages and capabilities
for each target. Native Metal can remain behind platform shims. Preserve queue,
fence and buffer ownership; replacing a pipeline cannot reinterpret live data.

**Gate:** a real render or compute pipeline, including malformed upload refusal,
native/GPU readback layout vectors and resource retirement. The scalar visual
emitter and current CPU reflection are foundations, not completion of this gate.

## 13. R10 — Browser vertical slice

Complete wasm32 startup/runtime and C/host ABI with explicit imports. Validate
pointer/count width, bounds, memory growth and stale JavaScript typed-array
views; asynchronous resources/cancellation; input/main-loop ownership; error
reporting and original-source debugging. Render the selected simulation or
geometry through a separate WebGPU/WGSL bridge. CPU Wasm is not shader code.

**Gate:** the same pinned workload reaches both the native Metal app and an
actual browser, with output comparisons where semantics require them and
separate GPU tolerance rules. Include startup/download/instantiation, assets,
memory growth and cleanup, not merely a successful Wasm link. Publish a target
matrix; a target is supported only to the level its ABI/runtime/debugging and
real-program evidence establish. Linux/Windows/Android require their own rows.

Expand protocol usage alongside these consumers. Retain bounded decoding,
presence/unknown-field policy, domain units/revisions and real admission checks.
Add maps or other descriptor features only with a real consumer and independent
wire tests. Preserve published/vendor schema identity and original signed bytes;
decoding, reflection and generated APIs never grant host/payment authority.

## 14. R11 — Enforce transitive effects

Complete transitive effect summaries for allocation, blocking, host I/O,
clocks, random sources, foreign calls, atomics and region reads/writes. A
summary covers every reachable implementation: generic instantiations,
procedure values and callbacks, cleanup and error paths, container growth,
foreign boundaries, and formatting or logging hidden behind a helper. Check
actual region and range overlap, including aliases carried by views/pointers;
field names do not establish disjointness. Unverifiable foreign effects need
explicit reviewed summaries; an unknown effect remains a visible, unfulfilled
obligation rather than an assumed absence.

Expose summaries through the R7 queries and source maps, so a developer can
ask why a call graph allocates or blocks and see the path. Summaries are facts
the optimizer may use only after they are checked; undocumented assumptions
about aliasing or effects are never an optimization input.

**Consumer/gate:** compute summaries for one real engine subsystem and one
generic library routine. Fixtures include an allocation reached only through
a generic instantiation, a callback, a cleanup path and a foreign call; each
must surface in the summary with its path. Compare declared versus observed
allocation and blocking on native runs. Mutating an effect source must change
the summary. Hot replacement re-checks summaries in R21–R22.

## 15. R12 — Units and coordinate frames

Add dimensioned quantities, and points, vectors and rigid transforms tagged
with a frame and a unit, as ordinary checked types over R4 type parameters:
for example a point in a world frame measured in metres. Points and vectors
are distinct: a point minus a point is a vector, a point plus a vector is a
point, and adding two points is refused. Mixing frames or units is refused
with a diagnostic that names the expected and received types and the
conversions or transforms required. Conversions are explicit, exact where the
scale is exact, and rounded under a stated rule where it is not.

A transform carries its source and target frames; composing transforms checks
that the frames meet. Rotation representations (quaternions, matrices and
axis-angle) state their conventions, including handedness and normalization,
and their tolerance rules. Frame declarations distinguish body and optical
conventions and continuous versus globally corrected frames that may jump, and
these distinctions survive import and arithmetic. A frame-checked value has
the same layout as its plain scalar fields; R5 layout reflection pins that
the tags cost nothing at runtime.

Begin with built-in operators for these types. User-defined operator overloads
wait until R11 effect checks can enforce that they are pure, nonallocating and
nonfallible. Units of time are quantities here; clocks and timestamps are R13.

**Consumer/gate:** move one Verse camera/world transform path and the joint
and pose values of the simulated robot (Psionic chapters 167–169) onto these
types. Fixtures cover point/vector algebra, refused frame and unit mixing,
composition through a frame chain, exact and rounded conversions, a jumping
frame, and diagnostics that name the missing conversion. Generated code and
layouts must match the untagged version byte for byte where no conversion is
written; measure any difference.

## 16. R13 — Typed clocks and multirate ticks

Distinguish simulation time, which can pause, scale or jump; steady time,
which is monotonic and used for timing against hardware or the host; and civil
time, which follows a calendar and may be corrected. Each is its own type of
instant, built on R12 durations. Converting between clocks is explicit and
names its source, such as a recorded mapping or a pinned offset. Reading any
clock is an R11 effect, absent from pure computation.

Add tick schedules for multirate systems: each task declares its rate, and
each connection between tasks declares what a faster reader sees between
slower updates (for example sample-and-hold of the latest value) and a maximum
age. A read older than its maximum age is a typed R2 error, not a silently
stale value. The schedule is deterministic: given rates and phases, the order
of tasks within a tick is defined and inspectable.

**Consumer/gate:** drive the simulated robot's physics and control steps at
different rates, and Verse's fixed-step update, from declared schedules.
Fixtures cover refused conversions between clock domains, simulated time
paused and stepped while steady time advances, sample-and-hold reads, a read
past its maximum age, and the exact task order over many ticks compared
against an independent reference trace.

## 17. R14 — Bounded control

Add a `#bounded` contract for procedures on latency-sensitive paths. It
declares a maximum stack and a named scratch region of fixed size, and forbids
allocation, blocking, recursion, unbounded loops and calls with unknown
effects anywhere in the procedure's call graph, including generic
instantiations, callbacks, cleanup and error paths. The checker derives these
facts from R11 summaries and R3 storage rules; a violation names the path to
the offending call. Loops need a bound the checker can see. Scratch comes
from a caller-owned R3 region; nothing escapes the call.

Report the computed stack bound per target from the lowered form, and compare
it with native measurement. A bound that cannot be computed is a refusal, not
an estimate. Bounded solver building blocks (iteration budgets, residuals and
convergence status as results) are library work on top of this contract.

**Consumer/gate:** make the simulated robot's control step and one Verse
per-frame kernel `#bounded`. Fixtures add allocation, blocking, recursion, an
unbounded loop and an unknown foreign call one at a time, directly and through
a generic helper or callback; each must be refused with its path. The computed
stack bound must cover the measured native stack on each claimed target.

## 18. R15 — Strict math and deterministic jobs

Specify atomic ordering and the memory model before exposing concurrent APIs.
Build the job scheduler, disjoint partitions, `for_parallel`, worker-local
context/scratch, cancellation and defined reductions on R11 effect and region
summaries. Reject conflicting jobs before claiming a safe schedule. Keep a
serial implementation as the oracle. Validate lifetime and release across jobs.

Implement strict/deterministic profiles with pinned math implementations,
rounding/contraction/reassociation rules, explicit RNG state and ordered
iteration. Fixed-point types need exact scale, rounding and overflow vectors;
they are not a drop-in proof of a different floating algorithm. Define SIMD
behavior under those profiles before enabling faster lowerings.

**Consumer/gate:** the selected ECS/simulation kernel replays with 1/2/4/8
workers under stress, with forbidden effects/aliases rejected. Compare target
results on the actual architectures claimed, including serialized state and
reduction order. Host-libm equivalence on one Mac is not cross-platform
determinism. Measure scheduler and validation overhead against the serial loop.

## 19. R16 — Deterministic replay and snapshots

Record every observation boundary of a run: input acceptance order, asset
publication, R13 clock readings, random-stream state, executable identity and
the R15 numeric/runtime profile. A snapshot saves logical state, entity
identities and generations, pending structural commands, accepted future
inputs and solver integration or warmstart state through versioned schemas
with R5 stable identities. Each snapshot carries an inspectable inventory:
what it includes, and which external resources (graphics devices, network
connections, device interfaces) must be reconstructed rather than restored.

Replay compares state exactly where the profile promises exact results, and
reports the first divergence by tick, field, system and source operation.
Numerically tolerant simulation uses a separate trajectory contract; a
per-kernel tolerance is not an end-to-end trajectory guarantee. A qualified
replay profile names its exact scope (build, worker count, target and
architecture) and refuses operations outside it.

**Consumer/gate:** record and replay the simulated robot's closed loop,
including the Psionic deterministic solver's state, and one Verse session.
Restore from a mid-run snapshot and match the uninterrupted run. Fixtures omit
each inventory item in turn and must show the divergence it causes; a replay
under a different profile must be refused, not silently compared.

## 20. R17 — Robot-model import

Import a published URDF subset into checked Coax data: links, joints, limits,
inertias, geometry references and frames, using R12 frame and unit types and
R5 derivation for the generated declarations. Validate frames and their tree,
positive and physically consistent inertias, joint limits and axes, and
provenance of the source file and meshes. Unsupported elements are refused by
name, never dropped. Other formats are qualified separately.

**Consumer/gate:** import one pinned arm description used by the simulation
and compare every value with an independent parser. Malformed fixtures cover a
cycle in the joint tree, a missing frame, a non-positive or inconsistent
inertia, an inverted limit and an unknown element. Import is reproducible from
the same input bytes.

## 21. R18 — Stamped sensors and actuator supervision

Give every sample a stamp: its R13 clock and instant, R12 frame, sequence
number and calibration identity, with uncertainty where the source reports it.
Freshness checks use maximum ages; extrapolation beyond a sample is explicit
and bounded. A transform applied to a measurement must match its timestamp and
calibration, not only its frame.

Add an actuator supervisor that validates each proposed command against
freshness of the state it was computed from, joint and rate limits from the
R17 model, controller identity, an authority epoch, sequence and expiry.
Commands from a revoked epoch or past their expiry are refused. The supervisor
and the control step it guards are R14 `#bounded`.

All devices in this phase are simulated. Driving real actuators is a
separately authorized integration with its own safety review, hardware
qualification and real-time deployment profile; nothing here grants it.

**Consumer/gate:** run the simulated robot's sensors and actuators through
stamps and the supervisor. Fixtures include a stale sample, a frame or
calibration mismatch, a command beyond a limit, a command from a revoked
epoch, a reordered or duplicated sequence and an expired command; each is
refused with its reason, and the refusals are R2 results, not traps.

## 22. R19 — Reproducible fault scenarios

Inject faults into simulated runs: sensor dropout, delayed or reordered
samples, timing overruns, actuator saturation, disconnects and invalid values.
A scenario is data, recorded with the R16 run format, so the same scenario,
seed and build reproduce the same run and the same supervisor decisions.

**Consumer/gate:** a scenario corpus for the simulated robot in which each
fault produces the expected refusal, hold or safe stop from the R18
supervisor, and the outcome replays exactly. Removing a supervisor check must
make at least one scenario fail.

## 23. R20 — Finish self-hosting and the supported toolchain

Move the remaining semantic work into Coax in independently reviewable slices:
verification/coverage and mutation policy; bounded evaluation; C/header/layout
and shader emission; then driver/workspace/tooling policy. Use the shared typed
graph instead of introducing another incompatible AST or wire runtime. Native
code remains the effects/storage/ABI layer for toolchains and platform SDKs.

Each migration gets checks and mutation tests, rejected-input/budget cases,
native sanitizers and independent semantic oracles. Matching the previous
output characterizes compatibility; execution against C/protoc/target oracles
establishes different correctness evidence. Keep provenance for synthetic code
so source mutations are not confused with corruption of compiler artifacts.

Retain explicit trusted seeds, deterministic regeneration and multiple-stage
fixed points. Document seeds, native shims, tools and compiler passes still
trusted. Where feasible, compare independent bootstrap chains; a fixed point
alone is not a bootstrap-security proof.

Complete the consumer-driven standard library and per-target distributions:
versioned host interfaces, reproducible toolchains, platform conformance,
debugging, language/tool documentation and recorded release evidence. Language
core stability, target support and library maturity need separate status.

**Exit:** a clean machine can bootstrap and build the real native/browser
consumers with the documented tools, without Python/protoc as normal compiler
dependencies. Library, diagnostics, ABI, mutation and conformance suites pass;
the remaining trust boundary is explicit. This track can start earlier whenever
R2–R8 provide enough facilities for the next compiler pass.

## 24. R21 — Restricted procedure-body replacement

Before semantic caching or general migration, use a deliberately restricted
Mac host to replace bodies behind unchanged exports, calling conventions,
semantic profiles and layout fingerprints. R1 interface identities and a
defined quiescent host lifecycle are enough; plugins, full self-hosting and
owning generic containers are not prerequisites. Keep persistent storage and
platform resources in the resident host.

Reject changed layouts/exports, escaped callbacks/code pointers, outstanding
old frames and unsupported worker activity. Admit replacement only at an
established safe point; rejecting an unsupported case is part of the feature.
Retain the old callable table until candidate validation/publication succeeds.
Measure edit-to-visible-result for an actual controller/world change. This
experiment must show value before paying for general state migration.

## 25. R22 — Resident semantics and transactional migration

Use content/declaration dependencies to build a bounded resident cache. Test
invalidation across targets, modules, descriptors, plugins, generics and build
options, with full-build output as the oracle. Preserve safe analysis mode.

Implement a resident Mac host behind a versioned C interface. Persistent data,
platform services and GPU resources remain host-owned. Quiesce R15 workers and
callbacks; ensure no old frame executes; replace procedure tables using stable
IDs rather than stored code addresses.

Layout changes require an explicit versioned migration, reusing R16's snapshot
inventory where it applies: allocate new state,
migrate/validate it and atomically publish or roll back. Keep old state until
success. Retire code/GPU resources only after their users and queues finish.

**Consumer/gate:** edit a real world/controller and shader while preserving
state, exercise incompatible migrations and rollback, and publish measured
edit-to-observable-frame latency. This is Mac development reload; signed iOS
device updates keep their signing/deployment rules. No arbitrary stack migration
or device code-loading bypass is implied.

## 26. R23 — Add backends only after profiling

The C backend stays a working reference. Add a fast native debug backend only
if C compilation/linking dominates the measured loop after caching and generated
code size are addressed. Add an optimizing backend independently if actual
runtime/code-size results justify its dependencies and maintenance.

Both consume the same checked semantics, target descriptions, effects and
source maps. Run the same language, ABI, uninitialized/overflow, verification,
debugging and platform suites. Compare whole-loop latency, output behavior,
runtime, code size and peak build memory on named workloads. An internal
bytecode evaluator is similarly conditional on measured evaluation cost; it is
not a mandatory runtime VM or a shortcut around semantics.

**Exit:** a measured gain on a real consumer without losing correctness or
tooling. Otherwise retain C output. Backend count is not the definition of the
full vision.

## 27. Feature coverage and completion policy

| Spec area | Phases that complete it |
| --- | --- |
| Representation, composition, strings, enums/unions and procedure forms (§4–9) | R2 basic forms; R3 ownership; R4 specialization; R6 storage transformations; R9/R15 vector and SIMD rules |
| Compile-time execution and conditional work (§10) | Existing bounded evaluation retained; R1 dependencies/`#if`; R5 metadata/derivation; R6 typed code; R8 plugins; R20 evaluator migration |
| Memory, context and errors (§11) | R2 typed errors/cleanup; R3 explicit storage/context; R4 owning libraries; R11 transitive effects; R14 bounded contracts |
| Jobs and determinism (§12–13) | R3 regions first; R10 target evidence; R11 effects; R15 math and schedules; R16 replay and snapshots |
| Public protocols (§14) | Existing importer retained; R5 schema-aware derivation; R8 reproducible inputs; R10 consumer/domain/target expansion |
| GPU programs (§15) | R5 target-aware metadata; R9 resources/MSL/WGSL pipeline; R15 deterministic restrictions where required |
| Builds, iteration and reload (§16) | R0 measurements/debugging; R1 graph; R8 workspaces/capabilities; R21–R22 cache/reload; R23 conditional backends |
| Modules, C/Swift interop and tooling (§17) | R1 namespaces/configuration; R2/R3 callback/ownership ABI; R7 formatter and safe compiler queries; R8 C-header import; R10 target matrix; R20 distribution |
| Physical-world foundations (spec stage 10; semantics added to the spec with each phase) | R12 units and frames; R13 clocks and ticks; R14 bounded control; R16 replay; R17 robot-model import; R18 sensors and supervision; R19 fault scenarios |
| Verification for human/agent development (§10, §17 and book chapters 11/23) | R0 independent gates/status; checks/oracles at every phase; R6 provenance; R7 safe analysis/evidence; R20 trusted-chain documentation |

C-header import in R8 should use a constrained importer with explicit handling
of macros, inline-only symbols, variadics, allocator pairing and unsupported
types. Pin any optional libclang/tool adapter in the workspace manifest; it
must not become a Python dependency for ordinary compilation. C variadics
remain unsupported unless their ABI is proved or an explicit wrapper is used.
Import does not mean parsing arbitrary C++: metal-cpp and other C++ APIs
stay behind reviewed `extern "C"` shims.

For every phase, update the affected spec/grammar and executable examples.
Require negative fixtures, relevant authority/mutation gates, a real consumer,
measured costs and a verification note explaining untested assumptions. Update
the book and generated status reference in the same release. Preserve working
native paths until replacement parity is demonstrated. Independent oracles and admission
rules cannot be weakened merely to make a port green.

Retain results with their source/dependency, target/profile, toolchain and
artifact identities. Re-run affected gates when those inputs or behavior change;
a remote commit or rebase alone is not a reason to repeat a full bootstrap or
unrelated suite. Evidence with changed or unknown required inputs is not
reusable. Fast edit-time selection and fuller release checks remain distinct;
this does not waive authority gates or release policy.

No phase introduces mandatory GC, hidden deep copies, exceptions, virtual
inheritance, automatic dependency fetching or platform-signing bypasses.
General-purpose memory safety and arbitrary formal proof are not implied by
the roadmap. The useful end state is an inspectable Coax development loop and
real applications with explicit contracts and evidence, not every imaginable
language feature.

### Optional bounded-verification research

Keep this off the critical path and outside ordinary compiler/build dependencies.
Try an external bounded checker on one real scalar decision, length calculation
or state transition with explicit pre/postconditions. Start without general
heap, concurrency, host calls or broad floating-point claims. An emitted-C
experiment is not automatically a proof of Coax semantics.

Use the same evidence format: source/lowering identity, assumptions, bounds,
tool version, execution semantics and outcome. Distinguish no counterexample
within an explored bound from coverage of all relevant executions; check loop
bound sufficiency explicitly. Continue only if one actual routine gets stronger
coverage than enumeration at manageable cost. No solver/proof syntax, mandatory
dependency or new milestone is justified before that result.

## 28. Next implementation sequence

This is a priority queue, not a serial barrier. Formatting, scoped evidence
and simple escape diagnostics can start from current APIs while R0 measurements
and review remain open; every slice still needs its listed prerequisites.

1. **Finish the existing R0 baseline.** Retain the closed pose median gap,
   frozen reference and current debugging results. Complete device/tail
   budgets, full-loop attribution and independent
   review enforcement. Do not repeat shipped kernel selection/debugging or
   release/status generation, or weaken a budget to close the milestone. Extend
   the shipped bounded differential tooling; start scoped evidence and the
   authoring-task corpus alongside this work.
2. **Extend semantic conformance and native check execution.** Preserve the
   0.25.0 independent profile selections and legacy defaults. Expand independent
   literal/ordering vectors and run eligible checks on emitted native code;
   broader target/profile evidence remains explicit. Keep the optional
   bounded-checker experiment separate.
3. **Build on R1's first delivery.** Declaration/content identities, explicit
   targets, original spans, basic namespaces and minimal inspectable procedure
   lowering have shipped. Use differential fixtures and independent oracles to
   pin execution semantics; scheduling/configuration can follow in smaller
   deliveries.
4. **Ship R7 tools early.** Formatting/evidence can use current APIs; typed
   queries and revision-bound fixes use R1. Restricted analysis and useful
   diagnostics must not wait for plugins, macros or owning containers.
5. **Continue R2 and R3, then owning storage.** Retain the shipped simple
   local-borrow diagnostics; add typed results, exhaustive cleanup,
   transfer/deferred-destruction rules and one real
   asynchronous adapter. R4 non-owning generics and R5 concrete derivation
   can proceed on their narrower prerequisites; owning libraries wait for R3's
   expansion.
6. **Test platform and iteration assumptions early.** Extend Coaxial's existing
   wasm32 probe and run R21's unchanged-layout Mac replacement experiment
   once their exact prerequisites exist. Neither implies full browser support
   or general transactional migration.
7. **Start the physical-world foundations on the simulated robot.** Once R4's
   type parameters ship, put the robot's joints and poses on R12 units and
   frames with built-in operators. Then add R11 effect summaries, R13 clocks
   and R14 bounded control step by step, each through the closed loop of
   Psionic chapters 167–169. Replay (R16) follows R15's strict profiles.
8. **Expand through consumers.** Complete richer generics/derivation, R8
   workspaces, GPU interfaces, jobs, robot-model import, supervision and fault
   scenarios using the dependency table. Continue self-hosting in reviewable
   slices; gate full migration/reload, optional proof tooling and extra
   backends on their own evidence.

## 29. Earlier phase labels

Earlier revisions used lettered sub-phases and a shorter list. Project queue
tasks keep their original slugs (for example `r4a-type-parameters`,
`r7a-format-evidence` and `r11a-reload-experiment`); this table maps each
earlier label to its phase now.

| Earlier label | Phase now |
| --- | --- |
| R0 | R0 — Evidence and debugging |
| R1a | R1 first delivery |
| R1b | R1 expansion |
| R2a | R2 first delivery |
| R2b | R2 expansion |
| R3a | R3 first delivery |
| R3b | R3 expansion |
| R4a | R4 first delivery |
| R4b | R4 expansion |
| R5a | R5 first delivery |
| R5b | R5 expansion |
| R6 | R6 — Typed code and storage transformations |
| R7a | R7 — Useful tools before privileged builds |
| R7b | R8 — Workspaces, plugins and build programming |
| R8a | R9 — Target-specific GPU interfaces |
| R8b | R10 — Browser vertical slice |
| R9 | R11 — Transitive effects, and R15 — Strict math and deterministic jobs |
| R10 | R20 — Self-hosting and the supported toolchain |
| R11a | R21 — Restricted procedure-body replacement |
| R11b | R22 — Resident semantics and transactional migration |
| R12 | R23 — Additional backends |
| (new) | R12 units and frames, R13 clocks, R14 bounded control, R16 replay, R17 robot-model import, R18 sensors and supervision, R19 fault scenarios |
