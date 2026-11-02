# Completing the four books

Reconciled against `main` on 2026-11-01, starting at `6f4c722640`. This is the
shared delivery sequence for [Coax](../../coax/book/README.md),
[Psionic](../../psionic/book/README.md), [Aethyr](../../aethyr/book/README.md) and
[Nostr](../../nostr/book/README.md). The [reader paths](paths.md) give smaller
routes through capabilities that already run. Each book's chapter registry
remains authoritative for its current teaching profiles.

## What completion means

Keep four different questions separate:

| Question | Where the answer lives | What it does not establish |
| --- | --- | --- |
| Is there a worked, runnable teaching chapter? | Chapter status and `profile` in `chapters.json`; worked trace, exact example, refused case and independent check in the chapter | Completion of every feature suggested by the title |
| On which targets was this profile checked? | `qualified_targets` and the chapter's retained source/build/run records | Qualification of another compiler, model, numerical profile or workload |
| Which code consumes its implemented components? | `adopted_by`, with the source-to-consumer scope explained in the book | Adoption of the entire chapter, live deployment or a successful acceptance project |
| Is the intended application complete? | The milestone gates below and each chapter's explicit remaining-work section | Success inferred from a chapter count or a simulated stand-in |

An empty adoption list means no reviewed consumer is recorded. It is not a
claim that none exists. Record a real library, application, host or build-binding
consumer; tests, teaching examples and fixture outputs belong in evidence.
Direct component use can be recorded without promoting a chapter's status or
adding a qualified target. A source load alone does not establish deployment.

`depends_on` names the minimum teaching prerequisites for the chapter's stated
implementation. This graph must be acyclic across all four books. A later
renderer, solver, importer or application may integrate an earlier component;
describe that integration in the chapter/plan, without making the foundation
depend on its future consumer. A forward reading reference is allowed.

## Coax: a dependable language and supported consumers

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Finish the current-language route | 5–10, 13–14, 16–20, 24–28 | Results/error handling, supported generic forms, memory/borrow boundaries, effects and target contracts have worked examples and diagnostics matching the compiler. Current limitations are visible before the reader uses a feature. |
| 2. Close the remaining ownership and development-loop gaps | 6, 14–16, 19–20 and roadmap R0 | A real fallible asynchronous consumer demonstrates buffer lifetime, cancellation and cleanup; compiler analysis has measured headroom; physical-device and development-loop gates retain their own evidence. |
| 3. Deliver physical-world language through consumers | 29–39 | Frames, clocks, bounded steps, integrators, replay, model import and supervision are used by an Aethyr world and a Psionic control path, with refused misuse and independent checks. |
| 4. Deliver simulation compute through a real solver | 40–43 | Layouts, compilation budgets, captured execution, buffer ownership and kernel derivatives serve a measured workload instead of parallel, unused abstractions. |
| 5. Finish the remaining toolchain and distribution route | 44–47 | A clean-machine real-application bootstrap, supported artifacts and reload/inspection behavior pass their declared gates. |

The retained-store B1 qualification is complete within its recorded boundary.
Do not reopen it based on an older stopping-point report. B2 helper-write
composition, B3 asynchronous request ownership and broader lifetimes remain;
the [current boundary](../../coax/verification-retained-stores.md) records them.

## Psionic: one complete model workflow

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Make a new model ordinary | 3–6, 38–40, 106–107, 114, 117–118, 121–124 | A new MLP and CNN use public modules, operators, autodiff, data, optimizers and checkpoints without runtime changes. The explicitly authored tensor IR is shared; arbitrary Coax extraction is not assumed. |
| 2. Make single-GPU training useful | 115–116, 125–131 | A larger small decoder trains with measured quality, throughput and memory, cooperative backward kernels, mixed precision and a reproducible independent baseline. The existing tiny GPU run remains valid even though it is slower than the CPU. |
| 3. Import, adapt and serve | 8, 79, 140, 172–181, 220–221 | A published model is imported, fine-tuned, saved, reloaded and served through real admission, cancellation, batching/cache and failure paths; toy/simulated serving results are not substituted. |
| 4. Prove a low-pressure game use | 24–32, 215–225, 239–241 | A Pylon consumer preserves local play on timeout/cancel, measures quality and latency against a strong local rule, and enables remote-selected behavior only after its own usefulness gate. Existing Directions remains local-first with optional shadow calls. |
| 5. Expand robotics and distribution from real consumers | 132–135, 167–171; Aethyr 17–37 | Neural actor/critic training uses a pinned real physical environment; batched contact/reset/sensor/replay behavior is checked before gait claims. Distributed training uses actual devices/interconnects and demonstrates failure recovery. |

The [framework acceptance projects](../../psionic/book/PLAN.md#a-framework-not-only-engines)
remain the graduation gates. Original model inference, a tiny GPU training run,
a simulated scheduler and a passing scalar LQR experiment are useful delivered
profiles; none alone satisfies those projects. Additional model architectures,
NVIDIA breadth, mesh inference and agent-harness chapters remain independent
branches with their written prerequisites and acceptance criteria.

## Aethyr: a complete playable engine route

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Consolidate a playable realm | 1–16, 56, 64, 65–69 | The Grid uses shared clocks, state, input, replay, queries, collision, walking, camera, renderer, HUD and budgets, with recorded native/browser behavior. Move other realms over subsystem by subsystem. Live audio is qualified separately from offline synthesis. |
| 2. Make Forge artifacts reproducible | 51–55, 82–86 | Edits share an invertible replay model; publication freezes document, engine build and assets; forks, save migration, host lifecycle and accessibility are checked on the usable route. |
| 3. Establish bounded physical worlds | 17–22, 31, 35–37 | A rigid/articulated contact scene imports, resets, batches and replays with measured resource use and a real Psionic controller/environment consumer. |
| 4. Broaden simulation and large worlds | 23–34, 38–50, 70–81 | Additional solvers, coupling, gradients, sensors, streaming and spacecraft fidelity qualify their own numerical/reference profiles. Genesis-scale scope remains a substantial later milestone. |

The [native GPU record](../verse/native-gpu.md) already documents generated Coax
shaders. The remaining rendering milestone is shared frame/pass/binding
admission and independent references, not repeating that port. The
[migration chapter](../../aethyr/book/ch064-every-realm-runs-on-aethyr.md) records
both adopted components and remaining private paths; a migrated clock is not a
whole migrated realm.

## Nostr: finish real consumers and operational qualification

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Close integration and evidence debt | 47, 52, 86, 97–104 | Live settlement uses the checked expiry rule; the changed NIP pin is reviewed; affected website and relay regressions have current evidence; current consumers and frozen teaching snapshots are labeled accurately. |
| 2. Qualify complete consumer profiles | 1–36, 97–100 | Intended native/browser signing, verification, encrypted storage and transport profiles have matching source/target evidence and explicit secret-handling boundaries. Remaining runner/unlock migrations are completed without inferring them from API verification. |
| 3. Finish the relay migration | 37–45, 103 | Required protocol wiring, production-sized persistence/tombstones, restart/import equality, shadow observations and rollback are qualified before the owner-operated cutover. Configurable host allocation alone does not prove production capacity. |
| 4. Connect integrated applications | 75–102; Aethyr 49; Psionic 24–32 | Rooms/presence, wallet interactions, Pylons and agent execution run through real transports and consumers. Assumed hop delays and offline payment traces remain teaching examples. |
| 5. Complete the treasury milestone | 104 | Actual signing, possession/recovery, relay carriage and independent signet recovery satisfy the chapter's remaining gates. |

Recent relay index/delete/rate/COUNT/search/protection/proof-of-work wiring,
host-supplied NIP-49 workspaces, streamed hashing and independent crypto
crosschecks are completed work within their records. Preserve those results.
The [Nostr completion route](../../nostr/book/COMPLETION.md) gives the detailed
remaining application and protocol scope. Existing owner actions stay in
[NEEDS_OWNER.md](../../NEEDS_OWNER.md).

## Keep the reconciliation current

Change a status only with its chapter marker, registry, contents and README/plan
labels. For changed source, run the focused checks and retain their real
producer identities; carry unchanged passing evidence rather than repeating
whole-model or whole-book execution. Preserve dated records and failed results.
The four tracks can advance in parallel around concrete shared consumers.

Metadata-only checks, without building a compiler or running examples:

```shell
python3 coax/tests/book_registry.py
python3 coax/book/check-structure.py --registry
python3 coax/book/check-structure.py
python3 psionic/tests/book.py
python3 psionic/tests/book.py --book aethyr
python3 psionic/tests/book.py --book nostr
```

These checks cover metadata, dependency cycles, adoption-path hygiene, local
links and source listings. They do not prove that a consumer uses the whole
profile or that a milestone is deployed. Review the actual source relationship
and acceptance evidence before recording either claim.
