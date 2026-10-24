# Risks and qualification experiments

Status: proposed experiments, 2026-10-22. No result is implied by selecting this architecture. Rank reflects impact on creation.

## Decision rule

Each experiment records its toolchain, browser, device, fixture, network, raw measurements, failures, and conclusion. [Performance](performance.md) defines resource targets; [Verification](verification.md) owns evidence and gates.

Failures block the capability or narrow its profile. Retain failed runs. Public evidence requires content rights.

## Ranked experiments

### 1. Structural edits can update one coherent world

**Risk:** Editable buildings become disconnected systems: the facade opens while collision remains, navigation uses old doors, or a roof seals a new extension.

**Experiment:** Edit the cottage, shared-wall rooms, floor holes, and adjacent buildings from [World model](world-model.md). Add rooms, move edges, cut openings, and undo. Fail stages, reorder completions, and place avatars in affected regions.

**Gate:** Every generation has matching geometry, collision, navigation, and selection. No player passes through a standing wall or falls through supported ground. At least 10,000 generated valid and invalid topology transactions complete without corrupted accepted state. Unsafe activation preserves the previous complete generation. Failure prioritizes semantic compiler correctness over new building kits.

### 2. Incrementality survives realistic edits and memory pressure

**Risk:** A “small edit” still hashes, compiles, uploads, or duplicates the whole world.

**Experiment:** Edit one wall, one material constant, one shader interface, and one behavior in small and profile-limit projects. Trace dirty graph nodes, bytes read, outputs rebuilt, GPU uploads, and simultaneous old/candidate residency. Repeat with a nearly full cache and canceled builds.

**Gate:** Unchanged artifacts retain identical identities and allocations where compatible. Work scales with the declared dependency closure rather than project size. Each workload meets its profile's [performance budgets](performance.md); candidate admission refuses insufficient residency safely. If dependencies are too broad, revise artifact boundaries before promising instant iteration.

### 3. The owned behavior VM preserves useful authoring speed

**Risk:** A safe language subset becomes too limited for actual games or takes too long to compile and migrate.

**Experiment:** Implement the same chase, patrol, inventory, puzzle, timed door, and multiplayer interaction behaviors in the specified TypeScript subset. Test closure/state rules, bounded collections, recursive-depth exhaustion, infinite loops, host-query caps, event storms, and failed migrations.

**Gate:** All scenarios use supported constructs and migrate compatible state without JavaScript escape. Exhausted fuel discards tick proposals atomically. Unsupported syntax receives diagnostics before execution. Measure creator completion and VM cost; failure changes the subset or tooling, never editor-origin safety.

### 4. Worker and language boundaries improve the complete loop

**Risk:** Rust/Wasm computation saves CPU time but serialization, copies, scheduling, or browser contention erase the benefit.

**Experiment:** Compare the baseline combined simulation/render-encoding worker against split workers on the same fixtures. Compare targeted Rust kernels with equivalent optimized TypeScript implementations for spatial queries, navigation, and procedural geometry. Record DOM input delay, tick cost, upload bytes, allocations, and snapshot age on desktop and touch profiles.

**Gate:** Choose a split only when end-to-end responsiveness or required isolation improves within memory limits; otherwise retain the simpler arrangement.

### 5. Browser isolation supports the intended creator experience

**Risk:** Separate preview origins, sandbox flags, worker rendering, pointer lock, storage, and optional shared memory interact poorly.

**Experiment:** Exercise the full hotbar workflow across qualified desktop and touch browsers, with permission denial, storage restrictions, non-isolated deployment, and screen-reader navigation.

**Gate:** Core editing works with transferable buffers and without pointer lock. No credential crosses into player content. Accessibility actions reach the same commands. Any unsupported combination receives an explicit profile result, not a blank world or weakened security policy.

### 6. Custom shaders remain recoverable

**Risk:** Valid custom GPU code causes device loss or browser-wide stalls that ordinary compilation checks cannot prevent.

**Experiment:** Use malformed, binding-incompatible, resource-heavy, and long-running shader corpora in disposable environments; trigger loss during compile and activation. Verify retained source and checkpoint recovery.

**Gate:** Invalid candidates never replace working pipelines; device loss follows the separate reconstruction path. Document residual GPU denial-of-service exposure. Public shader execution remains limited until the tested admission and isolation policy is acceptable; static validation cannot prove bounded execution time.

### 7. Natural-language actions reduce work without silent damage

**Risk:** Repeated clarification, incorrect targets, stale decisions, or missing catalog candidates make language slower than direct tools.

**Experiment:** Evaluate the cases in [AI authoring](ai-authoring.md), including placement counts, room extensions, wrong targets, unsupported languages, cancellation, and unknown requests. Compare exact-target accuracy and task completion with hotbar controls.

**Gate:** Each enabled route meets its explicit false-application and latency budgets on a held-out labeled set. Otherwise offer a concrete proposal or direct tool. Report clarifications separately from wrong edits and service failures.

### 8. Publishing and collaboration remain trustworthy during failure

**Risk:** A fast local preview conceals lost edits, invalid merges, private-cache leaks, or a release that references missing blobs.

**Experiment:** Kill services at transaction boundaries, disconnect collaborators, revoke rights, fill storage, race channel promotion, corrupt blobs, and reconnect an old offline branch. Run a clean player with no creator caches.

**Gate:** Exactly one accepted branch head and channel update wins each comparison; acknowledged commits remain recoverable. Releases start only from complete authorized dependencies. Incomplete exports declare their limits. Failure blocks publishing.

## Product and organizational unknowns

Qualify [authored game systems](game-systems.md) separately. Before general release, observe technical and nontechnical creators completing onboarding, rooms, behaviors, collaboration conflicts, and publishing. Record abandonment, errors, and accessibility failures. Revise confusing core interactions before expanding effects.

Name a decision owner per contract and a smallest qualifying workload per milestone. Revisit architecture when evidence contradicts it. Ownership must make replacement possible.
