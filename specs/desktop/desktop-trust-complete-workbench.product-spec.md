---
spec_format_version: "0.1"
title: "OpenAgents Desktop: Trust-Complete Coding Workbench"
artifact_type: "prd"
spec_revision: 7
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-19T00:00:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-desktop/"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_source_synthesis: "docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md"
  openagents_source_transcripts: "docs/transcripts/200.md through docs/transcripts/255.md plus the episode-256 draft transcript (Full Auto Implementation Audit and Roadmap)"
  openagents_admission_status: "roadmap-reconciled by docs/sol/MASTER_ROADMAP.md revision 119 as surface vision and target intent; implementation dispatch remains limited to live issues and exact accepted plans/work packets, with owner gates and proof rungs intact"
  openagents_revision_3_note: "Rev 3 elevates Full Auto to the roadmap-defining flagship per the episode-256 draft transcript (owner: an AFK-reliable multi-account multi-provider autonomous mode is 'important enough of an unserved need to build the whole short-term roadmap of the company around'). Full Auto becomes a dedicated run mode launched beside New Session (lightning action, one-time objective/workspace/provider-policy setup, full-screen read-only run view, explicit Play/Pause/Stop, no composer while running — 'steering is not Full Auto'); adds automatic model/account/provider failover on limits (the overnight Fable-limit halt is the named incumbent failure), same-thread cross-provider handoff with end-to-end acceptance proof (the live Claude-error -> Codex-continues-same-chat moment), active-run thread retention (the five-thread cache eviction root cause), and the run -> bounded report -> transcript analysis -> replayable fixture iteration loop. Also folds in back-catalog direction from episodes 200-237: the episode-206 Guidance Module lineage (between-turn decision engine with shared state/budget/environment, confidence-gated actions, hard guardrails over soft guidance, budget-bound autonomy); episode-237 clearing-layer doctrine (scoped-in-advance done, rubric grading, dereferenceable receipts, draft/verified/reviewed/bonded confidence tiers); episode-225 account-visibility peeves (at-a-glance identity, rate-limit burn-down, API-key fallback); episode-204 don't-break-userspace; episode-227 one-click full data export; the one-app cockpit/earning thesis deferred to its own contract."
  openagents_revision_4_note: "Rev 4 makes complete Cursor-class capability parity explicit rather than assuming trust advantages compensate for missing breadth. It binds the exhaustive parity ledger, restores editor depth, computer use, voice, automation, remote/background, marketplace, light/high-contrast themes, and agent-first/classic workbench outcomes to scope, and requires an unbundled harness/model/provider/placement/index architecture with inspectable local and remote data lifecycle."
  openagents_revision_5_note: "Rev 5 incorporates MemoHarness as the optimization layer for the already-unbundled harness plane. Desktop owns released six-dimension HarnessPolicyBundle selection, run-start adaptation from a frozen private experience-bank snapshot, selected-versus-effective provenance, adaptation receipts, private experience-bank lifecycle controls, and candidate/shadow/production release states. It forbids per-turn self-modification, current-run evaluation leakage, authority expansion, raw-memory projection to mobile/web, and candidate self-promotion. The Effect application/control plane owns schemas, retrieval, optimization, policy, storage, and release; Rust remains an isolated native-helper boundary for containment, PTY, and local-inference primitives only."
  openagents_revision_6_note: "Rev 6 turns the prior editor capability nouns into a Zed-quality integrated IDE contract. One generation-fenced Effect project graph now binds multi-root worktrees, documents, language, Git, terminals/tasks/debug, agents, proposals, evidence, local/remote placement, recovery, mobile/web continuation, and public code-share projections. Monaco, Pierre, xterm, and focused VS Code packages remain replaceable mechanics. Effect/TypeScript owns every identity, policy, state machine, database, projection, and receipt; Rust is limited to process-opaque authority-free PTY/containment and benchmark-admitted native kernels behind generated contracts and reversal tests. Adds AC-39 through AC-47 and SM-17 through SM-18."
  openagents_revision_7_note: "Rev 7 binds the canonical docs/ide/ROADMAP.md contract and makes its previously implicit first-editor decisions executable intent: Tokyo Night is the one initial Desktop IDE/workbench theme; built-in, persistent, off-by-default Vim is a first-party capability rather than an extension; broader light/high-contrast/system theme parity remains required before full parity but does not block the daily-use basic-IDE rung. Every boundary DTO is Effect Schema-first, types derive from schemas, capabilities use Context.Service and Layer.effect, public/non-trivial methods use named Effect.fn, typed failures use Schema.TaggedErrorClass, and process/watch/stream lifetimes are scoped. Adds AC-48 through AC-52 and SM-19 through SM-22 while preserving IDE-00..19 as roadmap sequencing rather than claiming implementation."
  openagents_ide_architecture: "docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md"
  openagents_ide_roadmap: "docs/ide/ROADMAP.md (IDE-00 through IDE-19; sequencing and release-rung vocabulary only)"
  openagents_ide_spec_crosswalk: "specs/IDE_ROADMAP_CROSSWALK.md"
  openagents_assurance_companion: "specs/desktop/desktop-trust-complete-workbench.assurance-spec.md (proposed; no admission or execution authority)"
  openagents_sibling_specs: "specs/openagents/cursor-capability-parity.product-spec.md, specs/mobile/mobile-any-host-fleet-controller.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md"
---

## Problem

Developers who supervise coding agents now get supervision breadth from every
competitor — parallel worktree agents, diff review, terminals, remote control
— but no product gives them trust. The full teardown catalog (ChatGPT/Codex
desktop, Claude desktop, Claude Code, Cursor, Factory, Amp, Grok Build,
OpenCode, T3 Code, OpenChamber, Command Code) shows the same holes everywhere:
permission dialogs presented as containment while sandboxes are off by
default. "Completed" claimed without delivery states. No record of what
authority a run was granted versus what enforcement actually ran. Unsigned or
fail-open release chains. Agent topology that is fully retained on disk yet
almost invisible in the UI.

And no product survives the owner walking away. The founder's own AFK test is
the sharpest statement of the unserved need: with a week of delegable work and
a baby due any day, none of the major harnesses can be trusted to keep going
for a day or two unattended. An overnight run with ample total usage halted
after an hour at a single model's limit instead of dropping down a tier
automatically — "why could you not do that automatically?" The incumbent loop
modes (`/loop`, `/goal`, cron) are "brittle to the harness: you could run out
of usage credits, you cannot connect multiple accounts, you cannot connect
multiple providers." A user with one Claude account, three ChatGPT accounts, a
Grok account, and a Cursor account has no system that connects all of them
and routes between them. The first OpenAgents Full Auto attempt failed the
same test for a different reason: the active run's thread was evicted from a
bounded cache and the next continuation addressed a conversation the host
could no longer open. The dogfood episodes record the rest of the defect
classes that make delegation untrustworthy: convincing green summaries
("according to whom?"), a UI chip naming a model that did not run, invented
launch commands, queued messages replaying in loops, composer state leaking
across chats, ambiguous running binaries, hotkeys that reshuffle under muscle
memory, reasoning hidden behind opaque status verbs. "Get the fuck out of the
terminal. Give me an app… Make it clicky."

## Hypothesis

If OpenAgents Desktop combines the market-converged supervision shape — one
typed engine seam, a workbench that deepens beyond chat, worktree-parallel
agents, a fully rendered live agent tree — with a legible trust layer that no
competitor ships (named execution profiles compiled to OS enforcement,
per-run authority manifests paired with execution receipts, a delivery
lifecycle distinct from completion, effective-identity truth, evidence-gated
status, a signed end-to-end release chain), presents it as a fast, clicky,
StarCraft-like operator surface, and crowns it with Full Auto — a dedicated
press-play-and-walk-away run mode that connects every account and provider
the user has and keeps working reliably through limits, errors, and restarts
for days — then developers will (a) choose it over broader-but-untrusted
competitors, (b) delegate materially more unattended work through it, and
(c) complete consequential work from ProductSpec intent to evidence-backed
acceptance without falling back to another agent interface, because the
surface makes delegation inspectable instead of faith-based and makes walking
away rational instead of reckless.

The breadth bar is now explicit: a Cursor user must not need Cursor for any
maintained Cursor-class daily workflow. OpenAgents reaches that breadth through
replaceable harness, model, placement, indexing, and extension planes rather
than recreating Cursor's closed bundle or cloud-canonical custody.

## Scope

```productspec-scope
in:
  - Satisfy `specs/openagents/cursor-capability-parity.product-spec.md` as the exhaustive breadth contract: every current Cursor capability is present, admitted, or visibly tracked as a gap, and no trust advantage excuses a missing user outcome.
  - Ship two complete densities over one canonical state graph: a classic coding workbench with editor, files, symbols, search, diagnostics, source control, diff, terminal, preview, extensions, settings, themes, and keymaps; and an agent-first operations window optimized for concurrent sessions, plans, worktrees, subagents, attention, and review.
  - Meet a Zed-quality integrated-IDE bar rather than assembling adjacent widgets: one generation-fenced `IdeProjectRef` graph owns multi-root projects, roots, files, documents, capability lifecycles, worktrees, language results, Git evidence, terminals/tasks/debug, agent attachments, proposals, persistence, and safe cross-surface projections; every tree, editor, search, symbol, diagnostic, hunk, test, terminal, and agent backlink resolves through that graph.
  - Bind the complete IDE outcome to the canonical `docs/ide/ROADMAP.md` packet and release-rung vocabulary without turning that roadmap into product or dispatch authority: Files foundation, daily-use basic IDE, agent IDE, portable IDE platform, and full parity remain visibly different claims, and every unopened IDE-00..19 dependency remains an explicit gap.
  - Keep file opening editor-first and provider-independent: Finder/Open With, Explorer activation, quick open, workspace search, Problems, symbols, Git hunks, recent restore, and agent backlinks target the primary main-region Monaco document immediately, while chat, provider, Git, index, LSP, and remote hydration continue asynchronously; refusal or unsupported content renders a typed editor state in that same region, never a small side pane or no-op.
  - Deliver complete document behavior: stable opaque document identity, multi-view editor groups and splits, preview/pin/reorder/reopen, multi-cursor and native editing, find/replace and navigation, encoding/BOM/EOL/large/binary/read-only truth, atomic save/save-all, explicit autosave, external-change conflict, dirty close, hot-exit recovery, and generation-bound stale refusal or an explicit rebase flow.
  - Ship Vim as an app-owned, first-party editor capability that is built in, packaged offline, off by default, and toggleable from Settings, the command palette, and the editor status control. Persist `editor.vim.enabled`; project NORMAL/INSERT/VISUAL/VISUAL LINE/VISUAL BLOCK/REPLACE/operator-pending state where the selected engine supports it; route `:write`, `:quit`, undo/redo, clipboard, search, and every mutation through the canonical command/document graph; and prove key-precedence, IME, accessibility, split/worktree isolation, conflict, restart, disable, and teardown behavior without a trusted extension host.
  - Ship Tokyo Night as the only initial Desktop IDE/workbench theme through one owned `DesktopThemeProjection` spanning Effect Native/app chrome, Monaco, Pierre tree/diffs, xterm, Problems/Output/debug, review, agent, browser, and status surfaces. Pin palette provenance and semantic-token mappings, apply accessibility adjustments explicitly, initialize before first editor paint, and prohibit executable theme code. Light, high-contrast dark/light, and system-following modes remain required at the complete-accessibility/parity rung rather than blocking the first useful editor.
  - Treat language intelligence as a project capability with visible lifecycle and placement. Monaco-local workers, tsserver, and LSP may provide different evidence tiers, but diagnostics, completion, hover, definitions, references, document/workspace symbols, rename, formatting, code actions, semantic tokens, inlay hints, folds, Problems, Outline, breadcrumbs, and excerpt views carry exact document/service generations, cancellation, supersession, restart, and unsupported/degraded truth.
  - Make terminal, tasks, tests, output, and debug first-class project evidence: xterm is the screen projection; named tasks, problem matchers, test discovery/results, DAP breakpoints/launch/attach/stack/variables, logs, and terminal sessions bind exact project/worktree/runtime generations and become agent-usable evidence under declared retention and disclosure policy.
  - Make the agent/code loop native to the IDE: a session attaches to an exact project without gaining implicit authority; the context tray discloses selected files/ranges/symbols/diagnostics/changes/rules/skills/retrieval reasons and provider destination; agents produce version-bound proposals rather than mutating Monaco; Pierre review supports safe file/hunk accept/reject/apply/undo; code and turns backlink; post-apply diagnostics/tests/format/Git/delivery attach as evidence.
  - Keep local and remote projects behind the same typed capability interface and stable project/file identities. Effective placement, component version, attachment generation, latency class, cached-versus-live evidence, disconnection, revocation, incompatibility, and recovery are explicit; Desktop never silently uploads a project, downloads a helper, changes custody, or substitutes a managed capability.
  - Use Effect/TypeScript as the authoritative IDE and control plane: it owns project/document/language/Git/terminal/task/debug/agent identities, commands, policy, admission, persistence, recovery, placement, projections, export/delete, and receipts; Monaco, Pierre, xterm, LSP/tsserver/DAP/Git/harness processes, and focused VS Code packages remain supervised mechanics behind Effect-owned services.
  - Define every persisted, IPC, wire, helper, mobile/web, public-share, and other boundary value once with Effect Schema (`Schema.Struct`, `Schema.TaggedStruct`, or `Schema.TaggedUnion`) and derive TypeScript types from that schema. Use constrained branded schemas for scalar refs and stable schema identifiers for codegen-facing contracts; raw interfaces or handwritten unions cannot become parallel wire or persistence authority, while internal-only state machines may use `Data.TaggedEnum`.
  - Model application capabilities with `Context.Service`, compose implementations with `Layer.effect`, wrap public and non-trivial operations in named `Effect.fn`, model domain failures with `Schema.TaggedErrorClass`, decode all untrusted input at the boundary, and scope/interrupt project watchers, language servers, terminal children, streams, and subscriptions with their owning layer.
  - Limit Rust to separately admitted process-opaque helpers for OS containment, PTY/process-group byte mechanics, optional local inference, and only benchmark-proven native kernels. Helpers receive generated versioned bounded contracts, no project/session/policy/credential/database/receipt authority, fail closed for required enforcement, degrade honestly when optional, and carry explicit necessity and reversal tests; file/index/search/Git/LSP/DAP remain Effect/Node or supervised external-process paths by default.
  - Match Cursor-class AI editing and repository intelligence: low-latency completion and next-edit prediction, inline ask/edit/generate, multi-file apply/review/undo, explicit file/symbol/path/docs/web context, and hybrid local lexical/symbol/path search plus selectable local or remote semantic embeddings with visible scope, freshness, custody, cost, export, rebuild, and deletion.
  - Unbundle workbench/editor, harness runtime, model/provider, execution placement, sync/relay, and persistence/indexing so each can be replaced independently without changing canonical session identity; support owner-local, owner-managed, OpenAgents-managed, and compatible audited-provider execution where the selected capability permits it.
  - Make MemoHarness a first-class optimization layer over that replaceable harness plane: a released, content-addressed `HarnessPolicyBundle` independently versions its six dimension policies (context assembly, tool interaction, generation control, orchestration, memory management, and output processing) and declares compatibility with engine protocol, provider/model/toolset, execution profile, and evaluator versions; experience construction, global optimization, pattern extraction/selection, and per-case adaptation remain separate typed pipeline components around the bundle.
  - Bind each run to both a selected base harness and an observed effective harness. An admitted run-start adaptation may retrieve only from a frozen, scope-filtered experience-bank snapshot, apply only already-released bounded module patches, emit a `HarnessAdaptationReceipt`, and freeze the resulting effective bundle for the entire run; no continuation silently changes policy beneath an active run.
  - Treat the experience bank as private evidence with two explicit layers: append-only `HarnessExecutionExperience` records compiled only after terminal runs, and `HarnessPatternCandidate` records derived offline whose released forms may become adaptation inputs. Desktop shows scope, provenance, retention, retrieval/training eligibility, export, deletion, tombstone, and release state for both layers without presenting raw transcripts, secrets, or provider tool output as ordinary memory.
  - Run global optimization asynchronously against admitted evaluation sets to produce candidate module versions. Candidate, shadow/dogfood, released, active, rejected, and rolled-back states are visibly distinct; no optimizer, run, or executor may promote its own candidate, and production selection resolves only released compatible versions through the Blueprint release gate.
  - Keep learning outside action authority: a harness adaptation may change prompt/policy modules and bounded runtime formatting, but never workspace grants, execution placement, provider/account admission, tool scopes, approval policy, guardrails, budgets, done conditions, release authority, or any external-effect permission.
  - Cover Cursor-class agent and ecosystem breadth: ask, plan, execute, review/debug, design, and custom modes; browser and receipted computer use; side chats and conversation search; checkpoints and compaction; CLI and protocol clients; remote/background agents; schedules and event-triggered automations; skills, MCP, rules, hooks, plugins, extensions, team bundles, and signed marketplace discovery under isolation.
  - Make local and remote state inspectable: inventory chats, object graphs, search indexes, embeddings, manifests, checkpoints, worktrees, terminal/browser state, extension state, auth material, telemetry, and caches with per-category location, size, retention, sync, export, reset, revocation, and verified deletion.
  - Consume one generated, versioned engine protocol with the hierarchy Thread -> Turn -> Item, extended with Work Unit (task identity plus delivery lifecycle), Authority Manifest, Execution Receipt, and Delivery Receipt; the renderer holds projections only and never conversation, tool, Git, or PTY authority.
  - Record every input durably before scheduling (client-chosen idempotent IDs, causal parent, typed delivery intent), and expose steer, queue, and interrupt as three explicit verbs with typed composer admission states; queue never silently becomes steer.
  - Key composer draft, queue, attachments, and stop/steer targeting per thread; every user action resolves unambiguously to exactly one thread, a dispatched queue item leaves the queue, and queued items are visible, editable, and cancellable before promotion.
  - Keep attachment capability invariant across composer states: images and files attach identically on a first message, a follow-up, a steer, and a queued input.
  - Provide three read surfaces per session as contract: bounded current projections, a durable replayable per-aggregate log with a replay-to-live marker, and a volatile live stream documented as lossy; reconnect is repair with honest transient-gap markers, never fabricated completion.
  - Survive restarts honestly: in-flight work resumes or terminates through typed recovery after an app restart, restart attribution is never fabricated, and the app always discloses which build, version, and working-tree source is running.
  - Ship a hardened stock Electron shell: locked fuses, sandboxed renderers, schema-decoded and sender-validated IPC on every channel, partitioned sessions for artifact, preview, terminal, and browser surfaces, deny-by-default permission and navigation handlers, and a locally versioned renderer (never a live site).
  - Work local-first with no account: the local workroom requires no OpenAgents account, uses the user's existing provider logins through named isolated accounts (never default provider homes), and keeps data local until the user explicitly opts into Sync or network capabilities.
  - Ship the StarCraft-like operator surface: clicky, fast, game-feel interaction; stable user-bindable hotkeys that never reshuffle underneath muscle memory (pinned chat command-groups, Harpoon-style); dense data made actionable with game UX; and a noob/pro dual mode — an opinionated out-of-the-box experience for newcomers with full power-user controls underneath.
  - Render reasoning and activity visibly by default: reasoning expanded in the main trace rather than hidden behind status verbs, honest tool-activity descriptions, usage shown inline in the main view, and no raw UUIDs in user-facing UI.
  - Display effective execution identity from observed execution events, never inferred from the selected brand: requested versus effective model are distinct typed facts on every message, no substituted output is ever streamed under the requested model's name, and provider rotation happens only on account or session failure before content.
  - Ship a right-panel workbench surface manager hosting review-summary, diff, files, file, terminal, plan, and preview tabs, each a projection over engine state with no renderer authority.
  - Virtualize the transcript with a turn navigator and a three-level message hierarchy (prose primary, compact work rows, exact evidence on disclosure); checked-in performance baselines gate merges on transcript surfaces.
  - Render the complete agent tree: a canonical persisted agent graph with live per-child lifecycle, causal inline child cards at the exact spawn point showing the child's latest activity, one-gesture navigation into each child's full independent transcript, named roster entries in the right rail, and explicit orphan or gap nodes; the same typed projection renders at every surface density with no capability tier.
  - Make ProductSpec the native intent and execution unit: guided conversational authoring of a valid spec, validation at the relevant section with exact revision, digest, and criterion IDs, a reviewable plan derived from acceptance criteria, durable criterion-addressed work packets that agents execute, and evidence-backed completion — with admission, evidence production, verification, owner acceptance, and release as distinct steps that no executor self-grades.
  - Host the resident Assurance layer: the app speaks both ProductSpec and AssuranceSpec, renders proof-design state honestly (proposed, needs-design, not-run — never a hardcoded second source of truth), and treats the named false-green failure modes (fixture asserts, API mirrors, mocked seams, coverage theater, rounded-up summaries) as detectable defects.
  - Maintain a UX Behavior Contract registry: every owner- or customer-stated UX expectation lands verbatim in a typed registry with oracle tests enforced in the normal sweep, contract deviations found in the wild are strict bugs filed with the contract ID, and in-app violation reporting feeds the fix loop.
  - Ship Full Auto as a dedicated run mode, not a composer preference: launched from its own action beside New Session (a lightning-bolt affordance), configured once with an explicit objective, granted workspace, and provider/account routing policy, then presented as a full-screen read-only run view with explicit Play, Pause, and Stop and no ordinary composer while running — press play and walk away; steering a running Full Auto is a later, separately admitted contract, because steering is not Full Auto.
  - Model Full Auto as a durable run-state machine: play and pause are typed, restart-survivable states owned by the main process; every continuation decision is re-derived from durable state at turn completion and at startup; caps, typed failure policy with bounded backoff, and a working stop are contract, not configuration.
  - Connect every account and provider the user has to one Full Auto routing policy: multiple Claude, ChatGPT/Codex, Grok, and Cursor accounts (and local model lanes) registered as named isolated accounts, with the system smartly routing work across them — usage-maxing the fleet within provider terms, own-capacity-only.
  - Never halt unattended work on a limit the routing policy can route around: when a model tier, account, or provider hits a usage limit or errors mid-run, Full Auto automatically continues on the next admitted model, account, or provider lane, records the rotation in the run receipt, and surfaces the honest provider condition — a run stops only for owner stop, cap, objective completion, or typed policy block, never to wait for a human to acknowledge a limit.
  - Support same-thread cross-provider handoff as a first-class capability: a thread can switch provider lanes mid-conversation, the next provider receives bounded host-owned history, and the capability is proven by end-to-end acceptance tests in which provider A writes a recognizable fact and provider B continues the same visible thread using it — with named, sidebar-visible handoff test runs so the proof is inspectable in the product, not only in CI.
  - Guarantee active-run thread retention: a thread bound to an active Full Auto run is never evicted from host caches or rendered unopenable; cache policy must exempt active runs, and a continuation that addresses an unopenable thread is a typed defect class with an owner-visible reason, never a generic "conversation no longer exists."
  - Run the Full Auto iteration loop as product machinery: every run produces a bounded run report (objective, turns, dispositions, rotations, failures, evidence links); run transcripts are analyzable for improvement; and any failed run is reproducible as a replayable fixture run.
  - Carry the Guidance Module lineage in Full Auto's decision layer: a between-turn decision engine with a shared model of goal, state, budget, and environment across turns, confidence-gated next actions (low confidence waits or consults rather than acting), and hard deterministic guardrails that override soft guidance — budget-bound autonomy as the operating contract.
  - Keep account truth at a glance: the logged-in identity for every connected account visible without digging, per-account rate-limit burn-down rendered inline (the ticking usage counter), and optional API-key fallback lanes when subscription quotas exhaust.
  - Consume FastFollow as a standing work source: owner-configured upstream study, content-addressed study packets, gap assessments, and evidence-only candidate work feeding Full Auto when the issue backlog runs dry — never as a second authority mode; implementation still requires separate admission.
  - Route one-interface consolidation through Khala: the user's own subscriptions and accounts fanned out behind semantic (never keyword) routing, own-capacity-only, with per-request routing disclosure of the effective backend.
  - Pursue local-inference fallback: a local model lane on Apple silicon for cheap operations and offline continuity, so losing the network degrades capability instead of killing the surface.
  - Provide file checkpoints independent of Git with conflict-aware, three-mode (conversation, code, both), staged two-phase rewind that discloses reversible versus irreversible effects before commit.
  - Treat worktrees as durable engine resources with outcome-sensitive lifecycle (auto-remove unchanged, retain changed, refuse dirty or unpushed) and cleanup receipts; render delivery states (changes_produced, reviewed, committed, pushed, merged, accepted) distinct from turn completion, and present confidence tiers (draft, verified, reviewed, bonded) as distinct visible states of a Work Unit.
  - Compile named execution profiles (projection-only, workspace-bounded, networked-build, isolated-guest, owner-local danger mode, managed cloud) to OS enforcement; owner-local danger mode is explicit and visually persistent, never a default; unavailable containment fails closed.
  - Provide a hermetic execution profile that excludes every ambient input unless explicitly admitted and emits a complete admitted-input manifest.
  - Drive palette, keyboard, menus, slash commands, and model-proposed actions from one central command registry of stable command IDs with typed schemas, capability requirements, approval flags, idempotency, and redaction class; there is no code path where prose becomes execution, and the app never surfaces an operational directive (command, script, path) that was not read from the system of record.
  - Ship the Fleet workspace: connected accounts and harnesses in one interface with capacity shown as quantities (available, busy, queued), readiness lights lit only from decoded fresh receipts ("no receipt means no light"), read-only account state on the fleet page, and explicit harness choice on new work.
  - Honor don't-break-userspace as a release law: once a workflow works for users it keeps working; startup predictability, hotkey stability, and existing UX contracts are regression-gated, and no update silently reshuffles a working surface.
  - Provide one-click complete data export: a user can export all local state (threads, specs, receipts, settings) and leave; no lock-in move ever ships.
  - Ship best-of-N and plan-first execution as typed fan-out with per-child receipts and an explicit comparison record, and review fan-out compiled into assurance manifests whose acceptance decision sits outside the reviewed party.
  - Provide a terminal renderer family (full-screen, native-scrollback, headless) over one typed transcript projection, gated by emulator-backed PTY test matrices and frame-time baselines.
  - Package for the full six-target matrix (macOS, Windows, Linux, x64 and arm64) under a signed release-set manifest, a component compatibility ledger across shell, engine, renderer, and extensions, retained-slot rollback, no downgrade flags, and coordinated drain-before-update of live engine work; macOS signed and notarized first.
  - Build in-app community feedback: bug reporting and contribution flows from inside the app, wired to the strict-bug and behavior-contract intake paths.
out:
  - The desktop app does not load a remotely deployed website as its renderer, and web deployment authority never becomes desktop code authority.
  - No browser-runtime fork or proprietary Chromium; capability ships through narrow typed native modules on stock Electron.
  - No terminal TUI as a product surface; the terminal is a workbench tool inside the app, not the app.
  - Steering a running Full Auto session is out of this revision: the first contract is hardcore press-play-and-walk-away; a steerable Full Auto is a later, separately admitted mode.
  - In-app rate-limit reset triggering is deliberately withheld until the surrounding reliability contracts are proven; accidentally consuming a reset at full quota is an unacceptable failure mode.
  - Voice remains independently specifiable and sequenceable, but Cursor-class voice input and bidirectional agent control are part of the parity target and cannot be permanently omitted from Desktop.
  - Multiplayer capacity contribution and the Pylon provider/earning mode (the Go Online button, wallet, and seed-derived Nostr/Lightning identity of the one-app cockpit thesis) are separate later contracts; contribution is off by default and there is no hidden background compute.
  - No ambient screen-recording or inferred-memory capability in this revision; any future ambient memory is a separate spec bound to private-by-construction custody.
  - No additional provider integrations before one provider is complete under the eleven-predicate closure bar (known, decoded, owned, retained, projected, presented, authorized, recovered, fast, receipted, shipped).
  - No third-party plugin execution inside the trusted engine or shell process; extensions run under declared isolation profiles with signatures and receipts or they do not run.
  - No ambient personal-memory product, cross-tenant experience retrieval, raw transcript synchronization for optimization, continuous per-turn self-rewriting harness, or automatic candidate promotion; MemoHarness is bounded run optimization over explicit evidence and release policy, not a new authority or surveillance mode.
cut:
  - CUT-DSK-01: Arbitrary unreviewed theme code is cut. Tokyo Night is the only initially supported IDE/workbench theme; first-party light, high-contrast dark/light, system-following, and accessible theme switching are still required before the complete-accessibility or full-parity gate, and portable declarative themes may later be admitted only through the extension-isolation path.
  - CUT-DSK-02: Simultaneous token-by-token mirroring of every child agent is cut; the roster shows live typed lifecycle and any child transcript opens on demand.
  - CUT-DSK-03: Forking and owning a proprietary editor runtime is cut; complete editing workflow parity is still required through replaceable, typed editor components.
  - CUT-DSK-04: Unsigned or trusted-process marketplace execution is cut; public discovery and signed ingestion of extensions, MCP/MCPB, skills, rules, hooks, and plugins with provenance, compatibility, permissions, isolation, rollback, and receipts are required.
  - CUT-DSK-05: Sub-brands are cut; the desktop app is "OpenAgents," not a separately branded client.
  - CUT-DSK-06: A chat box inside the Full Auto run view is cut; the run view is read-only with Play, Pause, and Stop only.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user sends a follow-up while a turn is running, the composer requires an explicit queue-or-steer choice, and the transcript later shows the input's admission, promotion, execution, and terminal states as distinct facts.
- id: AC-2
  criterion: When a user opens any run's detail view, they see both the authority manifest (what policy admitted) and the execution receipt (what containment actually enforced); requested and effective enforcement are never merged into one indicator.
- id: AC-3
  criterion: When a run is started under a profile whose OS enforcement cannot be represented on the current platform, the run refuses to start with a typed reason instead of degrading silently.
- id: AC-4
  criterion: When a session spawns child agents, the agent tree shows every retained child with live lifecycle state and latest activity, an in-flight spawn is visible before it resolves, any child's full transcript opens within two interactions, and unlinked history renders as an explicit gap node.
- id: AC-5
  criterion: When a user rewinds to a prior turn, the app stages the restore, discloses reversible versus irreversible effects and externally modified files, and only applies on explicit commit, emitting a rewind receipt.
- id: AC-6
  criterion: When agent work in a worktree finishes, unchanged worktrees are auto-removed, changed worktrees are retained, dirty or unpushed worktrees are refused for cleanup, and each outcome emits a cleanup receipt; the Work Unit's delivery state is visible and distinct from turn completion.
- id: AC-7
  criterion: When the packaged app is inspected by release tests, Electron fuse oracles pass (RunAsNode, NODE_OPTIONS, and inspect disabled; ASAR integrity on), and every IPC channel rejects messages that fail schema decoding or sender validation.
- id: AC-8
  criterion: When a transcript holds ten thousand or more items, scrolling and turn navigation stay within the checked-in p95 frame-time baselines, and those baselines gate release.
- id: AC-9
  criterion: When an update is offered, the client verifies the signed release-set manifest against the pinned key before install, refuses version downgrades, retains a rollback slot proven by test, and drains live engine work before relaunch.
- id: AC-10
  criterion: When a run executes under the hermetic profile, the emitted manifest lists every admitted input source, and no ambient instruction, hook, plugin, or memory outside that manifest influenced the run.
- id: AC-11
  criterion: When the same command ID is invoked from the palette, a keyboard shortcut, a menu, and a model-proposed action, all four paths produce identical typed outcome records.
- id: AC-12
  criterion: When a user binds a chat or command to a hotkey, that binding never reshuffles as chats are created, reordered, or closed; the bound target stays stable until the user rebinds it.
- id: AC-13
  criterion: When a user switches between chats mid-composition, draft text, queued inputs, attachments, and stop/steer controls each remain keyed to their own thread; no composer or queue state leaks across chats, and a dispatched queue item never replays.
- id: AC-14
  criterion: When any assistant message renders, its metadata shows the effective model, provider, and account from observed execution events; a turn whose effective model differs from the requested one is displayed as such and is never streamed under the requested model's name.
- id: AC-15
  criterion: When the Fleet workspace shows a readiness or status light, that light derives from a decoded fresh receipt; absent or stale evidence renders as unknown, never as green.
- id: AC-16
  criterion: When an agent turn produces reasoning, the reasoning is visible in the main trace expanded by default, tool activity is described by what actually ran, and current usage is visible inline without leaving the main view.
- id: AC-17
  criterion: When a user authors a ProductSpec in the workroom and accepts its plan, the resulting work packets each retain exact spec revision, criterion ID, and terminal evidence links, and no packet is displayed as completed without its matching terminal outcome and review post-image.
- id: AC-18
  criterion: When the app restarts with work in flight, each affected session either resumes through typed recovery or terminates with an owner-visible typed reason; restart attribution is never invented, and the app discloses the running build, version, and source on demand.
- id: AC-19
  criterion: When a selected account is exhausted or rate-limited, the surfaced error names the real provider condition, and where another connected account is ready, work fails over to it under the recorded execution profile with the rotation visible in the receipt.
- id: AC-20
  criterion: When a user launches Full Auto from its dedicated action, setup asks once for objective, workspace, and provider/account routing policy, then presents a read-only run view with explicit Play, Pause, and Stop and no ordinary composer; pause, resume, and stop are durable typed transitions that survive an app restart.
- id: AC-21
  criterion: When a model tier, account, or provider hits a usage limit or fails during a Full Auto run, the run continues automatically on the next admitted model, account, or provider lane per the routing policy with the rotation recorded in the run receipt; the run never halts to await human acknowledgment of a limit, and it terminates only by owner stop, cap, objective completion, or typed policy block.
- id: AC-22
  criterion: When a thread's provider lane is switched mid-conversation, the next provider receives bounded host-owned history and continues in the same visible thread, and an end-to-end acceptance test proves a recognizable fact written by provider A is used by provider B in that same thread, with named handoff test runs visible in the sidebar.
- id: AC-23
  criterion: When host caches face pressure, a thread bound to an active Full Auto run is never evicted or rendered unopenable; a continuation that addresses an unopenable thread surfaces as a typed defect with an owner-visible reason, never a generic conversation-not-found error.
- id: AC-24
  criterion: When a Full Auto run ends for any reason, it produces a bounded run report covering objective, turns, dispositions, provider/account rotations, failures, and evidence links, and any failed run can be reproduced as a replayable fixture run.
- id: AC-25
  criterion: When the Cursor parity corpus runs, every required row in `specs/openagents/cursor-capability-parity.product-spec.md` maps to a Desktop command, route, adapter, or explicit cross-surface continuation plus current acceptance evidence; no supported workflow silently disappears between the classic workbench and agent-first window.
- id: AC-26
  criterion: When a user performs Cursor-class editing work, completion, next-edit prediction, inline generation, multi-file apply, review, accept/reject/undo, semantic context, diagnostics, Git, terminal, preview, settings, keymaps, and first-party theme switching operate in one project without requiring another editor.
- id: AC-27
  criterion: When a user launches browser automation or computer use, the admission view states the exact browser partition, OS/network scope, secrets policy, and approvals; each action is receipted, and unavailable enforcement fails closed without removing the supported workflow.
- id: AC-28
  criterion: When a user configures a session, they can select compatible harness, model/provider/account, execution placement, sync posture, and indexing backend independently; the resulting session keeps one identity and reports selected and effective values plus all data flows.
- id: AC-29
  criterion: When a user inspects storage for a repository, Desktop enumerates every local and remote data class, size, freshness, retention, and sync state and proves complete export, chat-preserving index reset, repository-knowledge deletion, account/device revocation, and full deletion with remote tombstone receipts where applicable.
- id: AC-30
  criterion: When extensions, plugins, skills, MCP servers, rules, hooks, or subagents are discovered, installed, imported, updated, disabled, or removed, Desktop shows provenance, permissions, compatibility, isolation, and rollback; no untrusted code executes in the shell or trusted engine process.
- id: AC-31
  criterion: When background agents or automations are launched from schedule, repository, issue/PR, webhook, or manual triggers, they may use an admitted local, owner-managed, or OpenAgents-managed placement, survive client closure, accept typed intervention, enforce caps and idempotency, and return reviewable outcomes and receipts.
- id: AC-32
  criterion: When a clean Cursor profile is migrated, supported settings, keybindings, rules, skills, and MCP configuration import through an allowlist, credentials and proprietary state do not, and every item receives an imported, skipped, or rejected reason.
- id: AC-33
  criterion: When a user inspects or launches any run, Desktop identifies the selected base HarnessPolicyBundle and the observed effective bundle by immutable digest, displays each of the six dimension-policy refs and compatibility result, and never collapses requested and effective harness identity into one label.
- id: AC-34
  criterion: When run-start adaptation is enabled, Desktop shows the frozen experience-bank snapshot, scope filters, adaptation state, bounded released patches, and HarnessAdaptationReceipt one gesture away; the effective bundle is fixed before the first turn and remains byte-identical through continuations, restart, pause/resume, and provider handoff unless the run fails closed as incompatible.
- id: AC-35
  criterion: When a user inspects the experience bank, Desktop separately inventories execution experiences and released patterns with source-run provenance, visibility, retention, retrieval/training eligibility, size, export, deletion, and tombstone status; deleting a source makes it ineligible for future snapshots and preserves only the minimum non-content tombstone needed to prevent resurrection.
- id: AC-36
  criterion: When offline optimization produces a harness candidate, the UI distinguishes candidate, shadow/dogfood, released, active, rejected, and rolled-back states, presents held-out evaluation and compatibility evidence, and exposes no path by which the producing optimizer or run can self-verify or self-promote into production.
- id: AC-37
  criterion: When an adapted harness executes, its authority manifest is identical to the base run's authority manifest for workspace, placement, provider/account candidates, tools, approvals, guardrails, budgets, done condition, and external effects; any proposed delta outside the admitted harness-module schema refuses before dispatch with a typed reason.
- id: AC-38
  criterion: When Desktop projects MemoHarness state to mobile, web, exports, or public receipts, the projection is explicit-field allowlisted and may include safe digests, release state, compatibility, adaptation status, and redacted receipt refs, but never raw experiences, prompts, transcript text, tool output, embeddings, retrieval queries, private scores, secrets, or filesystem paths.
- id: AC-39
  criterion: When a supported source file is opened from Finder, Explorer, quick open, search, Problems, symbols, Git, recent restore, or an agent backlink, every route resolves the same current project/file/document identity and makes a real editable Monaco document primary in the main workspace before chat, provider, index, Git, LSP, or remote hydration completes; failure renders a typed editor state there rather than a side-pane fallback or no-op.
- id: AC-40
  criterion: When two projects or worktrees contain the same relative path, their Monaco models, dirty recovery, diagnostics, search/symbol results, Git evidence, terminals, tasks, breakpoints, agent context, proposals, and navigation history remain separated by exact project/root/attachment/document/service generations across rapid switching and restart.
- id: AC-41
  criterion: When a document is edited, viewed in splits, renamed, externally changed, deleted, revoked, saved, closed dirty, or recovered after forced termination, one Effect-owned document state reports encoding, EOL, disk revision, generation, dirty/conflict/recovery status and prevents silent overwrite; Monaco mechanics never become the only unsaved copy or filesystem authority.
- id: AC-42
  criterion: When Monaco-local, tsserver, LSP, task, test, or DAP capabilities start, stop, crash, reconnect, move, or return late results, the UI and agent tools show the exact available/degraded/unavailable evidence tier and effective placement, reject stale generations, honor cancellation and bounds, and never present a missing provider as a working feature.
- id: AC-43
  criterion: When an agent receives code context and proposes a single- or multi-file edit, the user can inspect the exact disclosure manifest and effective runtime, review version-bound changes in Pierre, accept or reject at supported granularity, apply or undo through canonical authority, and inspect post-apply diagnostics, tests, Git, and delivery facts; a changed base refuses or enters an explicit rebase flow rather than guessing line positions.
- id: AC-44
  criterion: When a terminal or task starts, Effect owns its project/worktree/session identity, environment and cwd admission, retention, commands, recovery policy, and receipts; xterm owns screen mechanics; a process-opaque Rust helper owns only PTY/process-group/resize/signal/byte mechanics; helper absence, incompatibility, crash, or stale generation produces a typed fail-closed or degraded result without leaking authority or replaying a mutating command.
- id: AC-45
  criterion: When the same project capability is fulfilled locally, on an owner-managed host, or on an admitted managed target, the command and result shapes stay identical while effective placement, version, latency, custody, freshness, and attachment generation remain visible; connectivity loss, revocation, or incompatibility never causes silent project upload, helper installation, placement change, or managed fallback.
- id: AC-46
  criterion: When Desktop hands a file, range, diagnostic, proposal, test, artifact, or run to mobile, web supervision, or a public share compiler, the projection uses opaque generation-bound safe refs and an explicit allowlist; Desktop reauthorizes every continuation, and no raw root, environment, credential, private context, terminal, embedding, or unselected repository content crosses the boundary.
- id: AC-47
  criterion: When an IDE implementation proposes Rust beyond PTY or required containment, its admission evidence names the missed cross-platform Effect/Node budget, optimized baseline, smallest authority-free protocol, failure behavior, generated conformance fixtures, and reversal threshold; without that evidence the capability remains in Effect/TypeScript or a supervised external process.
- id: AC-48
  criterion: When a user enables Vim from Settings, the command palette, or the editor status control, every Editor-mode Monaco view uses the same persistent off-by-default first-party setting; supported modes, motions, counts, operators, text objects, registers, repeat, search, supported colon commands, clipboard, undo/redo, and status are visible and operate through canonical commands, and restart, split, equal-relative-path worktree, conflict, IME, accessibility, disable, and listener-teardown journeys neither bypass document authority nor lose state.
- id: AC-49
  criterion: When any initially supported Desktop IDE surface mounts, one provenance-pinned Tokyo Night semantic projection controls app chrome, Monaco syntax and chrome, Pierre tree and diff, xterm, Problems, Output, debug, proposal/review, browser, and status colors before first paint; checked contrast and non-color cues pass, no executable or raw unreviewed theme code runs, theme initialization does not recreate canonical models or sessions, and no broader theme claim is made until the deferred light/high-contrast/system corpus passes.
- id: AC-50
  criterion: When an IDE contract crosses persistence, IPC, helper, renderer, mobile, web, or public-share boundaries, a contract audit finds one identified Effect Schema source using Struct, TaggedStruct, or TaggedUnion, derives its TypeScript type from that source, constrains scalar refs, and rejects untrusted input through the schema; the audit finds no raw interface or handwritten union acting as a parallel boundary contract.
- id: AC-51
  criterion: When a project, document, language, Git, terminal, task, debug, agent, projection, or storage capability starts and stops, its application service is a Context.Service implementation composed with Layer.effect; named Effect.fn operations expose Schema.TaggedErrorClass failures; watchers, processes, subscriptions, and streams are scoped to and interrupted with the owning project generation; and no renderer package or native helper becomes lifecycle authority.
- id: AC-52
  criterion: When a release or product surface describes IDE readiness, it names exactly one roadmap rung—Files foundation, daily-use basic IDE, agent IDE, portable IDE platform/parity candidate, or full parity—and the IDE-00..19 crosswalk shows current acceptance and assurance refs plus every remaining gap; Monaco, Pierre, Tokyo Night, Vim, or a Zed-quality architecture alone can never satisfy a broader rung or Cursor-parity claim.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: unattended_turn_share
  target: ">= 40% of desktop turns run in background or Full Auto modes without manual per-turn approval"
  target_status: provisional
  target_owner: "owner"
  window: within 60 days of general availability
- id: SM-2
  metric: median_concurrent_sessions_per_active_user
  target: ">= 3"
  target_status: provisional
  target_owner: "owner"
  window: within 60 days of general availability
- id: SM-3
  metric: receipt_or_manifest_inspections_per_weekly_active_user
  target: ">= 5 per week"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-4
  metric: forced_restart_session_recovery_rate
  target: ">= 99.5% of sessions resume intact with honest gap accounting"
  target_status: committed
  window: rolling 30 days from first packaged release
- id: SM-5
  metric: verified_auto_update_success_rate
  target: ">= 99% of update attempts verify, apply, and relaunch without user intervention"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days per release channel
- id: SM-6
  metric: first_launch_to_first_packet_activation
  target: "opted-in first launches create or open a valid spec, accept a plan, and start its first criterion within 15 minutes"
  target_status: committed
  window: rolling 30 days from workroom availability
- id: SM-7
  metric: no_fallback_task_completion
  target: "qualifying agent tasks reach one reviewed diff and terminal outcome without the user opening another agent interface"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days from workroom availability
- id: SM-8
  metric: second_durable_task_within_seven_days
  target: "activated developers start a second durably admitted task within seven days"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days from workroom availability
- id: SM-9
  metric: false_green_completion_incidents
  target: "zero confirmed incidents where the workroom showed completed without the matching terminal outcome and review post-image"
  target_status: committed
  window: rolling 30 days, continuously
- id: SM-10
  metric: full_auto_typed_termination_rate
  target: ">= 99% of Full Auto runs end by owner stop, cap, objective completion, or typed policy block — never by unhandled error or a limit awaiting human acknowledgment"
  target_status: committed
  window: rolling 30 days from Full Auto run-mode availability
- id: SM-11
  metric: multi_day_afk_dogfood_runs
  target: "at least one owner-AFK Full Auto run of 24-48 hours completes per release cycle with a reviewable run report and accepted work product"
  target_status: provisional
  target_owner: "owner"
  window: per release cycle from Full Auto run-mode availability
- id: SM-12
  metric: cursor_parity_desktop_rows_with_owner_accepted_evidence
  target: "100% before a full-parity claim"
  target_status: committed
  window: every release candidate
- id: SM-13
  metric: cursor_switcher_daily_workflows_requiring_another_editor_or_agent_client
  target: "0% across the maintained parity corpus"
  target_status: committed
  window: every release candidate and rolling 30-day dogfood
- id: SM-14
  metric: durable_data_classes_with_inspect_export_retention_and_verified_deletion_controls
  target: "100%"
  target_status: committed
  window: before a full-parity claim
- id: SM-15
  metric: runs_with_complete_selected_and_effective_harness_provenance
  target: "100%"
  target_status: committed
  window: every release candidate and rolling 30-day dogfood
- id: SM-16
  metric: memo_harness_self_promotion_or_authority_expansion_incidents
  target: "0"
  target_status: committed
  window: continuously
- id: SM-17
  metric: zed_quality_integrated_ide_journeys_completed_without_another_editor
  target: "100% across the maintained project/edit/language/git/terminal/task/debug/agent-review corpus before a Zed-quality or full-IDE claim"
  target_status: committed
  window: every release candidate and rolling 30-day dogfood
- id: SM-18
  metric: authoritative_ide_state_or_policy_classes_owned_by_rust_helpers_or_renderer_packages
  target: "0"
  target_status: committed
  window: continuously and every architecture/release audit
- id: SM-19
  metric: built_in_vim_acceptance_journeys_passing_without_document_or_command_divergence
  target: "100% across the maintained Vim-on, Vim-off, restart, split, conflict, IME, accessibility, and teardown corpus before the daily-use basic-IDE claim"
  target_status: committed
  window: every basic-IDE and later release candidate
- id: SM-20
  metric: initially_supported_ide_surfaces_rendered_from_the_owned_tokyo_night_semantic_projection
  target: "100%, with zero first-paint fallback-theme flashes and zero executable theme contributions"
  target_status: committed
  window: every packaged basic-IDE and later release candidate
- id: SM-21
  metric: boundary_contracts_with_a_single_effect_schema_source_and_derived_types
  target: "100%; zero raw interface or handwritten-union boundary authorities"
  target_status: committed
  window: continuously and every IDE architecture audit
- id: SM-22
  metric: ide_release_claims_with_exact_rung_complete_crosswalk_and_no_hidden_required_packet_gap
  target: "100%"
  target_status: committed
  window: every candidate, release, and public-copy review
```

## Solution

One long-lived local engine supervisor owns every conversation. Desktop is
its first and deepest client. The engine seam is a generated Effect Schema
protocol. All clients — Desktop renderer, terminal, mobile, web — consume the
same projections. Authority is compiled, not narrated: profiles become OS
enforcement, and every run leaves the manifest/receipt pair. Identity is
observed, not asserted: every message carries the effective model and account
that actually ran. Recovery is local-first: append-before-side-effect logs,
checkpoints, staged rewind, conservative worktree retention, restart-resilient
sessions with build disclosure. The workroom's native unit is the ProductSpec
— authored conversationally, decomposed into criterion-addressed packets,
executed by the fleet, closed only on evidence — with the Assurance layer
resident so green means what it claims.

Full Auto is the flagship built on that substrate: a dedicated run-state
machine (play/pause/stop, durable, restart-survivable) over the user's entire
connected fleet — every Claude, ChatGPT, Grok, and Cursor account plus local
lanes — with a between-turn decision layer in the Guidance Module lineage
(shared goal/state/budget/environment, confidence-gated actions, hard
guardrails over soft guidance), automatic rotation through limits, same-thread
cross-provider handoff with bounded host-owned history, active-run thread
retention, and a run-report/replayable-fixture iteration loop that turns every
failure into a regression. FastFollow keeps its work queue full. Distribution
is a signed transaction verified fail-closed on the client. The synthesis
essay, transcripts 200–255, and the episode-256 Full Auto audit record the
evidence and the owner's stated direction for each element.

MemoHarness is the learning loop around that flagship, not a second runtime.
The TypeScript/Effect control plane owns `HarnessPolicyBundle`, experience,
pattern, candidate, adaptation, receipt, retention, and release schemas.
semantic retrieval and offline optimization. Cloud SQL metadata. Private
Cloud Storage evidence. And Blueprint admission. Run start resolves a released
base bundle, freezes an eligible bank snapshot, optionally derives one bounded
effective bundle, and records the result before dispatch. A separate terminal-
run compiler may create a new experience only after the run can no longer be
influenced by it. Rust is reserved for isolated native containment, PTY, or
local-inference helpers behind generated Effect-owned contracts. It does not
own a parallel MemoHarness daemon, database, policy engine, or release path.

The IDE follows the same kernel-and-rind law. A scoped Effect
`IdeProjectService` composes project identity, worktree snapshots, documents,
search/index, language, Git, terminal/task/debug, agent context/proposals,
persistence, placement, and safe projection services. Monaco, Pierre, xterm,
focused VS Code protocol/language packages, Git, LSP/tsserver/DAP, and harnesses
are replaceable mechanics around that graph. A Rust child may hold an OS PTY or
compile/enforce a containment profile, but it cannot hold the project graph,
document journal, session store, policy, credential, approval, or receipt key.
Effect Schema is the source for any cross-language contract. Absence or drift
fails closed, and optional native acceleration must be removable without
changing canonical identities.

That service graph is schema-first rather than type-first. Boundary contracts
are `Schema.Struct`, `Schema.TaggedStruct`, or `Schema.TaggedUnion` values with
derived TypeScript types. Stable refs are branded schemas. Domain failures are
`Schema.TaggedErrorClass` values. Capabilities are `Context.Service`s composed
with `Layer.effect`, non-trivial operations are named `Effect.fn`s, and the
project scope owns interruption of watchers, language/debug processes,
terminals, streams, and listeners. `DesktopThemeProjection` supplies one
Tokyo Night semantic token plane to every initial IDE surface, while an
app-owned `VimModeController` translates a replaceable Monaco-compatible key
engine into the canonical command/document graph. Neither adapter receives
project, persistence, mutation, or policy authority.

## Strategic Positioning

Supervision breadth is converging table stakes across Cursor, T3 Code,
Factory, Amp, and OpenCode. "It is pretty easy to get to parity with the good
open source harnesses." The unclaimed lane is the trust half — explicit
authority, effective-containment truth, receipts, portable sessions, signed
provenance — plus the flagship no incumbent lab will build: reliable
AFK-grade Full Auto across all of a user's accounts and providers. The labs
assume a user "sitting there vibing". Their loops are brittle to their own
harness, halt at their own limits, and cannot span competitors' accounts.
OpenAgents routes across all of them — "they are not going to throw them all
into a super harness and route between them, but OpenAgents will" — and the
owner judges this "important enough of an unserved need to build the whole
short-term roadmap of the company around." The trust layer is what makes
walking away rational: run receipts, typed termination, and the clearing-
layer doctrine that the real product "is the receipt that proves the wiring
worked." The audience is deliberate: power users who like video games —
clicky, fast, dense, predictable — with an opinionated simple core for
newcomers. Do-not-break-userspace binds every release. One-click export means
no lock-in, ever. Everything ships open source. Being copied is accepted.

## Risks

- Trust surfaces could read as friction if receipts and manifests are not
  quiet by default. The design must keep them one gesture away, not in the
  path of every action.
- Fail-closed containment on platforms with weak sandbox primitives may block
  workflows competitors allow. The owner-local danger mode must remain an
  explicit, visible escape hatch without becoming a default.
- Full Auto is only trustworthy after the bug-bash defect classes (queue
  replay, composer leaks, restart ambiguity, active-thread eviction) are
  closed by contract. Shipping it earlier converts every defect into an
  unattended failure discovered a day later.
- Cross-provider handoff is currently plumbing-present, experience-unverified.
  claiming it before the end-to-end acceptance tests exist would be exactly
  the false-green failure the product exists to eliminate.
- Multi-account failover must stay within provider terms. Own-capacity-only
  and no-resale-of-subscription-inference are standing constraints.
- The six-target matrix and owned-runner release chain are operationally
  expensive. Sequencing must not let breadth starve depth.
- Rendering the full agent tree live at fleet scale has real performance
  risk. The perf-baseline gates exist to keep it honest.
- A shared experience bank can leak private run content or create cross-tenant
  influence if scope and retention are implicit. Eligible snapshots must be
  consented, tenant/workspace filtered, content-addressed, and deletion-aware.
- Adaptation can make evaluation meaningless if a run learns from its own
  outcome or changes policy between turns. The frozen pre-run snapshot and
  immutable effective bundle are release-blocking invariants.
- Optimization creates a false-green shortcut if candidates can self-promote.
  held-out evidence and an independent Blueprint release gate are mandatory.
- Monaco, Pierre, language servers, terminals, and agent adapters can create
  accidental parallel state owners unless every result is fenced by the one
  Effect project/document generation graph and every package remains behind an
  owned adapter.
- A premature “Rust backend” would duplicate the project and persistence
  planes, weaken Effect Schema authority, and make cross-surface behavior
  harder to prove. Native helpers therefore require empirical necessity,
  authority-free contracts, failure semantics, and reversal tests.
- Vim packages can bypass focus, command, dirty/conflict, accessibility, and
  teardown laws if treated as editor glue. The app-owned controller, canonical
  command translation, precedence table, and Vim-on/Vim-off corpus are the
  admission boundary.
- A fixed dark palette can masquerade as accessibility completion. Tokyo Night
  is the explicit initial product choice. Checked semantic contrast and
  non-color cues are immediate gates, while light, high-contrast, and system
  modes remain visible IDE-18/full-parity gaps until their own evidence passes.

## Open Questions

- Which two or three receipt kinds are surfaced most prominently in the
  default UI before users opt into the rest?
- Where exactly does the Full Auto launch affordance live — beside New
  Session in the sidebar, in the command palette, or both — and what does
  the run view show while paused?
- When steering of a running Full Auto eventually ships, is it a distinct
  "supervised run" mode rather than a composer inside the run view?
- Does the hermetic profile ship as a developer feature first or as the
  substrate for reproducible support bundles?
- Where do the cost-efficiency dials (speed versus token burn across
  connected accounts) live — per-run, per-spec, or global?
- When does the local-model lane graduate from fallback to a routable
  first-class capacity source?
- Which released adaptation policies are defaults for interactive sessions,
  Full Auto, and hermetic runs, and which stay explicit opt-in?
- What are the default retention and retrieval/training eligibility windows
  for private execution experiences before owner or enterprise policy changes
  them?

## Related Artifacts

- Canonical IDE implementation roadmap, packet definitions, release rungs,
  Vim contract, and Tokyo Night contract: `docs/ide/ROADMAP.md`
- IDE-02 complete Explorer implementation and evidence:
  `docs/ide/2026-07-19-ide-02-complete-pierre-explorer.md`
- Roadmap-to-ProductSpec/AssuranceSpec traceability:
  `specs/IDE_ROADMAP_CROSSWALK.md`
- Proposed proof-design companion bound to this exact revision:
  `specs/desktop/desktop-trust-complete-workbench.assurance-spec.md`
- Cursor parity contract and exact capability ledger:
  `specs/openagents/cursor-capability-parity.product-spec.md`
- Cursor product and local-state evidence:
  `docs/teardowns/2026-07-11-cursor-product-teardown.md`
- Canonical Zed-quality IDE and Effect/Rust split:
  `docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md`
- Zed integrated agent-IDE adaptation:
  `docs/ide/2026-07-18-zed-agent-ide-adaptation-analysis.md`
- Monaco/Pierre plan and VS Code package analysis:
  `docs/ide/2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md`
  and `docs/ide/2026-07-18-vscode-typescript-reuse-analysis.md`

- Roadmap reconciliation and AC-by-AC gap crosswalk:
  `docs/sol/MASTER_ROADMAP.md` revision 119 and
  `docs/fable/2026-07-17-surface-vision-gap-analysis-and-roadmap.md`
- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Full teardown catalog and evidence conventions: `docs/teardowns/README.md`
- Full Auto implementation authority: `specs/desktop/full-auto.product-spec.md`
  (the implementation-level Full Auto spec, this surface spec states the
  product vision above it) and
  `docs/fable/2026-07-17-full-auto-implementation-audit.md` (the corrected
  post-incident audit from the episode-256 session)
- MemoHarness architecture and Blueprint integration:
  `docs/research/2026-07-18-memoharness-paper-summary.md` and
  `docs/research/2026-07-18-memoharness-blueprint-integration-analysis.md`
- Transcript sources: `docs/transcripts/200.md`–`209.md` (Guidance Module
  lineage in `206.md`, identity/wallet in `207.md`, do-not-break-userspace in
  `204.md`), `docs/transcripts/214.md` + `225.md` (account visibility,
  hybrid routing, GUI-over-TUI), `docs/transcripts/228.md` (walk-away
  contract), `docs/transcripts/237.md` (clearing layer, confidence tiers),
  `docs/transcripts/244.md`–`255.md` (one-interface consolidation, UX
  contracts, workroom MVP, Assurance, bug bash, Full Auto/FastFollow), and
  the episode-256 draft transcript (Full Auto audit and roadmap)
- Sibling surface specs: `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`,
  `specs/web/openagents-com-trust-surface.product-spec.md`
- Desktop implementation-state authorities remain
  `apps/openagents-desktop/GUARANTEES.md` and `docs/sol/MASTER_ROADMAP.md`.

## Owner Gates

- Sign-off on the named execution-profile set and the visual treatment of
  owner-local danger mode.
- Sign-off on the Full Auto dedicated-mode UX (launch placement, read-only
  run view, play/pause/stop semantics) as the replacement for the composer
  toggle.
- The Full Auto release gate: owner confirmation that the queue, isolation,
  restart, thread-retention, and failover contracts hold — including at
  least one multi-day AFK dogfood run — before unattended operation is
  promoted in copy.
- Platform signing identities and the owned six-target runner fleet for the
  release matrix.
- Release channel policy (stable/RC identity split, rollback windows).
- In-app rate-limit reset triggering stays withheld until the owner
  explicitly admits it.
- Owner sign-off is required for default experience retention, retrieval and
  training eligibility, any cross-workspace pattern sharing, and promotion of
  a MemoHarness release policy from shadow/dogfood to production.
- MemoHarness's bounded, run-derived optimization bank does not admit ambient
  personal memory. Any future ambient-memory capability still requires a
  separate owner-admitted spec. Voice, multiplayer contribution, and the
  Pylon provider/earning mode ship as their own owner-admitted contracts.

## Receipts

Planned receipt kinds this surface emits or renders: execution receipts,
authority manifests, delivery receipts, worktree cleanup receipts, rewind
receipts, update/rollback receipts, hermetic admitted-input manifests,
fan-out comparison records, account/model/provider rotation records, Full
Auto run reports, handoff acceptance-test records, packet evidence links,
HarnessAdaptationReceipts, harness release/promotion/rollback receipts,
experience-compilation receipts, deletion tombstones, and assurance receipts
rendered from the resident proof layer. IDE-specific planned records include
the IDE-00 baseline, package/license and fallback decisions, Tokyo Night token/
contrast/visual evidence, Vim precedence/restart/teardown evidence, Schema and
service-lifecycle architecture audits, integrated packaged journey results,
and exact release-rung/capability-ledger closure. This section plans kinds. The
evidence ledger lives in the assurance and receipt systems, not in this spec.

## Promise Links

None yet. Public claims derived from this spec (trust layer, signed updates,
recovery guarantees, Full Auto AFK reliability, cross-provider handoff,
zero-false-green) must land in the promise registry with verification gates
before they appear in copy. UX Behavior Contracts carry the micro-scale
equivalents in the same discipline.
