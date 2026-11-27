# Completing the nine OpenAgents books

> **Scope.** Originally drafted on 2026-11-01 at `6f4c722640` for four books
> (Coax, Psionic, the OpenAgents Metaverse Engine and Nostr), this roadmap now
> tracks all **nine books** in the OpenAgents library — the eight
> Coax-implementation books plus the companion external-codebase study
> [Reverse Engineering Claude Code](../../claude-code/book/README.md). Each
> book's chapter registry (`chapters.json`) remains authoritative for teaching
> profiles, qualified targets, and direct adoption; start at
> [the OpenAgents books](README.md) and the nine-route catalog in
> [routes.json](routes.json).

## Library status at a glance (nine books, 832 chapters)

Reconciled against `origin/main` chapter registries on 2026-11-27. **Four of
the nine books now have 0 unimplemented chapters** (Everglade, Coder, Nostr,
and Reverse Engineering Claude Code). Across the remaining five books, 107
unimplemented chapters (71 design drafts, 36 planned) remain alongside the
integration, hardware, and production gates on partly implemented chapters.

| Book | Total | Implemented | Partly impl. | Design draft | Planned | Moved | Written | Unimplemented | Detailed plan |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| [The Coax Programming Language](../../coax/book/README.md) | 47 | 21 | 12 | 0 | 14 | 0 | 0 | 14 | [coax/book/PLAN.md](../../coax/book/PLAN.md) |
| [Building the OpenAgents Metaverse Engine](../../metaverse-engine/book/README.md) | 103 | 14 | 57 | 32 | 0 | 0 | 0 | 32 | [metaverse-engine/book/PLAN.md](../../metaverse-engine/book/PLAN.md) |
| [Building Everglade](../../everglade/book/README.md) | 33 | 3 | 30 | 0 | 0 | 0 | 0 | **0** | [everglade/book/PLAN.md](../../everglade/book/PLAN.md) |
| [Psionic: ML From Scratch](../../psionic/book/README.md) | 274 | 146 | 104 | 7 | 0 | 17 | 0 | 7 | [psionic/book/PLAN.md](../../psionic/book/PLAN.md) |
| [Coder: Coding Agents From Scratch](../../coder/book/README.md) | 43 | 11 | 20 | 0 | 0 | 0 | 12 | **0** | [coder/book/PLAN.md](../../coder/book/PLAN.md) |
| [Bitcoin From Scratch](../../bitcoin/book/README.md) | 125 | 101 | 3 | 0 | 21 | 0 | 0 | 21 | [bitcoin/book/PLAN.md](../../bitcoin/book/PLAN.md) |
| [Nostr From Scratch](../../nostr/book/README.md) | 104 | 88 | 1 | 0 | 0 | 15 | 0 | **0** | [nostr/book/COMPLETION.md](../../nostr/book/COMPLETION.md) |
| [The OpenAgents Book](../../openagents/book/README.md) | 78 | 0 | 45 | 32 | 1 | 0 | 0 | 33 | [openagents/book/PLAN.md](../../openagents/book/PLAN.md) |
| [Reverse Engineering Claude Code](../../claude-code/book/README.md) | 25 | 0 | 0 | 0 | 0 | 0 | 25 | **0** | [claude-code/book/OUTLINE.md](../../claude-code/book/OUTLINE.md) |
| **Total** | **832** | **384** | **272** | **71** | **36** | **32** | **37** | **107** | — |

The [reader paths](paths.md) and [supported-route catalog](routes.json) now
provide **nine executable bounded CPU routes** covering every
Coax-implementation book (`coax-current-ffi`, `aethyr-core-replay`,
`psionic-public-models`, `nostr-event-client`, `executor-network-flywheel`,
`bitcoin-crypto-settlement`, `coder-harness-memory`,
`everglade-world-srd-dragon`, and `openagents-app-economy`).

## Shared delivery issues (`#12624`–`#12631`) and historical baseline

The 2026-11-01 review (using `71c1d3fcdb` and the measured MNIST MLP in
`063a1a589a`) turned the first four books' cross-cutting milestones into eight
independently trackable issues in `OpenAgentsInc/openagents`, beside the
existing book epics
[#12610](https://github.com/OpenAgentsInc/openagents/issues/12610) and
[#12612](https://github.com/OpenAgentsInc/openagents/issues/12612). The largest
gap remains joining implemented components into complete supported workflows:
an operator, shader, signing vector, or simulated scheduler does not establish
a usable application on its own.

| Priority and issue | Independently reviewable tasks | Exit |
| --- | --- | --- |
| Delivered infrastructure: [Supported reader routes and delivery evidence, #12624](https://github.com/OpenAgentsInc/openagents/issues/12624) | Give each Coax book a clean-checkout entry command and observable application exit; bind evidence to source/compiler/target; run quick metadata checks; record published revision. Expanded from the original four routes to all nine routes in [routes.json](routes.json). | A reader can reproduce a named supported route, and a changed dependency invalidates only its affected evidence. |
| Delivered bounded route: [Coax outcomes and request ownership, #12625](https://github.com/OpenAgentsInc/openagents/issues/12625) | Initial R2 native LP64 language, real file cleanup, B2 helper composition and generation-fenced typed System One/Lab ownership are integrated. The [closure record](../verification/coax-12625-closure-2026-11-02/README.md) binds focused checks, canonical compiler fixedpoints and matching-image analysis cost under the unchanged budget. | The original 130 cache/origin survivors are closed; final `CB_query` kills 371/371, authored baseline passes 1,256/1,256, all native/fixedpoint gates pass, and actual compiler cost is 1,036,428/1,048,576. The [closure handoff](handoff-2026-11-02-12625-closed.md) records the completed route. Whole-desktop E0381, normal full-release/cache and R0 physical/reviewer/page-golden gates remain separate; no general linear ownership is claimed. |
| P1: [Psionic public models, #12626](https://github.com/OpenAgentsInc/openagents/issues/12626) | Connect convolution to IR/autodiff/modules; train and reload a frozen CNN; add residual-model and unfamiliar-consumer acceptance; publish operator/backend limits. A same-shape residual block and frozen CPU trial pass; independent artifact-reader checks are recorded, with fresh reader-led training and ResNet/CIFAR acceptance still open. | Ordinary models use reusable APIs, with independent gradients, held-out quality and saved artifacts. |
| P2: [Useful single-GPU training, #12627](https://github.com/OpenAgentsInc/openagents/issues/12627) | Cooperative backward and resident whole-step state, precision/overflow/resume, then a frozen 10–50M decoder with independent baseline. | Quality, tokens/s, memory and startup are measured together on the 4080. Distributed scaling follows this useful baseline. |
| P1: [Metaverse Engine shared rendering and playable route, #12628](https://github.com/OpenAgentsInc/openagents/issues/12628) | Common frame/pass/binding admission; migrate the Grid, then catalogue subsystems; qualify independent render profiles and resource refusals. | One playable route uses shared engine contracts on its declared native/browser targets; remaining private paths stay visible. |
| P2: [Reproducible Forge artifacts, #12629](https://github.com/OpenAgentsInc/openagents/issues/12629) | Common invertible edits, stale-revision refusal, pinned document/engine/assets, reload, migration and isolated forks. | A later engine release cannot silently change a published world's pinned artifact. |
| P1: [Nostr live consumers and evidence debt, #12630](https://github.com/OpenAgentsInc/openagents/issues/12630) | Close recorded pin/regression/settlement decisions, qualify complete signer/storage/transport profiles, migrate remaining actual consumers and connect applications individually. | Real consumer evidence covers its actual target and key boundary; offline traces and test keys retain their narrower claims. |
| P1: [Production relay qualification, #12631](https://github.com/OpenAgentsInc/openagents/issues/12631) | Freeze required parity, qualify durable import/restart and production budgets, refresh differentials, shadow traffic and rehearse rollback. | Every production exit is evidenced before a separate owner-operated cutover. |

[Issue #12624](https://github.com/OpenAgentsInc/openagents/issues/12624) has
[executable bounded routes](routes.json), [delivery instructions](DELIVERY.md),
and [retained qualification](../verification/book-routes-2026-11-01/README.md)
(initial 23 native step/mode executions passed and retained with zero example
re-executions; all nine routes are checked via `scripts/book_routes.py --check`).
Publication identity tracking is implemented and tested; all nine books publish
via native Coax Web static SSR (`/books/<slug>`), and six also publish to
GitBook spaces (see [`coax/book-maintenance.md`](../../coax/book-maintenance.md)).

The [Coax issue execution map](coax-outcomes-request-ownership.md) preserves
R2, analysis-cost, B2 and B3 identities and the independent R0 owner/reviewer
gates. The later
[wide-store H qualification](../verification/coax-wide-retained-stores-2026-11-01/README.md)
reports 1,035,932 steps (leaving 12,644 under the unchanged 1,048,576 budget),
and the [current closure record](../verification/coax-12625-closure-2026-11-02/README.md)
records the matching-image measurement for the integrated R2 route. The
[2026-11-01 stopping-point handoff](handoff-2026-11-01-12625-12628.md) preserves
the historical polling, residual-model, dense-GPU and Grid-admission slices and
Coder's chapter move.

## What completion means

Keep four different questions separate:

| Question | Where the answer lives | What it does not establish |
| --- | --- | --- |
| Is there a worked, runnable teaching chapter? | Chapter status and `profile` in `chapters.json`; worked trace, exact example, refused case and independent check in the chapter | Completion of every feature suggested by the title |
| On which targets was this profile checked? | `qualified_targets` and the chapter's retained source/build/run records | Qualification of another compiler, model, numerical profile or workload |
| Which code consumes its implemented components? | `adopted_by`, with the source-to-consumer scope explained in the book | Adoption of the entire chapter, live deployment or a successful acceptance project |
| Is the intended application complete? | The milestone gates below and each chapter's explicit remaining-work section | Success inferred from a chapter count or a simulated stand-in |

An empty `adopted_by` list means no reviewed direct consumer is recorded, not
that none exists. Record only real library, application, host or build-binding
consumers that directly load, call, or bind a chapter's source; tests, teaching
examples and fixtures belong in evidence. `depends_on` names the minimum
teaching prerequisites for a chapter's stated implementation and must stay
acyclic across all nine books.

## Real engineering gates remaining by book

### Books with remaining unimplemented chapters

#### 1. The OpenAgents Book (33 unimplemented: 32 design drafts, 1 planned; 45 partly implemented)

Product shipping in `apps/openagents-desktop/` (macOS Metal 4, Linux Vulkan,
Windows Win32/Vulkan `#14046`), `apps/openagents-ios/`, `openagents/core/` and
`openagents/runtime/oa-worker.coax`, Coax Web SSR (`components/ui/`), WorkOS
hybrid `acct_` auth, Breez Spark wallet, and the capability economy (`#14060`)
has moved ahead of the book's 4-rule chapter proofs:

- **Promote core app & runtime drafts (Ch 1–36):** bring the 45 partly
  implemented chapters and remaining GPU-component, login, session, wallet, and
  multi-platform host drafts to full 4-rule teaching status backed by runnable
  examples and target receipts (`openagents/book/PLAN.md`).
- **Close remaining planned product subsystems (Ch 20–22, 38–53, 60–61, 73):**
  complete knowledge base (`openagents/knowledge/`), sandboxed plugin SDK, and
  public Gym runner (`#14058`–`#14059`), keeping release/signing and Pylon
  Executor Network chapters (Ch 54–58, 69–70, 76–78) synced with real builds.

#### 2. The Metaverse Engine (32 design drafts; 14 implemented, 57 partly implemented)

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Consolidate a playable realm | 1–16, 56, 64, 65–69 | The Grid and real realms use shared clocks, state, input, replay, queries, collision, walking, camera, renderer, HUD (`1e37eb87d0`) and budgets, with recorded native/browser behavior and shared frame/pass/binding admission (#12628). |
| 2. Make Forge artifacts reproducible | 51–55, 82–86 | Invertible edit replay, stale-revision refusal, and pinned document/engine/asset artifacts so later engine releases cannot silently alter published worlds (#12629). |
| 3. Establish bounded physical worlds | 17–22, 31, 35–37 | A rigid/articulated contact scene imports, resets, batches and replays with measured resource use and a real Psionic controller/environment consumer (closing the Part IV physics drafts). |
| 4. Broaden simulation, space and multiplayer | 23–50, 70–81, 86–103 | Qualify remaining physics/spacecraft/world-system drafts and promote the partly implemented Part XII multiplayer room chapters (86–101) with measured drift/soak exits. |

The [native GPU record](../metaverse/native-gpu.md) documents generated Coax
shaders; the [migration chapter](../../metaverse-engine/book/ch064-every-realm-runs-on-the-metaverse-engine.md)
tracks adopted components separately from remaining private realm paths.

#### 3. Bitcoin From Scratch (21 planned; 101 implemented, 3 partly implemented)

All cryptographic primitives, BIP-340/ECDSA signatures, Taproot, transactions,
consensus building blocks, Lightning foundations, and `#authority` 3-leg
Spark/Lightning settlement (Ch 123) are implemented. The 21 remaining planned
chapters (`bitcoin/book/PLAN.md`) are the full-node operational and
differential-verification gates:

- **Chain sync & pruning (Ch 64, 76, 78–80):** UTXO pruning, parallel script
  checks/signature cache, `assumevalid`, AssumeUTXO snapshots, and full mainnet
  IBD matching Bitcoin Core's UTXO set.
- **Mempool & node operation (Ch 83–89, 92–93):** transaction relay/orphans,
  RBF/TRUC, packages/CPFP/anchors, eviction/persistence, fee estimation, block
  templates, node daemon lifecycle, notifications, and shadow mode beside
  Bitcoin Core.
- **Wallets, verification & OpenAgents integration (Ch 98–99, 110, 117–118, 120, 124):**
  Miniscript, descriptor wallet on our node, chain watcher, differential and
  Bitcoin Core functional test suites, formal properties, and running our node
  on Pylons and Metaverse OS.

#### 4. The Coax Programming Language (14 planned; 21 implemented, 12 partly implemented)

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Keep the current-language guide exact | 5–10, 13–14, 16–20, 24–28 | Results/error handling, supported generic forms, memory/borrow boundaries, effects, build drivers (`coax/build.sh`, `tool.sh`, `wasm.sh`, `serve.sh`) and target contracts match the compiler. |
| 2. Close ownership & development-loop gaps | 6, 14–16, 19–20, 41, 46 and roadmap R0 | Real fallible asynchronous consumer lifetime/cleanup, analysis headroom, R22 startup/kernel budgets, and physical-device/development-loop evidence. |
| 3. Deliver physical-world language through consumers | 29–39 (planned: 31, 33–39) | Bounded control, replay/snapshots, robot model import, stamped sensors/actuators, fault replay, provenance, and robot/freighter capstones driven by Metaverse Engine and Psionic consumers. |
| 4. Deliver simulation compute through a real solver | 40–43 (planned: 40, 42–43) | Logical tensors, device graphs/zero-copy buffers, and kernel derivatives serving a measured workload. |
| 5. Finish the toolchain route | 44–47 (planned: 44–45, 47) | Self-hosted clean-machine bootstrap, live procedure-body replacement and state migration atop `coax serve` (Ch 46), and profiling-driven backends. |

Retained-store B1/B2/B3 boundaries are recorded in
[`coax/verification-retained-stores.md`](../../coax/verification-retained-stores.md).

#### 5. Psionic: ML From Scratch (7 design drafts; 146 implemented, 104 partly implemented, 17 moved)

All Core ML, Decision/Pylon, Inkling 2 Mini Metal 4/CUDA + Desktop (Ch
266–271), Exact Compiled Weight Executors (Ch 226–238), and Byzantine-safe
DiLoCo/Quarantine Optimizer (Ch 272–274) chapters are implemented. Only **7
design drafts** remain — all in the physical robotics capstone (**Ch 256,
260–265**):

| Order | Existing chapters | Completion gate |
| --- | --- | --- |
| 1. Make a new model ordinary (#12626) | 3–6, 38–40, 106–107, 114, 117–118, 121–124 | Fresh reader-led training, general CNN/pooling options, larger datasets, and ResNet/CIFAR acceptance beyond the retained CPU MNIST MLP/CNN (88.5742%) and residual-block (87.3535%) records ([residual record](../verification/psionic-residual-framework-2026-11-01/README.md)). |
| 2. Make single-GPU training useful (#12627) | 115–116, 125–131 | Cooperative backward kernels, mixed precision, resume, and a frozen 10–50M decoder measured against an independent baseline on RTX 4080. |
| 3. Complete serving, mesh & large-model profiles | 41–43, 48, 50–51, 53, 68–69, 71, 74, 76, 78, 81–87, 99–102, 173–199 | Promote partly implemented serving scheduler/cache/failure paths, multi-device/mesh inference, and unmeasured original-weight profiles (Mammoth Metal, 120b GPU, DeepSeek V4.1 Flash, Mistral Large 4). |
| 4. Prove low-pressure game & Pylon usefulness | 24–32, 215–225, 239–241 | Gate optional remote Pylon decisions against the strong deterministic local rule in Everglade Directions. |
| 5. Deliver the Microduck robotics pipeline (7 drafts) | 132–135, 167–171, 256–265; Metaverse Engine 17–37 | Implement neural walking policy training, skill switching, vision/perception, policy export/supervision, real-robot commissioning, and multi-robot course repeatability (Ch 256, 260–265; see [framework acceptance projects](../../psionic/book/PLAN.md#a-framework-not-only-engines)). |

### Books at 0 unimplemented chapters (maintenance & promotion gates)

- **Nostr From Scratch (88 implemented, 1 partly implemented, 15 moved — 0 unimplemented):**
  Close remaining live-consumer and operational gates tracked in `#12630`,
  `#12631`, and [nostr/book/COMPLETION.md](../../nostr/book/COMPLETION.md):
  settlement expiry wiring, NIP-KEYS pin review, hosted runner/unlock signer
  cutovers (`OA_RUNNER_SIGNER=coax`, `COAXIAL_UNLOCK_SIGNER=coax`), production
  relay persistence/shadow/rollback qualification, and Ch 104 treasury
  recovery. Owner decisions stay in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
- **Coder: Coding Agents From Scratch (11 implemented, 20 partly implemented, 12 written — 0 unimplemented):**
  Keep the 20 partly implemented harness chapters synced with unified
  `openagents/core/` (`oa-worker`), promote remaining bounded CLI/harness
  chapters to full 4-rule teaching status, and maintain the 12 external-harness
  `written` study chapters (`coder/book/PLAN.md`).
- **Building Everglade (3 implemented, 30 partly implemented — 0 unimplemented):**
  Upgrade the 30 partly implemented world/SRD/multiplayer/city chapters into
  full 4-rule chapters with standalone runnable Coax/`gamebuilder` examples
  alongside Ch 4, 14, and 33, once Everglade II city interiors, NPC routines,
  player trade wire commands, and 150-player zone gates land
  (`everglade/book/PLAN.md`).
- **Reverse Engineering Claude Code (25 written — 0 unimplemented):**
  Complete 25-chapter bottom-up teardown of Claude Code 2.1.88–2.1.331
  (`status: written`, external survey; `claude-code/book/OUTLINE.md`). Maintain
  revision pins and cross-links as new snapshots are surveyed; no Coax target
  or `adopted_by` claims apply.

## Keep the reconciliation current

Change a chapter's status only together with its chapter marker, registry
entry, `SUMMARY.md` label, and README/plan row. For changed source, run focused
checks and retain real producer identities; carry unchanged passing evidence
rather than repeating whole-model or whole-book execution.

Metadata-only checks across all nine books, without building a compiler or
running examples:

```shell
scripts/check-books.sh --check
```

These checks cover metadata, dependency cycles across all nine books,
adoption-path hygiene, local links, capability claims, and source listings.
They do not prove that a consumer uses an entire chapter profile or that a
milestone is deployed; review the actual source relationship and acceptance
evidence before recording either claim.
