# Coax implementation roadmap

Updated 2026-10-28. Baseline: **coax0 0.13.0**, including source composition,
CPU layout reflection and the first R0 benchmark/source-debugging slice. This roadmap reconciles the technical review dated
2026-10-28 with the code that has actually shipped.

The goal is a systems language in which OpenAgents and Metareal can express
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

The review describes an older 0.10.0/0.11.0 snapshot. Preserve its criticism
of missing evidence, but do not schedule existing features as new work.

| Area | Shipped | Still missing |
| --- | --- | --- |
| Language core | Scalars, strings, structs/defaults, enums, fixed arrays/views, loops, cleanup, checked casts and ordered evaluation | Tagged results/unions, exhaustive switching, richer procedures, pointers, owned containers and generics |
| Compile-time work | Constants, bounded `#run`, assertions, finite checks, execution coverage for authority units, mutation testing | Typed quotation/insertion, general declaration generation, plugins and a dependency scheduler |
| Source graph | Checked `#load`, physical-file ownership, bounded graphs, original-file diagnostics/traps, `coax deps` | Namespaces/visibility, parameterized modules, content manifests, packages and incremental invalidation |
| Reflection | Schema-1 CPU type/field/enum records; constant projections and `#run` tables; a real Haunted House layout consumer | First-class types, stable declaration identity, richer metadata, alternate-target layouts and derivation |
| Compiler | Coax lexer/parser/checker/source policy, descriptor importer, wire runtime and fingerprinting; explicit reproducible seeds | Coax evaluator, verifier/mutator, emitters and driver; complete debugger/type integration and a resident compiler |
| Native product | Actual iOS scene/HUD, Grid, movement, animation, wallet rules, Haunted House and voice logic | Measured whole-frame parity against a frozen native reference, broader platform/ABI evidence and complete product migration |
| Protocols | Selected proto3 descriptor imports, bounded borrowed codecs and real app consumers | Remaining consumer/domain admission paths and consumer-justified schema features |
| GPU/platforms | Scalar visual functions emitted as Metal; native stage wrappers; Mac build host and iOS integration | Shared GPU resource declarations, general shader profiles, browser vertical slice and evidence for other targets |
| Builds/iteration | Single-shot C11 output, shell/Xcode integration, JSON diagnostics, original-source line/name maps and phase timings | Capability-aware `build.coax`, safe editor analysis, cache, transactional reload and additional backends |

The [source-unit evidence](../docs/verification/coax-source-units-2026-10-28.md)
and [reflection evidence](../docs/verification/coax-layout-reflection-2026-10-28.md)
record recent results and their limits. Native runtime traps already retain
source ownership; native debugger stepping and name maps now reach Coax files. The pulse
example establishes native behavior parity, not a measured full-engine speed
claim. No cross-platform floating-point, race-freedom or general memory-safety
guarantee follows from the current tests.

## 2. Order and dependency rules

Finish R0 first. R1–R7 build the language and compiler infrastructure for the
larger vision. Prove native GPU and browser integration in R8 before committing
to a new backend. Effects work starts with ownership in R3; the full parallel
and deterministic runtime is R9. Move the remaining compiler into Coax in R10,
then complete resident compilation/reload in R11. R12 adds backends only if
measurements justify them.

The numbers give the default priority, not an instruction to block an
independent experiment behind unrelated features. In particular, a browser
feasibility test belongs in R1; the first GPU/browser slice does not require
parallel jobs, full macros or a self-hosted compiler. Coax compiler migration
can proceed in small pieces before R10 once each piece has its prerequisites.
Resident compilation also does not require completed self-hosting; R10 and
R11 may overlap once their own dependencies and trust boundaries are satisfied.

| Milestone | Required foundations | First useful result |
| --- | --- | --- |
| R0 — Evidence and debugging | Current compiler/app | A measured live engine kernel that can be debugged from Coax source |
| R1 — Compiler graph, targets and modules | R0 baseline | Explicit declarations, target descriptions and safe source/module queries |
| R2 — Results and complete control flow | R1 graph; retain current ABI | A real fallible native adapter expressed with typed errors and cleanup |
| R3 — Ownership, memory and context | R2 errors; R1 targets | A bounded owning subsystem with diagnosed invalidation and explicit host lifetimes |
| R4 — Generics and foundational libraries | R1–R3 | Reusable containers and algorithms with constraints and bounded specialization |
| R5 — Source-aware metadata and derivation | R1 identities/targets, R4 type parameters | One generated engine registry/inspector with stable identities and provenance |
| R6 — Typed code and storage transformations | R5 metadata, R1 scheduler | Hygienic checked generation and a measured AoS/SoA consumer |
| R7 — Workspaces, plugins and tools | R1 manifests, R2 errors, R5/R6 checked generation | A real asset/build workflow and safe agent/editor queries |
| R8 — Native GPU and browser proof | R1 target ABI, R3 resource ownership, R5 layouts; R7 build inputs for reproducibility | The same selected simulation/data path reaches native Metal and a browser |
| R9 — Effects, strict math and jobs | R3 regions, R4 libraries, R1 target rules | A subsystem replays under checked effects at 1/2/4/8 workers |
| R10 — Complete compiler migration and distribution | The data, ownership and generation facilities each compiler pass needs | Coax semantic passes and emitters with a small native effects boundary |
| R11 — Resident compiler and transactional reload | R1/R7 dependencies, R5 stable IDs, R3 ownership; R9 quiescence when workers are used | Measured Mac edit/reload with state preservation and rollback |
| R12 — Additional backends | Earlier adoption/target evidence and a measured backend bottleneck | A faster debug or optimized path conforming to the reference semantics |

### How this relates to spec section 20

The spec's existing stage numbers remain useful topic identifiers. Stages 0/1
are the bootstrap foundation; unfinished stage 2 evidence is R0/R2/R3. Stage 3
expands through R1/R4/R5/R6. Stage 4 is R7. Stage 5 is R3/R9. Stages 6/7 are
R5/R8 and ongoing protocol adoption. Stage 8 is R11, and stage 9 is R12.
R10 is an explicit compiler migration track across those topics.

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
See [methodology](../metareal/benchmarks/README.md), [debugging](DEBUGGING.md) and
[retained results](../docs/verification/coax-r0-2026-10-28.md).

**R0 remains open.** Output tolerance and observed zero-allocation checks pass;
the initial optimized pose comparison misses the proposed 5% median budget.
The current measurements are Mac/Simulator evidence, not phone performance or
whole-engine parity. Finish optimization/profiling, physical-device and tail
budgets, asset/install/launch/GPU-preparation attribution, status generation
and independent reference/gate review before closing this milestone.

1. Select and freeze a kernel already called by the live app, preferably the
   animation pose path in `HomeRenderer.swift`/`metareal/cast.coax`, or the
   player/collision update if profiling identifies it as more representative.
   Record inputs, expected outputs, reference source revision and algorithms.
   Use the historical native implementation where available; otherwise require
   independent review of a separate reference. The archived paynet pulse is a
   later candidate until its real source/data/host path is available. A deleted
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
4. Emit original-source mappings and test native compiler errors, sanitizer
   reports and LLDB breakpoints/stepping/call stacks across loaded units.
   Start with line directives plus a durable source map; validate rather than
   assuming line directives solve variable/type inspection or generated code.
5. Define one release/status source from which CLI version, accepted-feature
   tables and book status are generated. Add a publication drift check. Keep
   implementation version, schema version and historical introduction version
   distinct. The manual 0.12.0 corrections in these docs are not that generator.
6. Preserve independent checks and frozen compatibility inputs. Define who
   reviews changes to invariants, oracles and verification policy, with the
   enforcement outside the compiler on owned infrastructure. An author who can
   weaken both code and checks cannot certify their own authority by compiling.

**Exit:** reproducible kernel and whole-loop reports; zero unexpected output or
allocation regressions; usable original-source native debugging; a versioned
baseline and an independently enforced review/gate policy. If Coax worsens
reliability or maintenance, stop feature expansion and fix that first.

## 4. R1 — Build the compiler substrate once

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

**Consumer/gate:** split one real engine subsystem and the compiler graph into
modules without weakening authority or changing behavior. Test cross-module
cycles, private names, configuration invalidation and original diagnostics.
Run a deliberately small wasm32 feasibility test here: types/layout, a host
import, memory access and a useful source location. This is not browser support.

## 5. R2 — Make fallible application code expressible

Implement tagged unions, optionals, exhaustive `switch`, typed results,
ignored-error diagnostics, `try` and `or_else`. Propagation must execute active
`defer` cleanup exactly once in reverse order. Keep fatal traps distinct from
recoverable domain errors; foreign error domains/codes must survive adapters.

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

## 6. R3 — Make ownership and allocation policies explicit

Specify non-null/nullable pointers, address/dereference operations, initialization,
borrows and invalidation before adding an allocator-shaped convenience API.
Retain shallow value/view semantics. Define a conservative region discipline
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
Start effect summaries here; do not enable parallel execution yet.

## 7. R4 — Generics, constraints and foundational libraries

Implement `$T`/`$N` parameters, generic procedures/types and constraint checking
with source-level instantiation traces. Specialization keys include resolved
types, values, targets and relevant options/effects. Bound recursive instances,
cache repeated work and report instance counts, time and code size.

Add explicit argument baking through the same mechanism. Do not specialize
every constant call silently. Operator overloads require pure, nonallocating,
nonfallible behavior; no allocation or I/O behind math syntax. Do not overload
assignment, field lookup, short-circuiting or calls.
Admit overloads only when conservative transitive effect checks enforce these
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

Build on the shipped layout queries. Add first-class `Type`, borrowed `Any`,
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

**Consumer/gate:** replace a hand-maintained Metareal registration or inspector
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

## 10. R7 — Build workspaces, plugins and safe tooling

First define isolated workspaces, staged typed events and an input manifest.
Then add plugins through scoped handles and checked proposals. Generated work
must converge before `complete`; a plugin cannot mark unchecked code valid.
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

Ship a formatter, typed symbol/layout/effect queries, expansion views and a
safe compiler-service/LSP analysis mode. Agents should get stable diagnostics,
dependency/provenance queries and evidence records, not scrape emitted text or
silently execute privileged plugins. Report missing generated inputs honestly.

**Consumer/gate:** generate and build one real asset/registry/app workflow.
Test deterministic outputs, denied effects, changed-input invalidation,
plugin failure, stale cache rejection and editor operation without credentials
or host side effects. Pause if conventional generation is simpler and delivers
the same useful workflow.

## 11. R8 — Prove the complete native GPU and browser path

### R8a — Target-specific GPU interfaces

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

### R8b — Browser vertical slice

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

## 12. R9 — Enforce effects, strict math and deterministic jobs

Complete transitive effect summaries for allocation, host I/O, clocks, random
sources, foreign calls, atomics and region reads/writes. Check actual region and
range overlap, including aliases carried by views/pointers; field names do not
establish disjointness. Unverifiable foreign effects need explicit reviewed
summaries or remain serial/unsafe. Validate lifetime and release across jobs.

Specify atomic ordering and the memory model before exposing concurrent APIs.
Build the job scheduler, disjoint partitions, `for_parallel`, worker-local
context/scratch, cancellation and defined reductions. Reject conflicting jobs
before claiming a safe schedule. Keep a serial implementation as the oracle.

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

## 13. R10 — Finish self-hosting and the supported toolchain

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
R2–R7 provide enough facilities for the next compiler pass.

## 14. R11 — Resident compiler and transactional reload

Use content/declaration dependencies to build a bounded resident cache. Test
invalidation across targets, modules, descriptors, plugins, generics and build
options, with full-build output as the oracle. Preserve safe analysis mode.

Implement a resident Mac host behind a versioned C interface. Persistent data,
platform services and GPU resources remain host-owned. Quiesce workers and
callbacks; ensure no old frame executes; replace procedure tables using stable
IDs rather than stored code addresses.

Layout changes require an explicit versioned migration: allocate new state,
migrate/validate it and atomically publish or roll back. Keep old state until
success. Retire code/GPU resources only after their users and queues finish.

**Consumer/gate:** edit a real world/controller and shader while preserving
state, exercise incompatible migrations and rollback, and publish measured
edit-to-observable-frame latency. This is Mac development reload; signed iOS
device updates keep their signing/deployment rules. No arbitrary stack migration
or device code-loading bypass is implied.

## 15. R12 — Add backends only after profiling

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

## 16. Feature coverage and completion policy

| Spec area | Milestones that complete it |
| --- | --- |
| Representation, composition, strings, enums/unions and procedure forms (§4–9) | R2 basic forms; R3 ownership; R4 specialization; R6 storage transformations; R8/R9 vector and SIMD rules |
| Compile-time execution and conditional work (§10) | Existing bounded evaluation retained; R1 dependencies/`#if`; R5 metadata/derivation; R6 typed code; R7 plugins; R10 evaluator migration |
| Memory, context and errors (§11) | R2 typed errors/cleanup; R3 explicit storage/context; R4 owning libraries; R9 transitive effects |
| Jobs and determinism (§12–13) | R3 regions first; R8 target evidence; R9 checked effects, math and schedules |
| Public protocols (§14) | Existing importer retained; R5 schema-aware derivation; R7 reproducible inputs; R8 consumer/domain/target expansion |
| GPU programs (§15) | R5 target-aware metadata; R8 resources/MSL/WGSL pipeline; R9 deterministic restrictions where required |
| Builds, iteration and reload (§16) | R0 measurements/debugging; R1 graph; R7 workspaces/capabilities; R11 cache/reload; R12 conditional backends |
| Modules, C/Swift interop and tooling (§17) | R1 namespaces/configuration; R2/R3 callback/ownership ABI; R7 C-header import, formatter and safe compiler queries; R8 target matrix; R10 distribution |
| Verification for human/agent development (§10, §17 and book 19/25) | R0 independent gates/status; checks/oracles at every milestone; R6 provenance; R7 safe analysis/evidence; R10 trusted-chain documentation |

C-header import in R7 should use a constrained importer with explicit handling
of macros, inline-only symbols, variadics, allocator pairing and unsupported
types. Pin any optional libclang/tool adapter in the workspace manifest; it
must not become a Python dependency for ordinary compilation. C variadics
remain unsupported unless their ABI is proved or an explicit wrapper is used.
Import does not mean parsing arbitrary C++: metal-cpp and other C++ APIs
stay behind reviewed `extern "C"` shims.

For every milestone, update the affected spec/grammar and executable examples.
Require negative fixtures, relevant authority/mutation gates, a real consumer,
measured costs and a verification note explaining untested assumptions. Update
the book and generated status reference in the same release. Preserve working
native paths until replacement parity is demonstrated. Independent oracles and admission
rules cannot be weakened merely to make a port green.

No phase introduces mandatory GC, hidden deep copies, exceptions, virtual
inheritance, automatic dependency fetching or platform-signing bypasses.
General-purpose memory safety and arbitrary formal proof are not implied by
the roadmap. The useful end state is an inspectable Coax development loop and
real applications with explicit contracts and evidence, not every imaginable
language feature.

## 17. Next implementation sequence

1. Record the R0 benchmark kernel/reference, input corpus and acceptance budget.
2. Add stage timing/generated-size/allocation reports and establish the baseline
   on the actual frame path; retain raw measurements.
3. Implement and test original-source native debugging before increasing the
   amount of generated/generic code developers must debug.
4. Add release/status generation and independently enforced oracle/invariant
   change review; keep the existing compiler/bootstrap/product gates green.
5. Begin R1's declaration/target graph, use it for real modules and run the small
   wasm32 feasibility test. Then proceed through the dependency table, shipping
   consumer-sized slices rather than attempting an entire phase in one patch.
