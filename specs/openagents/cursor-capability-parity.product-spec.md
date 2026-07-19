---
spec_format_version: "0.1"
title: "OpenAgents Cursor-Class Capability Parity and Beyond"
artifact_type: "prd"
spec_revision: 3
author: "OpenAgents"
created_at: "2026-07-18T00:00:00.000Z"
updated_at: "2026-07-19T00:00:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-desktop/"
  - path: "apps/openagents-mobile/"
  - path: "apps/openagents.com/"
  - path: "packages/"
custom_sections:
  - id: "custom-cursor-capability-ledger"
    label: "Cursor Capability Ledger"
    after: "scope"
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
  openagents_primary_evidence: "docs/teardowns/2026-07-11-cursor-product-teardown.md"
  openagents_fast_follow_source: "FASTFOLLOW.md source-cursor and its exact pinned source revision"
  openagents_parity_rule: "Every user-observable Cursor capability needs an implemented, admitted, or explicitly owner-dispositioned OpenAgents row; trust advantages do not excuse capability loss."
  openagents_sibling_specs: "specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/mobile/mobile-any-host-fleet-controller.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md, specs/openagents/portable-coding-sessions.product-spec.md, specs/openagents/fast-follow.product-spec.md"
  openagents_revision_2_note: "Rev 2 defines full IDE parity as Zed-quality integration, not a checklist of adjacent Cursor features. It adds one generation-fenced project/document/language/Git/terminal/task/debug/agent evidence graph, editor-first cold open, version-bound proposals and backlinks, local/remote symmetry, and review-quality public code-share bundles. It fixes the implementation split: Effect/TypeScript owns all application authority and persistence; Rust is limited to process-opaque PTY/containment and benchmark-admitted native kernels behind generated contracts. Adds CP-AC-19 through CP-AC-23 and SM-7."
  openagents_revision_3_note: "Rev 3 binds the canonical docs/ide/ROADMAP.md closure contract. The daily-use editor starts with one owned Tokyo Night semantic theme and first-party persistent off-by-default Vim, while light/high-contrast/system themes remain mandatory before complete accessibility/full parity. Boundary contracts are Effect Schema-first and application lifecycles use Context.Service, Layer.effect, named Effect.fn, Schema.TaggedErrorClass, and scoped resources; Rust remains an authority-free rind. IDE-00..19 and release rungs become traceability vocabulary, not implementation or release claims. Adds CP-AC-24 through CP-AC-27 and SM-8 through SM-10."
  openagents_ide_architecture: "docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md"
  openagents_ide_roadmap: "docs/ide/ROADMAP.md (IDE-00 through IDE-19; sequencing and release-rung vocabulary only)"
  openagents_ide_spec_crosswalk: "specs/IDE_ROADMAP_CROSSWALK.md"
  openagents_assurance_companion: "specs/openagents/cursor-capability-parity.assurance-spec.md (proposed; no admission or execution authority)"
---

## Problem

Cursor is no longer merely an editor with autocomplete. Its installed product
is a complete agent platform: a classic IDE and an agent-first window, inline
generation and semantic code intelligence, plan and agent modes, parallel
worktrees and subagents, browser and computer use, local checkpoints, cloud
and background agents, automations, remote control, mobile and web clients, a
CLI and agent protocol, skills, plugins, MCP, hooks, rules, team controls, and
large amounts of durable local and remotely indexed state.

OpenAgents already has stronger architectural commitments around authority,
receipts, effective model identity, owner custody, portable sessions, and
replaceable providers. Those commitments are not a substitute for product
breadth. If a user must reopen Cursor for an ordinary Cursor workflow, then
OpenAgents has not reached parity no matter how much safer the narrower path
is. Conversely, copying Cursor's closed bundle, cloud dependency, opaque
indexing, credential storage, or hidden data lifecycle would abandon the
reason OpenAgents should win.

The product therefore needs one explicit parity contract. Without it,
competitor findings remain prose, sibling surface specs silently disagree
about what is in scope, and “better architecture” can become an excuse for a
missing capability rather than the way the capability is delivered.

## Hypothesis

If OpenAgents supplies every user-observable Cursor capability through a
single typed product system, while unbundling the editor, harness, model,
provider, execution placement, sync, and storage choices and making local,
owner-managed, OpenAgents-managed, and compatible vendor-hosted placement
explicit options, then Cursor users can switch without workflow loss and gain
capabilities Cursor cannot structurally provide: host-independent sessions,
honest model and usage identity, inspectable data custody, least-authority
execution, evidence-backed completion, complete export and deletion, and the
ability to choose the best model or harness without surrendering the rest of
the product.

## Scope

```productspec-scope
in:
  - Maintain an exact, release-gated Cursor capability ledger covering Desktop, mobile, web, CLI, remote, automation, extension, team, model, indexing, storage, security, and distribution surfaces; every observed capability has an evidence ref, target surface, implementation owner, acceptance ref, placement, data posture, and disposition.
  - Define parity as equivalent supported user outcomes and workflow depth, not copied code, identical visual design, or identical internal architecture; OpenAgents may exceed or safely reshape an interaction only when the original outcome remains available without material extra friction.
  - Ship both a complete coding workbench and an agent-first operations window, with one session and command system underneath rather than separate products or divergent transcript stores.
  - Define complete coding-workbench parity as a Zed-quality integrated project graph, not the presence of editor widgets: multi-root/worktree identity, editor-first file opening, revisioned documents and recovery, language/navigation, Git/review, terminal/tasks/tests/debug, agent context/proposals/backlinks, persistence, and local/remote capability lifecycle share exact generation-fenced identities and evidence.
  - Use `docs/ide/ROADMAP.md` IDE-00..19 and its release rungs as the maintained closure vocabulary: Files foundation, daily-use basic IDE, agent IDE, portable IDE platform/parity candidate, and full parity are distinct states, and no dependency, package, screenshot, architecture document, or agent report upgrades a row without its own current acceptance, assurance, and owner disposition.
  - Make built-in Vim and Tokyo Night part of the parity baseline rather than optional polish. Vim is a packaged first-party, persistent, off-by-default mode controlled through OpenAgents commands and document authority, not a trusted extension. Tokyo Night is the single initial semantic IDE/workbench projection; first-party light, high-contrast dark/light, and system-following themes remain required before complete accessibility/full parity but do not block the first useful basic-IDE rung.
  - Preserve the architecture split while reaching that breadth: Effect/TypeScript owns project, document, language, Git, terminal/task/debug, agent, command, policy, placement, persistence, recovery, export/delete, share, and receipt authority; Monaco, Pierre, xterm, focused VS Code packages, LSP/tsserver/DAP/Git/harness processes, and any Rust child are replaceable mechanics behind generated Effect-owned contracts.
  - Define every persisted, IPC, wire, helper, surface-projection, and public-share contract once with `Schema.Struct`, `Schema.TaggedStruct`, or `Schema.TaggedUnion`; derive its TypeScript type, constrain scalar refs with branded schemas, identify codegen-facing schemas, and reject raw interfaces or handwritten unions as parallel boundary authority. Application capabilities use `Context.Service` plus `Layer.effect`; named `Effect.fn` operations return `Schema.TaggedErrorClass` failures; untrusted inputs decode at entry; watchers, processes, streams, and subscriptions are scoped to their owning layer.
  - Admit Rust only as a process-opaque authority-free helper for PTY/process groups, OS containment/spawn, optional local inference, or a native kernel proven necessary by cross-platform benchmarks after TypeScript optimization. Every helper has bounded frames, generation fencing, failure semantics, conformance fixtures, and a reversal test; it never owns a project/session/policy/credential/database/receipt.
  - Match Cursor's editor intelligence, agent runtime, parallelism, context, browser/computer control, remote/background execution, automations, mobile/web supervision, CLI/protocol, ecosystem, enterprise administration, and platform distribution breadth described in the capability ledger below.
  - Unbundle six independently selectable planes: editor/workbench surface, agent harness, model/provider, execution placement, sync/relay, and persistence/indexing; no adapter may become the hidden owner of the canonical session, workspace, authority, or receipt graph.
  - Offer explicit execution placement per session or automation: owner-local, owner-managed remote host, OpenAgents-managed sandbox, or compatible audited provider; use of a managed server is optional except for capabilities whose selected placement inherently requires one, and the UI states that dependency before admission.
  - Preserve native harness behavior through typed adapters for Codex, Claude Code, Pi, Cursor-compatible or other admitted runtimes while projecting text, reasoning, tools, permissions, compaction, changes, usage, and session state into one OpenAgents protocol.
  - Keep model choice independent of harness choice; support connected subscriptions, API keys, OpenAI-compatible endpoints, local inference, policy routing, explicit model pins, and owner-approved automatic fallback with observed effective identity and usage on every turn.
  - Provide local-first repository understanding and optional remote semantic indexing with explicit corpus scope, ignore rules, freshness, embedding provider, retention, encryption, export, rebuild, and deletion controls; local lexical/path/symbol indexes remain useful when remote embeddings are disabled.
  - Inventory every durable local and remote datum the product creates, including chats, object graphs, search indexes, embeddings, snapshots, checkpoints, file manifests, terminal history, browser state, rules, skills, plugins, auth material, analytics, and caches, and expose per-category inspect, export, retention, reset, and deletion controls.
  - Preserve OpenAgents trust laws while reaching parity: typed commands, deny-by-default permissions, sandbox profiles, authority manifests, execution and delivery receipts, effective model truth, explicit network/data flows, no silent capability degradation, and evidence distinct from self-reported completion.
  - Match shareable review outcomes without cloud-canonical custody: authenticated supervision links and deliberate public `CodeShareBundle` links can expose an allowlisted tree subset, bounded syntax-highlighted excerpts, diffs/proposals/checkpoints/commits, Problems, tests/tasks, artifacts, bounded logs, agent causal links, effective runtime facts, and receipt refs with exact snapshot/live mode, audience, expiry, revocation, staleness, omission, export, and verification metadata; they expose no mutation authority or excluded host/private state.
  - Track current Cursor releases continuously through Fast Follow; a newly evidenced Cursor capability creates a dated parity row and gap assessment rather than silently changing implementation authority.
out:
  - No requirement to reproduce Cursor's proprietary source, branding, visual design, telemetry, cloud-canonical custody, opaque embedding service, credential format, or undocumented server behavior.
  - No claim that specification equals implementation, release, verification, or public availability; roadmap admission, work packets, assurance, owner acceptance, and release receipts remain separate authorities.
  - No requirement that every capability use OpenAgents-managed infrastructure; placement is a product choice constrained by the selected capability and declared data flow.
  - No weakening of sandbox, permission, provenance, extension isolation, release signing, or receipt laws merely to imitate a less constrained competitor path.
cut:
  - CUT-CP-01: Pixel-identical Cursor UI and binary-extension ABI compatibility are cut; workflow parity and supported open protocols are required.
  - CUT-CP-02: Hidden upload, undeclared remote indexing, silent model substitution, cloud-only canonical transcripts, and irrecoverable local credential storage are explicitly rejected even where Cursor uses them.
  - CUT-CP-03: A capability cannot be permanently cut only because OpenAgents has a stronger trust primitive; an owner-approved substitute must preserve the user's input, outcome, interoperability, and reasonable latency.
```

## Cursor Capability Ledger

The ledger below is the minimum completeness boundary, not an implementation
status claim. Each row expands into finer-grained target-owned work and proof.
The Cursor teardown and its pinned source snapshot are the evidence baseline.
Fast Follow keeps the rows current.

| Capability family           | Cursor-class outcome OpenAgents must cover                                                                                                                                                                      | OpenAgents form and advantage                                                                                                                                                                                                             |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Product shells              | Classic project IDE plus a dedicated agent-first multi-session window, project/repository opening, recent work, settings, updates, and account entry                                                            | One canonical session graph rendered as workbench, agents window, mobile, web, and terminal densities                                                                                                                                     |
| Editor core                 | Files, tabs/splits, search, symbols, navigation, diagnostics, source control, diffs, terminal, tasks/tests/debug, keymaps, themes, settings, recovery, remote workspaces, and extension-driven language support | Zed-quality generation-fenced Effect project graph with Monaco/Pierre/xterm as replaceable mechanics. Built-in persistent Vim and one initial owned Tokyo Night semantic projection. Light/high-contrast/system themes before full parity |
| AI editing                  | Low-latency single- and multi-line completion, next-edit prediction, inline ask/edit/generate, selection transforms, fast multi-file apply, review, accept, reject, and undo                                    | Model/provider-selectable editing with exact patch provenance, checkpoints, receipts, and local or managed execution                                                                                                                      |
| Repository intelligence     | Codebase chat, semantic search, file/symbol/path context, explicit mentions, docs/web context, ignore rules, and continuously refreshed repository knowledge                                                    | Hybrid local lexical/symbol/path index plus optional selectable embedding backends. Visible scope, freshness, cost, custody, export, and deletion                                                                                         |
| Conversation system         | Durable threads, side chats, branching, searchable past conversations, titles, attachments, mentions, reasoning, tool activity, usage, export, and deletion                                                     | Host-independent session identity, complete object graph, honest gaps, full-text search, portable custody, no cloud-only canonical history                                                                                                |
| Agent modes                 | Ask/read-only, plan, agent/execute, debug/review, design/front-end, and custom modes with mode-specific tools, models, permissions, and instructions                                                            | Typed execution profiles and reusable mode policies. Mode names never conceal model, authority, or placement                                                                                                                              |
| Agent tools                 | Workspace read/write/search, shell and PTY, Git and worktrees, diagnostics/tests, browser navigation, screenshots, web search, computer/OS use, image input, and artifact preview                               | Schema-decoded commands, scoped capabilities, sandbox enforcement, human approvals, and per-action receipts                                                                                                                               |
| Parallel agency             | Background shells, multiple concurrent sessions, subagents with independent transcripts, worktree isolation, plan fan-out, best-of-N generation, comparison, review, and merge                                  | Canonical agent graph, collision-safe claims, typed fan-out/fan-in, explicit comparison records, and acceptance outside the producing agent                                                                                               |
| Recovery and memory         | Checkpoints, code/chat rewind, crash recovery, compaction, summaries, rules, memories, repository instructions, and reusable context                                                                            | Two-phase conflict-aware restore. Declared admitted-input manifests. Inspectable memory sources and per-source disable/delete controls                                                                                                    |
| Background and cloud agents | Start work locally or remotely, continue after the client closes, monitor status, inspect diffs/logs/artifacts, intervene, and hand back to the workstation                                                     | Portable sessions with exclusive attachment generations across local, owner-managed, OpenAgents-managed, and audited-provider targets                                                                                                     |
| Automations                 | Scheduled and event-triggered agents, recurring jobs, repository/issue/PR/webhook triggers, isolated workspaces, budgets, notifications, result review, rerun, pause, and cancel                                | Durable Full Auto/automation state machines with typed triggers, caps, leases, receipts, optional placement, and provider/account failover                                                                                                |
| Remote control              | Continue and supervise work from web and mobile, answer questions, approve, steer, queue, stop, review changes, and receive notifications                                                                       | Any-host remote control with scoped revocable device grants, E2EE relay, exactly-once command outcomes, and no desktop credential on the phone                                                                                            |
| CLI and protocols           | Headless agent CLI, interactive terminal use, scripting/JSON output, session resume, CI use, and editor/agent protocol integration                                                                              | One typed protocol with full-screen, native-scrollback, and headless renderers. SDK and ACP/MCP-compatible bridges without a second authority path                                                                                        |
| Extensibility               | Extensions, marketplace/discovery, plugins, skills, MCP servers, rules, commands, hooks, subagent definitions, team bundles, import/export, enable/disable, and updates                                         | Signed provenance, permissions, isolation profiles, compatibility ledger, review, receipts, and support for open portable formats without trusted-process execution                                                                       |
| Browser and design          | Embedded browser, DOM/screenshot context, browser automation, preview/dev server, responsive inspection, visual editing, design-to-code, and image workflows                                                    | Partitioned browser/preview surfaces, explicit network profiles, receipted computer use, and optional specialized design harnesses                                                                                                        |
| Sharing and review links    | Share conversations, background-agent outcomes, changed code, artifacts, and review state with collaborators from web                                                                                           | Audience-scoped verifiable code-share bundles over selected tree/excerpt/diff/problem/test/artifact/agent evidence, with expiry/revocation and zero execution authority                                                                   |
| Models and accounts         | Broad first- and third-party model catalog, automatic/best routing, explicit model selection, multiple accounts, usage visibility, quotas, and team policy                                                      | Better-model-by-default without lock-in: harness/model separation, local lanes, BYOK/subscriptions, honest effective identity, policy routing, and failover                                                                               |
| Teams and enterprise        | Shared rules/plugins/skills, centralized settings, model/tool/network controls, privacy controls, SSO/SCIM-style identity, audit, billing/usage administration, and managed deployment                          | Typed policy compilation, organization-scoped capability grants, exportable audit receipts, self-host/owner-managed options, and no admin-policy ambiguity                                                                                |
| Security and privacy        | Sandbox, permission prompts, network controls, privacy modes, codebase indexing controls, secret handling, workspace trust, and update integrity                                                                | Requested-versus-effective enforcement, hermetic profiles, secret broker, explicit data-flow matrix, signed release sets, rollback, and fail-closed behavior                                                                              |
| Distribution                | Supported signed desktop builds and updates across macOS, Windows, and Linux on x64 and arm64 where platform support exists                                                                                     | Six-target release manifest, compatibility ledger, retained rollback slot, deterministic source/build disclosure, and no live-site desktop renderer                                                                                       |
| Data lifecycle              | Local databases, chats, search indexes, snapshots, checkpoints, caches, auth, telemetry, remote indexes, and server-side run state                                                                              | Human-readable data inventory, storage viewer, one-click complete export, selective/full deletion, verified remote tombstones, retention controls, and rebuild paths                                                                      |
| Quality and accessibility   | Fast startup, responsive transcripts/editor, reliable long-running agents, keyboard completeness, screen-reader support, reduced motion, high contrast, localization, and graceful offline behavior             | Checked-in performance/accessibility budgets and fault matrices. Capability state is available, degraded with reason, or unavailable—never silently missing                                                                               |

Every maintained ledger row must eventually include: `cursor_evidence_ref`,
`source_version`, `source_observation_date`, `target_surface`, `target_owner`,
`target_spec_revision`, `acceptance_refs`, `implementation_refs`,
`assurance_refs`, `placement_options`, `local_data_classes`,
`remote_data_classes`, `network_dependencies`, `disposition`, and
`freshness_state`. Allowed dispositions are `observed`, `gap`, `admitted`,
`implemented_unverified`, `verified`, `owner_accepted`, `superseded`, or
`owner_exception`. “Not needed” is not a disposition.

## User Experience

A Cursor user can install OpenAgents, open the same repository, import or
recreate their portable rules, skills, MCP servers, keybindings, and settings,
choose a familiar workbench or agent-first layout, and complete the same daily
flows without learning which internal service owns them. They can autocomplete
and edit in place, ask and plan, launch parallel agents in worktrees, browse and
test the result, compare candidates, run an automation, leave it running on a
chosen host, supervise it from mobile or web, and return to the same session.
The initial editor is deliberately opinionated rather than incomplete: Tokyo
Night is already coherent across chrome, source, tree, review, terminal, and
agent evidence, and Vim is a built-in toggle that survives restart without
requiring or trusting an extension marketplace. Broader theme choice remains a
visible later parity item rather than a hidden prerequisite for ordinary use.

Before any task starts, the placement disclosure answers: where the harness
runs, where the model runs, what repository material can leave the host, what
is indexed locally or remotely, what credentials are redeemable, what it will
cost, and what survives afterward. Defaults can be simple, but every plane can
be unbundled by an advanced user or organization without switching products.

The product includes a data inventory screen. It shows exact local paths or
logical stores, sizes, categories, last-write times, retention, sync state,
remote counterparts, and the consequences of deletion. A user can export all
data, delete one repository's knowledge, clear an embedding index while
keeping chats, revoke a device or provider account, or delete everything and
receive a deletion report. Rebuilding knowledge never requires deleting the
canonical session graph.

## Solution

One host-owned protocol separates surface, harness, model, placement, sync,
and persistence. Harness adapters translate native runtime events and resume
state. Provider adapters supply model calls. Sandbox providers supply isolated
filesystems and processes. The session service owns durable identity and the
canonical event graph. Projection services render that graph into Desktop,
web, mobile, terminal, and SDK shapes. Adapters can add native configuration
without taking ownership of authority or history.

The IDE implementation follows the kernel/rind split in the linked Zed-quality
architecture. A scoped Effect project service composes multi-root identity,
documents, search/index, language, Git, terminal/task/debug, agent context and
proposals, persistence, placement, and safe projections. Monaco, Pierre,
xterm, focused VS Code libraries, LSP/tsserver/DAP/Git, and harness processes
remain replaceable. Rust child processes may supply OS PTY or containment
facts, but never application state. Optional native acceleration enters only
after a measured cross-platform gate and can be removed without changing
canonical project or session identity.

The contract graph is schema-first: `Schema.Struct`, `Schema.TaggedStruct`, and
`Schema.TaggedUnion` own boundary shapes and their TypeScript types are derived.
branded schemas own stable refs. `Schema.TaggedErrorClass` owns typed failures.
`Context.Service` and `Layer.effect` own capability construction, named
`Effect.fn` operations own observable behavior, and project scopes interrupt
their watchers, processes, streams, and subscriptions. One
`DesktopThemeProjection` maps the pinned Tokyo Night palette into every
initial IDE adapter. One app-owned `VimModeController` translates a
replaceable Monaco Vim engine into the canonical command/document graph.

The repository knowledge service has explicit tiers: always-available local
path/text/symbol search. Optional local embeddings where a compatible model is
installed. Optional owner-managed or OpenAgents-managed semantic indexing.
and explicit third-party documentation/web retrieval. Content manifests,
chunk metadata, embeddings, caches, and remote index handles are distinct data
classes with independent retention and deletion. No UI label such as “indexed”
may collapse those facts.

The parity ledger is release input. A release may honestly label a row as a
gap, but it may not omit the row or claim full Cursor parity while any required
row lacks owner acceptance. Capability regression tests bind the ledger to
commands, routes, protocol messages, platform artifacts, and evidence so a UI
move or adapter replacement cannot silently remove a supported workflow.

## Acceptance Criteria

- **CP-AC-01:** The maintained capability ledger contains every family and
  required field named in this spec, pins the Cursor evidence/version/date,
  and has no missing, duplicate, stale-without-warning, or “not needed” row.
- **CP-AC-02:** A release claiming Cursor parity has no required row below
  `owner_accepted`. An owner exception names the preserved user outcome,
  approved substitute, expiry/review date, and evidence, and is never counted
  as parity until its substitute is accepted.
- **CP-AC-03:** One test repository can be opened in the classic workbench and
  agent-first window, and both project the same threads, drafts, queues,
  worktrees, terminal sessions, agent graph, changes, checkpoints, and
  effective execution identities without duplication or lost state.
- **CP-AC-04:** The editing corpus proves single-line and multi-line completion,
  next-edit prediction, inline generation/transformation, multi-file apply,
  accept/reject/undo, diagnostics, semantic repository context, and exact patch
  provenance under checked-in latency and correctness budgets.
- **CP-AC-05:** Ask, plan, execute, review/debug, design, and custom modes compile
  to explicit model, tool, permission, placement, memory, and instruction
  policies. Changing a mode cannot silently broaden authority or conceal the
  effective model.
- **CP-AC-06:** Parallel-session, subagent, worktree, background-shell, and
  best-of-N tests prove complete child transcripts, isolated mutation claims,
  collision detection, explicit comparison, deterministic fan-in, and an
  acceptance decision outside the producing agent.
- **CP-AC-07:** Browser, preview, screenshot, DOM, and computer-use tests prove
  partition isolation, declared network/OS authority, approval enforcement,
  secret redaction, and action receipts while preserving the supported Cursor-
  class workflow.
- **CP-AC-08:** Background and automation tests start from schedule, repository,
  issue/PR, and webhook/manual triggers. Run under local, owner-managed, and
  OpenAgents-managed placement. Survive client closure/restart. Enforce caps
  and idempotency. Notify and accept intervention. And produce reviewable
  outcomes without duplicate execution.
- **CP-AC-09:** The same portable session can start on Desktop, continue in a
  background placement, be supervised from web and mobile, and return to an
  owner host with one identity and at most one accepting attachment generation.
- **CP-AC-10:** The CLI, SDK, terminal UI, Desktop, web, and mobile invoke the
  same stable command IDs and observe the same durable outcomes. Scripting and
  JSON modes expose typed errors and never introduce a parallel authority path.
- **CP-AC-11:** Skills, MCP servers, rules, hooks, plugins, extensions, and
  subagent definitions support discover/install/import/export/update/disable/
  remove and team distribution, with provenance, compatibility, declared
  permissions, isolation, rollback, and receipts. Untrusted code never executes
  inside the trusted shell or engine process.
- **CP-AC-12:** For every session, the placement disclosure and receipt identify
  selected and effective harness, model/provider/account, execution target,
  sandbox profile, network policy, index/data flows, cost/usage, and retained
  artifacts. Automatic substitution is visible and policy-bound.
- **CP-AC-13:** With remote embeddings disabled, repository path/text/symbol
  search and agent context remain functional. Each optional local or remote
  semantic backend has an independent scope, freshness, rebuild, export,
  retention, and verified deletion path.
- **CP-AC-14:** The data inventory enumerates all local and remote data classes
  named in scope, reports size and last write, and proves selective repository-
  knowledge deletion, chat-preserving index reset, complete export, account/
  device revocation, full local reset, and remote deletion/tombstone receipts.
- **CP-AC-15:** macOS, Windows, and Linux release evidence covers supported x64
  and arm64 targets, signed update and rollback, startup, editor, agent,
  terminal, browser, extension, accessibility, and offline-degradation matrices.
  unsupported platform/capability pairs are visible gaps, not silent omissions.
- **CP-AC-16:** Fast Follow refresh detects a new or changed Cursor capability,
  records exact evidence and freshness, creates or updates the parity gap
  without granting mutation authority, and links any admitted implementation
  to target-owned acceptance and assurance refs.
- **CP-AC-17:** A migration dogfood imports supported portable settings, rules,
  skills, MCP configuration, and keybindings from a clean Cursor profile without
  importing credentials, opaque telemetry IDs, proprietary binaries, or hidden
  cloud state, and reports every unsupported item explicitly.
- **CP-AC-18:** Performance, accessibility, offline, crash/restart, revocation,
  provider failure, index corruption, and network-partition fault suites fail a
  parity claim when the equivalent workflow disappears, degrades silently, or
  fabricates completion.
- **CP-AC-19:** The integrated-IDE corpus opens the same file from Finder,
  Explorer, quick open, search, Problems, symbols, Git, and an agent backlink
  into one current Monaco document. Proves multi-root/worktree isolation,
  dirty/conflict/restart recovery, language/navigation, Git/review,
  terminal/tasks/tests/debug, and agent proposal/apply/undo without another
  editor. And rejects every stale attachment/document/service generation.
- **CP-AC-20:** A coding session can attach to an exact project, disclose a
  generation-bound context manifest, receive a version-bound multi-file
  proposal, review it in the same Changes/diff plane, apply or refuse it under
  current authority, rerun diagnostics/tests, and backlink between code and
  the creating turn without granting the harness direct editor authority.
- **CP-AC-21:** Local, owner-managed, and admitted managed project-capability
  journeys use identical command/result contracts and stable refs while
  exposing effective placement, version, latency, freshness, custody, and
  attachment generation. Disconnect, revocation, or incompatibility never
  silently uploads, installs, relocates, or substitutes a capability.
- **CP-AC-22:** A public or audience-scoped code-share journey compiles an
  allowlisted, verifiable bundle containing only selected tree/excerpt/diff/
  proposal/problem/test/artifact/log/agent/receipt evidence. Snapshot/live
  mode, omission, staleness, expiry, revocation, access, and public-copy risk
  are visible. And the page has no workspace, terminal, Git, model, or mutation
  authority and leaks no root, environment, credential, secret, private
  context, raw terminal, embedding, or unselected repository content.
- **CP-AC-23:** An architecture audit finds every authoritative project,
  document, language, Git, terminal/task/debug, agent, policy, persistence,
  projection, and receipt class in Effect/TypeScript. Any Rust helper is a
  process-opaque bounded primitive with generated conformance fixtures,
  explicit failure behavior and reversal threshold, and no credential,
  project/session/policy/database/receipt authority.
- **CP-AC-24:** The maintained daily-editor corpus enables and disables Vim
  from Settings, command palette, and status control. Proves persistent
  off-by-default mode, core Normal/Insert/Visual operations, counts, operators,
  text objects, registers, repeat, search, supported colon commands, clipboard,
  conflict-safe save/close, split/worktree isolation, IME/accessibility,
  restart, and complete listener teardown. And finds no extension-host or
  direct-Monaco authority path.
- **CP-AC-25:** Every initially supported Desktop IDE projection renders from
  one provenance-pinned Tokyo Night semantic token plane before first paint,
  passes checked contrast and non-color-state journeys, works offline, and
  retains document/terminal/review state across theme initialization. A full-
  parity claim additionally requires the deferred first-party light, high-
  contrast dark/light, and system-following corpus. Tokyo Night alone is never
  reported as complete theme/accessibility parity.
- **CP-AC-26:** An architecture audit finds a single Effect Schema source and
  derived TypeScript type for every persisted, IPC, helper, mobile/web, and
  public-share boundary. Constrained branded refs and stable codegen schema
  identifiers where applicable. No raw interface or handwritten union acting
  as a parallel contract. And every untrusted input decoded before use.
- **CP-AC-27:** A lifecycle and release audit finds IDE capabilities composed
  as Context.Service/Layer.effect services with named Effect.fn operations,
  Schema.TaggedErrorClass failures, and scoped resource interruption, and maps
  every Cursor ledger row to the exact IDE-00..19 packet, release rung,
  acceptance ref, proposed/admitted assurance state, remaining gap, and owner
  disposition without promoting a narrower rung by inference.

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: required_cursor_capability_rows_with_current_evidence_owner_acceptance_and_passing_acceptance_refs
  target: "100% before any full-parity claim"
  target_status: committed
  window: every release candidate
- id: SM-2
  metric: qualifying_cursor_switcher_workflows_that_require_reopening_cursor
  target: "0% across the maintained migration and daily-work corpus"
  target_status: committed
  window: every release candidate and rolling 30-day dogfood
- id: SM-3
  metric: parity_workflows_whose_execution_and_data_placement_can_be_selected_without_changing_surface_or_session_identity
  target: ">= 95%; every exception explicitly names an inherent managed dependency"
  target_status: provisional
  target_owner: "owner"
  window: before general availability parity claim
- id: SM-4
  metric: owner_accepted_capabilities_removed_or_materially_degraded_without_a_versioned_owner_exception
  target: "0"
  target_status: committed
  window: every release and rolling 90 days
- id: SM-5
  metric: session_data_classes_with_inspect_export_retention_and_verified_deletion_controls
  target: "100%"
  target_status: committed
  window: before general availability parity claim
- id: SM-6
  metric: cursor_import_items_imported_skipped_or_rejected_with_an_explicit_reason
  target: "100%"
  target_status: committed
  window: every migration run
- id: SM-7
  metric: zed_quality_integrated_ide_and_code_share_journeys_with_current_owner_accepted_evidence
  target: "100% before a full IDE, Zed-quality, or Cursor-parity claim"
  target_status: committed
  window: every release candidate
- id: SM-8
  metric: built_in_vim_daily_editor_journeys_passing_without_extension_or_authority_bypass
  target: "100% before the daily-use basic-IDE or later claim"
  target_status: committed
  window: every release candidate
- id: SM-9
  metric: initial_ide_surfaces_using_the_owned_tokyo_night_semantic_projection
  target: "100%, with zero unreviewed executable theme contributions"
  target_status: committed
  window: every packaged release candidate
- id: SM-10
  metric: cursor_ledger_rows_with_exact_ide_packet_rung_acceptance_assurance_gap_and_owner_disposition
  target: "100% before any parity-candidate or full-parity claim"
  target_status: committed
  window: every ledger refresh and release candidate
```

## Risks

- Cursor changes quickly. Evidence pinning, observation dates, and automated
  freshness warnings are required or parity becomes a stale marketing claim.
- “Everything” can flatten sequencing. The ledger is complete. Implementation
  remains criterion-addressed and dependency-ordered through roadmap and work
  authority.
- An unbundled product can expose too many choices. Opinionated defaults and
  progressive disclosure must coexist with inspectable advanced control.
- Extension and computer-use breadth expand the attack surface. Isolation,
  least authority, provenance, and receipts are launch requirements, not
  follow-up hardening.
- Local embedding models can underperform managed indexes, while remote indexes
  can violate custody expectations. The product must state quality, cost,
  corpus, and data-flow differences rather than presenting false equivalence.
- Compatibility import can accidentally ingest secrets or proprietary state.
  Import uses an allowlist and produces a complete disposition report.
- “Better model” is not permanent architecture. Model quality remains a
  replaceable plane and is measured independently from product parity.
- A package checklist can look like IDE parity while project, document,
  language, Git, terminal, and agent states still diverge. The integrated
  generation-fenced journey corpus, not dependency presence, is the gate.
- Public code sharing can accidentally become path/content exfiltration or a
  browser authority path. Bundle compilation is allowlisted, bounded,
  audience-aware, verifiable, revocable, and mutation-free.
- Choosing Rust because the reference IDE uses Rust would create a second
  application core and cross-language drift. Native helpers require measured
  necessity, generated contracts, authority-free state, and reversal tests.
- Treating Vim or a theme as library configuration would let adapter state
  bypass the command, document, focus, accessibility, and teardown laws. Both
  remain app-owned projections with explicit packaged journeys.
- Shipping only Tokyo Night can be mislabeled as accessibility/theme parity.
  It is the admitted initial default. The broader first-party theme corpus
  stays a visible parity gap until separately proven.

## Related Artifacts

- Canonical IDE roadmap, packet definitions, release rungs, Vim contract, and
  Tokyo Night contract: `docs/ide/ROADMAP.md`
- IDE-02 complete Explorer implementation and evidence (a CP-AC-19
  prerequisite, not parity closure):
  `docs/ide/2026-07-19-ide-02-complete-pierre-explorer.md`
- Roadmap-to-ProductSpec/AssuranceSpec traceability:
  `specs/IDE_ROADMAP_CROSSWALK.md`
- Proposed proof-design companion bound to this exact revision:
  `specs/openagents/cursor-capability-parity.assurance-spec.md`
- Cursor product and local-state evidence:
  `docs/teardowns/2026-07-11-cursor-product-teardown.md`
- Zed-quality integrated IDE and Effect/Rust architecture:
  `docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md`
- Zed agent-IDE adaptation, VS Code package reuse, and Monaco/Pierre plan:
  `docs/ide/2026-07-18-zed-agent-ide-adaptation-analysis.md`,
  `docs/ide/2026-07-18-vscode-typescript-reuse-analysis.md`, and
  `docs/ide/2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md`
- Fast Follow source and lessons: `FASTFOLLOW.md` (`source-cursor`)
- Desktop surface contract:
  `specs/desktop/desktop-trust-complete-workbench.product-spec.md`
- Mobile controller contract:
  `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`
- Web trust and supervision contract:
  `specs/web/openagents-com-trust-surface.product-spec.md`
- Portable execution contract:
  `specs/openagents/portable-coding-sessions.product-spec.md`
- Continuous competitor-learning contract:
  `specs/openagents/fast-follow.product-spec.md`
- Full Auto run contract:
  `specs/desktop/full-auto.product-spec.md`

## Owner Gates

- The exact release at which OpenAgents publicly claims full Cursor parity.
- Any permanent parity-row exception or substitute that materially changes a
  familiar Cursor workflow.
- Default remote indexing, telemetry, managed execution, or retention policy.
- Public marketplace publication, extension trust tiers, and organization-wide
  auto-install policy.
- Any default computer-use profile broader than workspace-bounded, approved
  actions.

## Receipts

- Versioned Cursor capability ledger with pinned evidence, freshness, target,
  disposition, acceptance, implementation, and assurance refs.
- Cursor-to-OpenAgents migration report with per-item dispositions and secret/
  proprietary-material scan.
- Cross-surface parity corpus results for workbench, agents window, CLI, web,
  mobile, remote/background execution, and automations.
- Harness/model/placement/data-flow manifests and effective execution receipts.
- Local/remote data inventory, export, selective deletion, full deletion, and
  verified tombstone receipts.
- Platform release, signing, rollback, performance, accessibility, and fault-
  injection evidence.

## Promise Links

No public parity claim is created by this ProductSpec. “Cursor parity,” “full
parity,” “drop-in replacement,” or any enumerated feature-availability claim
requires the maintained ledger at the claimed release, all required rows at
owner-accepted, passing evidence, a registered versioned promise, and owner
sign-off. OpenAgents may describe target intent before then only as planned or
in progress.
