# Completing the four books

Reconciled against `main` on 2026-11-01, starting at `6f4c722640`. This is the
shared delivery sequence for [Coax](../../coax/book/README.md),
[Psionic](../../psionic/book/README.md), [Aethyr](../../aethyr/book/README.md) and
[Nostr](../../nostr/book/README.md). The [reader paths](paths.md) give smaller
routes through capabilities that already run. Each book's chapter registry
remains authoritative for its current teaching profiles.

The follow-up review on 2026-11-01 uses `71c1d3fcdb` and the measured MNIST
MLP in `063a1a589a`. It turns the broad milestones below into eight
independently trackable issues. The books share this repository, so their
tasks belong in `OpenAgentsInc/openagents`, beside the existing book epics
[#12610](https://github.com/OpenAgentsInc/openagents/issues/12610) and
[#12612](https://github.com/OpenAgentsInc/openagents/issues/12612).

## The next structural work

The largest gap is joining implemented components into complete supported
workflows. More worked chapters help, but an operator, shader, signing vector
or simulated scheduler does not establish a usable application. Complete a
small route, measure it, then expand its supported profile. Keep four parallel
tracks; the CNN work does not wait for all compiler or relay work to finish.

| Priority and issue | Independently reviewable tasks | Exit |
| --- | --- | --- |
| Delivered infrastructure: [Supported reader routes and delivery evidence, #12624](https://github.com/OpenAgentsInc/openagents/issues/12624) | Give each book a clean-checkout entry command and observable application exit; bind evidence to source/compiler/target; run quick metadata checks; record published revision. | A reader can reproduce a named supported route, and a changed dependency invalidates only its affected evidence. |
| Delivered bounded route: [Coax outcomes and request ownership, #12625](https://github.com/OpenAgentsInc/openagents/issues/12625) | Integrate the initial R2 native LP64 language, real file cleanup, B2 helper composition and generation-fenced typed System One/Lab ownership. [Current qualification](../verification/coax-outcomes-12625-2026-11-02/README.md) binds focused checks, canonical compiler fixedpoints and matching-image analysis cost under the unchanged budget. | Actual native outcomes, cleanup, cancellation/reap/reuse and transcript lifetimes pass within their declared component profile. Whole-desktop E0381, normal full-release/cache gates and R0 physical/reviewer/page-golden gates remain separate; no general linear ownership is claimed. |
| P1: [Psionic public models, #12626](https://github.com/OpenAgentsInc/openagents/issues/12626) | Connect convolution to IR/autodiff/modules; train and reload a frozen CNN; add residual-model and unfamiliar-consumer acceptance; publish operator/backend limits. A same-shape residual block and frozen CPU trial now pass; independent artifact-reader checks are recorded, with fresh training still open. | Ordinary models use reusable APIs, with independent gradients, held-out quality and saved artifacts. The tiny CNN is a step toward ResNet/CIFAR, not that project's completion. |
| P2: [Useful single-GPU training, #12627](https://github.com/OpenAgentsInc/openagents/issues/12627) | Cooperative backward and resident whole-step state, precision/overflow/resume, then a frozen 10–50M decoder with independent baseline. | Quality, tokens/s, memory and startup are measured together on the 4080. Distributed scaling follows this useful baseline. |
| P1: [Aethyr shared rendering and playable route, #12628](https://github.com/OpenAgentsInc/openagents/issues/12628) | Common frame/pass/binding admission; migrate the Grid, then catalogue subsystems; qualify independent render profiles and resource refusals. | One playable route uses shared engine contracts on its declared native/browser targets; remaining private paths stay visible. |
| P2: [Reproducible Forge artifacts, #12629](https://github.com/OpenAgentsInc/openagents/issues/12629) | Common invertible edits, stale-revision refusal, pinned document/engine/assets, reload, migration and isolated forks. | A later engine release cannot silently change a published world's pinned artifact. |
| P1: [Nostr live consumers and evidence debt, #12630](https://github.com/OpenAgentsInc/openagents/issues/12630) | Close recorded pin/regression/settlement decisions, qualify complete signer/storage/transport profiles, migrate remaining actual consumers and connect applications individually. | Real consumer evidence covers its actual target and key boundary; offline traces and test keys retain their narrower claims. |
| P1: [Production relay qualification, #12631](https://github.com/OpenAgentsInc/openagents/issues/12631) | Freeze required parity, qualify durable import/restart and production budgets, refresh differentials, shadow traffic and rehearse rollback. | Every production exit is evidenced before a separate owner-operated cutover. |

[Issue #12624](https://github.com/OpenAgentsInc/openagents/issues/12624) now has
[executable bounded routes](routes.json), [delivery instructions](DELIVERY.md)
and [retained qualification](../verification/book-routes-2026-11-01/README.md):
23 native step/mode executions passed, then all 23 were retained with zero
example executions. Publication identity tracking is implemented and tested;
no real GitBook publication was performed for this change. These exits do
not promote the separate application milestones below.

Each issue has a task checklist and acceptance criteria. Execute it in bounded
slices and retain failed results; do not mark an umbrella complete because one
slice passes. The first Psionic slices delivered the MLP and reusable CPU convolution/CNN
through the same training/state interfaces. Residual-model and unfamiliar-consumer
acceptance come next. GPU and compiled-backward support must be qualified
separately or refuse explicitly.

The [Coax issue execution map](coax-outcomes-request-ownership.md) preserves
R2, analysis-cost, B2 and B3 identities and the independent R0 owner/reviewer
gates. The initial tagged-result language and real file adapter now join typed
System One/Lab request ownership, with actual native offline child traces and
snapshot/transcript lifetime oracles. The bounded raw helper-return continuation
preserves retained-only manual fallback. General ownership, broader roadmap R2
forms and whole-desktop readiness remain open; the execution map names those
boundaries separately from the delivered component route.

This ordering preserves completed work. B1 ownership qualification, generated
realm shaders, API NIP-98 verification and the relay's wired operations already
exist. The qualified analysis-cost candidate in the existing
`book30-analysis-cost` task reports 1,010,708 steps; B1's older 1,047,475-step
observation is historical, not a measurement of current main. Measure the exact
current compiler before adding analysis. The later [delivered wide-store H qualification](../verification/coax-wide-retained-stores-2026-11-01/README.md) reports 1,035,932 steps, leaving 12,644; the integrated R2 route has its own matching-image measurement in the current qualification record. Forge currently pins the document
revision but still plays it with the current engine, which is the concrete
artifact gap for #12629. Nostr's frozen conformance example must not be relabeled
as a current operational report.

Later branches still matter: Aethyr rigid/articulated worlds and simulator
interfaces, Psionic import/fine-tune/serve and real Pylon game decisions, and
Nostr treasury signing/recovery. They keep their existing prerequisites and
owner actions below. Advance them through actual consumers rather than
flattening them into a single chapter-count queue.

The [2026-11-01 stopping-point handoff](handoff-2026-11-01-12625-12628.md)
records the delivered polling, residual-model, dense-GPU and Grid-admission
slices, the earlier unmerged B2 compiler cost blocker, and the untouched remaining
queue. All agents in that team have stopped; Coder's chapter move is preserved.

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
Do not reopen it based on an older stopping-point report. B2's selected raw
helper-write composition preserves retained-only manual fallback and does not
discharge foreign retention or host lifetimes. Broader B2, B3 asynchronous
request ownership and general lifetimes remain; the
[current boundary](../../coax/verification-retained-stores.md) records them.

## Psionic: one complete model workflow

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Make a new model ordinary | 3–6, 38–40, 106–107, 114, 117–118, 121–124 | A new MLP and CNN use public modules, operators, autodiff, data, optimizers and checkpoints without runtime changes. The explicitly authored tensor IR is shared; arbitrary Coax extraction is not assumed. |
| 2. Make single-GPU training useful | 115–116, 125–131 | A larger small decoder trains with measured quality, throughput and memory, cooperative backward kernels, mixed precision and a reproducible independent baseline. The existing tiny GPU run remains valid even though it is slower than the CPU. |
| 3. Import, adapt and serve | 8, 79, 140, 172–181, 220–221 | A published model is imported, fine-tuned, saved, reloaded and served through real admission, cancellation, batching/cache and failure paths; toy/simulated serving results are not substituted. |
| 4. Prove a low-pressure game use | 24–32, 215–225, 239–241 | A Pylon consumer preserves local play on timeout/cancel, measures quality and latency against a strong local rule, and enables remote-selected behavior only after its own usefulness gate. Existing Directions remains local-first with optional shadow calls. |
| 5. Expand robotics and distribution from real consumers | 132–135, 167–171; Aethyr 17–37 | Neural actor/critic training uses a pinned real physical environment; batched contact/reset/sensor/replay behavior is checked before gait claims. Distributed training uses actual devices/interconnects and demonstrates failure recovery. |

The first bounded CPU MNIST MLP consumer is now measured in
[chapter 118](../../psionic/book/ch118-one-trainer-for-every-model.md): public
module/autodiff/trainer composition, caller-sized buffers, AdamW, a checked
checkpoint replay and named inference export, with a frozen held-out quality
rule. Its next bounded CPU CNN slice now reaches 88.5742% through reusable
convolution/flatten layers and the same trainer, with independent gradients,
replay and export checks. A public same-shape residual block now reaches
87.3535% with the same trainer and named state; its earlier failed linear-head
trial remains visible. A separate reader has reproduced retained MLP/CNN/residual
artifact inference after a documented recipe gap was fixed. This advances the
first milestone; fresh reader-led training, general CNN options, larger datasets,
GPU efficiency and ResNet/CIFAR acceptance remain open. See the
[residual delivery record](../verification/psionic-residual-framework-2026-11-01/README.md).

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

While this follow-up was in progress, `1e37eb87d0` added the reusable monochrome
HUD library and its recorded demo, moving chapter 15 forward. Preserve that
component and its evidence in #12628; shared frame/pass/binding admission and
complete realm adoption still need their own exits.

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
