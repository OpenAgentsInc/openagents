# Effect vs Rust for the OpenAgents Core: The Architecturally Sensible Split

Date: 2026-07-17
Class: architectural analysis — a clean-sheet argument from product requirements
and cross-product teardown evidence only. This document deliberately makes **no
reference to OpenAgents' actual current code, packages, or implementation
state**; it is not a status report and confers no sequencing authority. Every
external claim is traceable to the teardown catalog (evidence labels are the
catalog's: [source]/[schema]/[test]/[inferred] live in the cited documents, not
re-derived here).

Inputs: the three surface-vision ProductSpecs (desktop trust-complete
workbench, mobile any-host fleet controller, openagents.com trust surface, all
rev 3), the full-catalog teardown synthesis
(`docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`),
and the deep teardowns of Codex (Rust, ~1.13M lines), Grok Build (Rust, ~1.35M
lines), OpenCode V2 and its Effect architecture (TypeScript/Effect), T3 Code
(TypeScript/Effect 4), and Claude Code (TypeScript, Bun-compiled).

---

## 1. Frame the decision by workload, not by fashion

"Effect vs Rust" is a false binary the moment it is asked about *the product*
instead of about *a workload*. The audited market contains flourishing examples
of both: two of the strongest engines in the catalog are million-line Rust
codebases (Codex, Grok Build), and two of the strongest are whole-app Effect
codebases (OpenCode V2, T3 Code). Neither camp's success is attributable to
the language per se; each is attributable to a specific architectural property
the language made cheap:

- The Rust engines are strongest exactly where the workload touches the
  operating system as an adversary or a hard-real-time peer: compiled
  containment (Seatbelt, bubblewrap+seccomp, restricted tokens), PTY and
  frame-time rigor, single static distributable binaries, no-GC latency floors.
- The Effect engines are strongest exactly where the workload is *coordination
  under partial failure*: schema-first typed contracts, durable admission
  before execution, structured concurrency with interruption as control flow,
  one request processor across every transport, service graphs replaceable in
  tests.

So the honest question is: given the product the vision docs describe, which
workloads does it actually contain, and which property does each workload need
more — OS truth and latency floors, or typed coordination and velocity?

## 2. What the product actually requires

Reading the three surface specs as requirements (not as implementation), the
product is, in order of architectural gravity:

1. **A trust machine.** Authority manifests paired with execution receipts on
   every run (desktop AC-2); named execution profiles compiled to OS
   enforcement that *fail closed* when the platform cannot represent the
   policy (AC-3); a public trust ledger where third parties mechanically
   verify release manifests and receipts (web AC-4); counters that reconcile
   exactly to receipted rows (web AC-8). The differentiating half of the
   product is proof, not features.
2. **A durable coordination engine.** Queue/steer/interrupt as three explicit
   verbs with typed admission states (desktop AC-1); exactly-once command
   resolution from a subway tunnel (mobile AC-3, SM-4 committed at 99.9%);
   restart-survivable runs re-derived from durable state (desktop AC-18,
   AC-20); a run that never halts on a routable limit and rotates across every
   account and provider the user has (AC-19/AC-21); same-thread cross-provider
   handoff (AC-22); ≥99.5% forced-restart session recovery (SM-4 committed).
3. **A many-clients projection system.** One typed engine protocol consumed
   identically by desktop renderer, terminal, mobile, and web (all three
   specs); the complete agent tree rendered from a canonical persisted graph
   (desktop AC-4, mobile AC-7); three read surfaces (bounded projection,
   replayable log with sync marker, lossy live stream) as contract.
4. **An orchestration layer over foreign systems.** Provider adapters for
   multiple vendors' harnesses and accounts, fleet capacity as receipted
   quantities (AC-15, mobile AC-10), Full Auto routing policy over all of it.
5. **A small set of genuinely native problems.** OS-level containment
   compilation; PTY/terminal hosting; media/audio; local inference on Apple
   silicon (desktop scope: local-model fallback lane); E2EE relay primitives;
   six-target signed distribution with update appliers that swap running
   binaries atomically.

Item 5 is real but *narrow*. Items 1–4 are the overwhelming majority of the
system's surface area, and they are all coordination, typing, projection, and
policy — not syscalls.

## 3. The workload classes

Derived from the requirements above, the decision decomposes into thirteen
workload classes:

- **W1 — Typed protocol / engine seam.** Thread→Turn→Item extended with Work
  Unit, Authority Manifest, Execution/Delivery Receipt; generated clients;
  drift-tested fixtures.
- **W2 — Durable admission and event-sourcing.** Idempotent input admission
  before scheduling; durable per-aggregate logs; projections; replay-to-live
  markers; exactly-once outbox replay.
- **W3 — Policy, authority, receipts.** The authority compiler (requirements ∩
  user policy ∩ profile ∩ containment), approval taxonomy, manifest/receipt
  emission, promise/ledger reconciliation.
- **W4 — Provider adapters and orchestration.** Harness adapters (JSON-RPC
  subprocesses, ACP, SDK streams), Full Auto routing/rotation, fleet
  dispatch, guidance/continuation loop.
- **W5 — OS-enforcement containment.** Compiling named profiles into
  Seatbelt/bubblewrap+seccomp/restricted-token policy; managed egress proxy;
  fail-closed refusal.
- **W6 — PTY / terminal.** Real pseudo-terminals, native-scrollback
  rendering, frame-time budgets, terminal-host crash restoration.
- **W7 — Media / audio.** Device I/O, resampling, bounded buffers, low-jitter
  packetization for the voice contract the desktop spec defers but names.
- **W8 — Local inference.** The Apple-silicon local-model lane (desktop scope).
- **W9 — Sync / relay / E2EE.** Owned relay ("reachability without
  authorization"), DPoP-bound capability tokens, device pairing crypto,
  cross-device projection transport.
- **W10 — Signed distribution and update appliers.** Six-target packaging,
  Ed25519 release-set manifests, atomic slot swap, drain-before-update,
  rollback.
- **W11 — Web/API surfaces.** The trust ledger, counters, promise registry,
  public API, supervision-parity client.
- **W12 — Renderer/UI.** Settled elsewhere (typed component contract over
  swappable renderers); noted here only because its language choice
  (TypeScript) exerts gravity on everything that feeds it projections. Not
  relitigated.
- **W13 — Tests, oracles, and behavior contracts.** The verification fabric
  the specs make load-bearing (oracle tests per AC, fault injection,
  screenshot matrices, replayable fixture runs).

## 4. What the Rust codebases actually bought — and what it cost

**Bought** (evidence: Codex and Grok Build teardowns):

- **Containment compiled to OS truth.** Codex is the only audited product
  whose permission profiles compile into real platform enforcement on all
  three OSes — Seatbelt policy generation on macOS, bubblewrap + namespaces +
  seccomp on Linux, restricted-token sandboxes on Windows — *with fail-closed
  refusal when a policy cannot be represented* (the Windows behavior the
  synthesis names as the precedent), plus a managed network proxy with
  deny-wins domain policy and structured audit events. This is the single
  capability no Effect codebase in the catalog even attempts.
- **A production-grade generated protocol.** Codex generates version-matched
  TypeScript and JSON Schema from Rust source (601 generated schema files at
  the audited revision, drift-tested), proving Rust can be the *source* of a
  typed seam that TypeScript clients consume.
- **Terminal/PTY rigor.** Grok Build treats the terminal as a hostile foreign
  host: emulator-backed PTY tests against the real binary, resize storms,
  leader clusters, crash-mode restoration, a p99 frame-time regression gate,
  and a native-scrollback renderer that commits finalized blocks exactly once.
  ~25,000 test attributes back it.
- **Single static binaries and a managed leader lifecycle.** One binary serving
  TUI, headless, and ACP clients; the Grok leader's socket/lock discovery,
  version eviction, and coordinated update-relaunch protocol; Codex's atomic
  versioned installer with rollback slots.
- **No-GC latency floors.** The frame-time and PTY work presumes a runtime
  without GC pauses; both codebases treat p95/p99 latency as contract.

**Cost** (same documents, their own words):

- **Accretion at staggering scale.** Codex: 125 crates, ~1.13M lines of Rust,
  a 92-entry feature registry including removed flags that must parse forever,
  V1/V2 protocol surfaces coexisting, parallel legacy/new permission concepts,
  and a repository contract that explicitly begs contributors to stop growing
  `codex-core`. Grok: 79 crates, ~1.35M lines, individual composition modules
  approaching ten thousand lines. Both teardowns close with the same warning:
  *adapt the boundaries, not the accumulated compatibility burden*.
- **Iteration speed.** The compatibility residue is the visible symptom of a
  deeper cost: in a fast-moving product category, the Rust cores paid for
  their guarantees with migration layers they cannot delete. Codex still
  carries two SDK generations, two collaboration-tool generations, and legacy
  config projections.
- **Maturity ≠ safety.** Grok Build is the sharpest counterexample to "Rust
  means secure": a magnificently engineered Rust codebase that ships
  sandbox-off-by-default, warn-and-continue degradation, a no-op macOS network
  restriction, fail-open hooks, and plaintext credential files. The language
  bought performance and control; it did not buy the trust posture. Trust is
  an architecture decision, not a toolchain decision.

## 5. What the Effect codebases actually bought — and what it cost

**Bought** (evidence: OpenCode V2/Effect teardowns, T3 Code teardown):

- **Durable admission as the coordination gold standard.** OpenCode V2 is the
  catalog's strongest reference for exactly the property the specs commit to
  hardest (mobile SM-4, desktop AC-1/AC-18): every input durably recorded
  with a client-chosen idempotent ID and causal parent *before* scheduling,
  promotion at safe boundaries, exact-retry reconciliation, explicit
  steer/queue verbs, a refusal to persist a "running" status a crash would
  turn into a lie, and three named read surfaces. This was built by a small
  team in a fraction of the Rust codebases' line count.
- **Schema-first contracts as package law.** One canonical Schema identity per
  public value, browser-safe contract packages below the engine, Promise and
  Effect clients generated from one HttpApi, and an embedded SDK that installs
  a memory-backed fetch into the *same router* so in-process callers can never
  bypass policy. "Networked and embedded are the same application" is the
  single most important property for a one-engine/many-clients product, and
  Effect made it nearly free.
- **Interruption as control flow.** User decline becomes interruption with
  guaranteed finalizer cleanup, never a fabricated tool failure; tool fibers
  owned in sets; uninterruptible masks only around state settlement. This maps
  one-to-one onto the specs' honesty requirements (no fabricated completion,
  honest transient gaps).
- **The service graph is testable architecture.** Scope-owned registrations,
  graph-aware replacement in tests, deterministic TestClock time for leases,
  retries, and expiry — the verification fabric W13 demands. OpenCode's
  dependency-graph tests are executable architecture contracts.
- **A control plane at scale, in one language.** T3 Code is independent
  confirmation: a full event-sourced CQRS core (commands, pure deciders,
  typed events with causation/correlation IDs, projection tables, reactor
  workers, a receipt bus tests await instead of polling) orchestrating *five
  foreign harnesses*, shipping desktop + web + a real mobile app off one
  hand-written Effect RPC contract, with ~531k lines typechecking under the
  native-preview compiler. One language across engine, clients, and tests is
  not hypothetical — it is shipping in a direct competitor.
- **TypeScript-at-scale has an existence proof for distribution too.** Claude
  Code ships as a single Bun-compiled executable with aggressive dead-code
  elimination, staged lazy startup, prompt-prefix cache discipline,
  virtualized terminal history, and screen diffing. A TS engine can be fast
  and single-binary.

**Cost** (same documents):

- **Beta-framework churn.** Every audited Effect adopter pins a pre-1.0
  Effect 4 beta with unstable HTTP/SQL/RPC modules; T3 goes further and
  *patches the framework itself* and stacks a native-preview compiler and a
  pre-1.0 toolchain on top. Load-bearing pre-1.0 dependencies are a real,
  recurring tax.
- **Runtime bridges and ambient-context scar tissue.** OpenCode V1's history
  is a catalog of context-loss bugs from `AsyncLocalStorage` fallbacks and
  multiple managed runtimes; V2 had to build a custom Layer-graph compiler to
  escape it. Effect rewards discipline and punishes ambient shortcuts.
- **Performance ceilings at specific seams.** Claude Code's own perf
  strategies (virtualization, diffing, byte prefilters) are compensations for
  a GC runtime driving a terminal; none of the Effect codebases attempts
  Grok-grade p99 PTY gates. T3's transcript perf work is real but the frame
  budget game is harder in JS. For W6/W7/W8 the ceiling is genuine.
- **Plugin-in-process risk.** OpenCode runs third-party plugins as trusted
  code inside the authority process; Scope guarantees cleanup, not isolation.
  A TS kernel makes "just import it" dangerously easy; the refusal list must
  be enforced structurally.
- **No containment story.** No Effect codebase in the catalog compiles
  policy to OS enforcement. T3 — the best Effect control plane — defaults to
  `danger-full-access` with approvals off. The trust half of the product
  cannot be built *in* TypeScript alone, because its enforcement substrate is
  the kernel's, not the runtime's.

## 6. The agent-authorship consideration

Most future code in this system will be written by coding agents, and this is
not a neutral fact — it is a first-order architectural input.

- **Corpus and feedback loops favor TypeScript/Effect for coordination code.**
  Models are strongest where training data is deepest and the
  edit-typecheck-test loop is fastest. An Effect kernel gives an agent a
  single language across engine, contracts, clients, tests, and UI
  projections; a schema change propagates through generated clients and
  fixture drift tests in one loop. T3's practice — vendoring framework source
  with instructions that agents read it before writing Effect code, and
  encoding architecture law as custom lint rules rather than prose — shows how
  an Effect codebase is made *mechanically legible* to agent authors.
- **Rust's compiler is a stricter reviewer, and that cuts both ways.** The
  borrow checker catches whole bug classes agents introduce, but agent
  iteration on a 125-crate workspace with dual build systems is slow, and the
  observed failure mode of large Rust codebases under sustained velocity is
  exactly the accretion Codex warns about: compatibility flags, parallel
  generations, modules nobody dares delete. Agents amplify accretion because
  they add rather than restructure.
- **Blast radius argues for small Rust, not no Rust.** A process-opaque native
  helper a few thousand lines long, behind a frozen schema'd contract, is the
  ideal agent-authored Rust artifact: reviewable in one sitting, testable
  against fixtures, and incapable of corrupting kernel state because it holds
  none. A million-line Rust core is the worst: too large to review, too
  entangled to regenerate, and its `unsafe` and policy surfaces demand exactly
  the sustained human scrutiny an agent-heavy workflow underprovides.
- **Review-ability is the trust product applied to itself.** A system whose
  thesis is receipts and legibility should bias toward the stack where its
  own agents' output is most legible to its own oracles. Today that is
  Effect/TypeScript for coordination, with Rust confined to seams where the
  contract is small enough that the *contract*, not the implementation, is
  what gets reviewed.

## 7. The recommendation

**Keep the application kernel — the engine that owns conversations, admission,
policy decisions, receipts, orchestration, projections, and every client
contract — in Effect. Adopt Rust deliberately and narrowly, as process-opaque
native helpers behind schema-defined IPC/stdio contracts, for the workloads
where the OS or the clock is the counterparty.**

The bright-line decision rule, applied per workload:

> **If the workload's correctness is defined by typed coordination — who may
> do what, in what order, surviving which failures — it belongs in the Effect
> kernel. If its correctness is defined by what the operating system enforces
> or by sub-frame/sub-buffer latency, it belongs in a Rust helper process that
> holds no authority, speaks a frozen schema'd contract, and whose absence
> fails closed. Nothing is ever FFI-linked into the kernel process: no native
> module may share the kernel's address space, hold its secrets, or learn its
> command/state authority.**

Three corollaries:

1. **Helpers are opaque and authority-free.** A Rust helper receives narrow
   typed requests and returns narrow typed results (plus receipts). It never
   holds provider credentials, never owns conversation or policy state, and
   never gets a general command channel. If the helper dies, the kernel
   degrades honestly (the containment helper's absence refuses the run —
   fail closed; the media helper's absence disables voice — degrade).
2. **The kernel is the only conversation and policy owner.** Exactly the
   Codex end-state lesson (the TUI became a client of its own engine) and the
   OpenCode lesson (embedded must not bypass): every transport — IPC, socket,
   relay, in-process — enters one request processor. Rust helpers are *below*
   that processor, never beside it.
3. **The never-list.** Never: a Rust rewrite of the engine loop; Rust holding
   the event log or admission authority; N-API/FFI natives linked into the
   kernel for "speed"; a second protocol defined in Rust that TypeScript
   mirrors by hand (one side generates, the other consumes); third-party
   native plugins in any trusted process.

## 8. Workload-to-language table

| # | Workload class | Language | Rationale (evidence) |
| --- | --- | --- | --- |
| W1 | Typed protocol / engine seam | **Effect** (Schema is the source of truth; generate all clients) | One canonical schema identity per value, generated Promise/Effect clients, drift tests (OpenCode V2). Codex proves generation works from Rust too, but the clients, tests, and UI are TS — put the source where the consumers are. |
| W2 | Durable admission / event sourcing | **Effect** | The two best implementations in the catalog (OpenCode V2 admission, T3 CQRS core) are both Effect; durability here is transactional discipline over SQLite/Postgres, not performance. |
| W3 | Policy / authority / receipts | **Effect** (decision) + **Rust** (enforcement, via W5) | The authority *compiler* and manifest/receipt emission are typed coordination; the *enforcement* it compiles to is OS work. Split exactly at Codex's approval-vs-containment seam. |
| W4 | Provider adapters / Full Auto / fleets | **Effect** | Subprocess JSON-RPC, ACP, SDK streams, rotation policy, guidance loop — all typed orchestration under partial failure; T3 proves five harnesses behind one Effect core. |
| W5 | OS containment (Seatbelt/Landlock-seccomp/restricted tokens, egress proxy) | **Rust helper** | No JS runtime can express this; Codex's sandbox crates and network proxy are the reference. Helper compiles a profile document → OS policy, applies it to spawned work, emits the execution receipt. Fail closed on absence (desktop AC-3). |
| W6 | PTY / terminal hosting | **Rust helper** | Grok's PTY/frame-time rigor is unreachable on a GC runtime at the p99 gate the specs demand for transcript surfaces (desktop AC-8); the transcript *projection* stays typed and kernel-owned. |
| W7 | Media / audio | **Rust helper** | Device I/O, resampling, bounded buffers, jitter — classic native real-time; contract stays schema'd, cancellation prompt. |
| W8 | Local inference | **Rust (or vendored native) helper** | Apple-silicon inference is a native ecosystem; expose it as one more provider lane behind the same adapter seam as W4 — the router must not know it is local. |
| W9 | Sync / relay / E2EE | **Effect** for protocol/state; **Rust only if** a crypto or throughput seam demands it | Codex's remote-control envelope (seq/ACK/cursor/bounded segmentation) is protocol design, not syscalls; libsodium-class crypto is available to both stacks. Start Effect; the reversal test is measured, not aesthetic. |
| W10 | Signed distribution / update appliers | **Effect** for release pipeline + manifest tooling; **Rust/native** for the applier binaries | The applier that atomically swaps a running install (retained slots, no-downgrade, drain) should be a tiny static binary with no runtime deps — Codex's installer and Grok's updater are the mechanics; Ed25519 verification is trivial in either. |
| W11 | Web/API surfaces | **Effect** | Same schemas, same request processor, receipts rendered not produced; nothing native here. |
| W12 | Renderer/UI | TypeScript (settled elsewhere) | Noted for gravity only: the UI consumes typed projections, which is one more reason the projection producer should share its language. |
| W13 | Tests / oracles / contracts | **Effect** (plus per-helper native test rigs) | Graph replacement + TestClock determinism is the Effect payoff; each Rust helper carries its own Grok-style emulator/fixture rig, and the *kernel-side* contract tests treat the helper as a black box. |

Rough proportionality: by surface area this is perhaps 85–90% Effect, 10–15%
Rust — but the Rust slice carries a disproportionate share of the trust
story's enforcement, which is precisely why it must stay small enough to
audit.

## 9. Boundary discipline: how the two sides talk

- **Types are owned once, in Effect Schema.** Every helper contract (requests,
  results, receipts, error taxonomy) is defined in a schema package; JSON
  Schema fixtures are generated from it; the Rust side consumes generated
  types or validates against the fixtures in its own CI. Drift between kernel
  and helper is a build failure, not a runtime surprise. (This inverts Codex's
  direction — Rust generating TS — because here the kernel and all clients are
  TS; the generation arrow should point from the majority consumer's source
  of truth.)
- **Transport is boring on purpose.** Newline-delimited JSON or
  length-prefixed frames over stdio/Unix socket, with the Grok leader's
  lessons applied: per-generation identity, protocol+binary version in the
  handshake, bounded frames, bounded queues with typed overload errors, and
  authenticated local clients — never ambient socket-directory trust.
- **Helper lifecycle is product state.** Version, process generation,
  readiness, capability set, and update/drain state are queryable and
  rendered, exactly as the Grok leader and Codex daemon expose theirs. A
  helper is supervised by the kernel; it never supervises the kernel.
- **Receipts cross the boundary as data.** The containment helper returns the
  effective-enforcement record that becomes the execution receipt; the kernel
  signs/stores it. Helpers report; the kernel attests.
- **Contract tests on both sides of every seam.** Kernel-side: fixture-driven
  black-box tests against a real helper binary (and a schema-faithful fake for
  CI speed). Helper-side: OS-policy tests per platform (Codex's
  sandbox-policy test discipline), PTY emulator tests (Grok's harness), with
  results retained as receipts.

## 10. Sequencing implications

1. **Freeze W1+W2 in Effect first.** Protocol kernel, durable admission,
   three read surfaces, one request processor. Everything else consumes this;
   the catalog is unanimous it is the highest-leverage layer, and it is pure
   Effect strength.
2. **Ship W3's honest half before W5 exists.** Emit authority manifests and
   execution receipts that truthfully record *no containment* before the
   containment helper is built. The receipt recording the absence of
   enforcement is the trust product working; a green shield ahead of
   enforcement would be the fraud the product exists to eliminate.
3. **Adapt, don't rebuild, from the Rust references for W5/W6/W10.** The
   Seatbelt/bubblewrap policy-compilation shape, the egress proxy's deny-wins
   semantics, the PTY harness architecture, and the atomic
   updater-with-rollback mechanics are all Apache/MIT-licensed designs in the
   teardown set. Port the designs into small owned helpers; do not vendor the
   megacores.
4. **W4 grows in Effect as adapters, one provider to closure before breadth.**
   The eleven-predicate closure bar in the synthesis applies per lane.
5. **W7/W8 wait for their contracts.** Voice and local inference are named
   future contracts in the specs; their helpers should not exist before their
   product specs do.

## 11. Reversal tests

Each assignment is falsifiable. Flip it if:

- **Kernel Effect → Rust** if, after real virtualization and projection
  discipline, the kernel demonstrably cannot hold the checked-in p95
  frame/latency baselines (desktop AC-8) or the recovery SLO (SM-4 99.5%) on
  representative hardware — measured, with the Claude-Code-style perf
  techniques exhausted first — or if Effect 4's post-beta churn forces a
  second load-bearing framework patch in one year (T3's patched-beta posture
  is a warning sign, not a model).
- **Containment Rust → anything else** never flips to TS, but flips *shape*
  if the OS vendors ship policy APIs a supervised process can apply to
  children without a native compiler — then the helper shrinks to a shim.
- **PTY Rust → Effect** if an emulator-backed test matrix shows a JS PTY host
  meeting the same p99 gates under resize storms and large-output floods on
  all supported platforms. (Grok's rigs make this a runnable experiment, not
  an opinion.)
- **Sync/relay Effect → Rust** if measured relay throughput/latency or E2EE
  handshake cost on owned infrastructure exceeds what the Node runtime
  sustains at target fleet size — a load-test verdict, not a vibe.
- **Update applier native → Effect** if a runtime-dependent applier ever
  proves it can survive the "update the thing that is running me" problem
  across all six targets with rollback; until then the tiny static binary
  stands.
- **The whole split** reverses toward more Rust if the agent-authorship
  premise inverts — i.e., if agent competence in large-scale Rust (including
  refactoring and deletion, not just addition) observably surpasses TS on
  this team's own review-and-oracle metrics.

## 12. Risks

- **Dual-toolchain tax.** Two build systems, two CI matrices, two dependency
  audit surfaces. Contained by keeping helper count small (target: countable
  on one hand), contracts frozen, and per-helper scope written down with a
  reversal test — every helper must justify its existence against the
  bright line, forever.
- **Drift.** The classic two-language failure is the hand-mirrored type. The
  generate-and-fixture discipline in §9 is not optional; the moment a helper
  contract is edited by hand on both sides, the split has failed.
- **Hiring and agent competence.** The kernel stays in the stack where both
  the team's tooling and agent authorship are strongest; the Rust surface is
  deliberately small enough that a single systems-competent reviewer (human
  or specialized agent lane) can own all of it.
- **The rewrite temptation.** The Rust references are seductive: they are the
  best engines in the catalog, and every performance incident will invite
  "port the kernel." The catalog's own evidence is the antidote — Codex's
  92-flag registry and Grok's 1.35M lines show where a Rust core under
  product velocity lands, and both teardowns say adapt the boundaries, not
  the burden. Conversely, resist the opposite temptation: do not let the
  kernel's convenience creep into the helpers' domain (a "temporary" JS
  sandbox shim or in-process native module is how fail-open happens).
- **Beta exposure.** The Effect bet is a pre-1.0 bet today. Mitigate with
  upgrade gates (contract, startup, resource-finalization regressions), no
  framework patches without an exit plan, and the kernel/helper seam itself —
  which conveniently caps how much of the system any framework migration can
  touch.

## 13. Conclusion

The product described by the vision docs is a coordination-and-proof machine
with a thin native rind. The teardown evidence assigns the coordination-and-
proof core to Effect decisively: the best durable-admission, schema-contract,
interruption, and control-plane implementations in the audited market are
Effect codebases, built at a fraction of the Rust engines' mass, in the
language every client, test, and future agent author already speaks. The same
evidence assigns the rind to Rust just as decisively: compiled containment,
PTY rigor, real-time media, static appliers — the places where the counterparty
is the kernel or the clock — have no credible JS implementation anywhere in the
catalog.

So the ideal split is not a compromise between two camps; it is the seam both
camps' evidence points at. An Effect application kernel that owns every
contract, every decision, and every receipt; a handful of small, opaque,
authority-free Rust helpers behind frozen schema'd contracts, each one doing a
job the OS respects and the kernel attests; and a standing rule that nothing
native ever links into the kernel's address space. Keep the Rust surface small
enough to audit, the Effect surface disciplined enough to trust, and let the
receipts — not the language war — carry the argument.
