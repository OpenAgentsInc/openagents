# Porting Zed to Effect — where we are and what is left

- Date: 2026-07-19
- Class: status analysis and episode source
- Baseline: OpenAgents `bdadc3ca437dbfcbf054764da77edbfe8d29612b`
- Zed source pin: `zed-industries/zed` `f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4` (app version 1.13.0)
- Status: analysis only. This document changes no ProductSpec, no promise
  state, and no release authority. The status authorities remain
  [`ROADMAP.md`](./ROADMAP.md), current code, live issue state, tests, and
  receipts.

## Why this document exists

The next video is "Porting Zed to Effect." This document collects, in one
place, what "port Zed to Effect" actually means for OpenAgents, what is already
built, what is in flight, and what still needs a design decision. It is scoped
to the Zed lane only. It does not cover the Cursor and VS Code parts of the IDE
program except where they share the same substrate. Those live in
[`2026-07-18-vscode-typescript-reuse-analysis.md`](./2026-07-18-vscode-typescript-reuse-analysis.md)
and the Cursor teardown.

## The one-sentence answer

We are not porting Zed. We are doing a **semantic port**: we reproduce Zed's
best idea — one typed project graph that the editor and the agent both operate
over — in Effect and Effect Native, and we keep Monaco and Pierre as the visible
editor and diff components. We have shipped that graph and the first agent loop
on top of it. We have not yet shipped Zed's UI breadth: search, tasks,
debugging, and language coverage are the visible gaps.

## What "Zed parity in Effect" means here

The load-bearing decision was made in the Zed adaptation analysis
([`2026-07-18-zed-agent-ide-adaptation-analysis.md`](./2026-07-18-zed-agent-ide-adaptation-analysis.md))
and locked into the roadmap. Quoting the adaptation analysis:

> The most important Zed idea is not GPUI, Rust, a rope, or an agent panel. It
> is that the editor and agent operate over the same typed project graph.

The product thesis it settles on:

> One project and evidence graph, many editors and agent runtimes, one
> canonical authority path.

So "Zed parity" for us is not a component-for-component match. It is:

1. **Take** Zed's coherence — worktrees, buffers, language, Git, terminals,
   tasks, and agents share one project model with stable refs and explicit
   generations.
2. **Keep a stronger authority boundary than Zed.** Sharing identity and
   evidence must not imply sharing privilege. A Monaco model, a Codex harness, a
   Full Auto run, and the mobile controller can all name the same file version
   and receipt while holding different execution, approval, and disclosure
   scopes.
3. **Skip** GPUI, the Rust application core, the rope/SumTree/MultiBuffer/Editor
   internals, and the GPL-default source itself.

The take/skip split from the roadmap, verbatim:

| Source | What OpenAgents takes | What OpenAgents does not take |
| --- | --- | --- |
| Zed | one coherent project/document/language/Git/terminal/agent graph, plus agent context and code evidence as native IDE concepts | GPUI, a Rust application core, or Zed's editor implementation |
| VS Code | Monaco, plus focused URI, language, LSP, terminal, debug lessons | a Code-OSS fork, workbench internals, or trusted extension host |
| Pierre | React tree and diff kernels, theming seams, virtualized review | filesystem, Git, document, mutation, session, or policy authority |

The architecture the port lands in is fixed by
[`2026-07-18-zed-quality-ide-effect-rust-architecture.md`](./2026-07-18-zed-quality-ide-effect-rust-architecture.md):
Effect/TypeScript owns the complete project/document/language/Git/agent/policy/
persistence graph, and Rust is a narrow rind for process-opaque PTY/containment
and benchmark-admitted native kernels only. Rust never owns a project, document,
session, credential, policy, database, approval, or receipt.

## Where we are now

The build sequence is [`ROADMAP.md`](./ROADMAP.md), packets IDE-00 through
IDE-19. As of this baseline, the first eleven packets are closed with issue
receipts, and the ACP protocol layer that lets external agents (Claude Code,
Codex, Grok, Cursor) share the same graph is complete.

### Delivered — the project graph and the agent loop

| Packet | Outcome | Issue | State |
| --- | --- | --- | --- |
| IDE-00 | Schema-first project graph, branded refs and generations, boundary decoders, baseline benchmarks | #9015 | closed |
| IDE-01 | Pinned `monaco-editor@0.55.1` and `@pierre/diffs@1.2.12`, Electron packaging/worker/CSP spikes, Vim engine decision, Tokyo Night projection | #9016 | closed |
| IDE-02 | Complete generation-fenced Pierre Explorer over an Effect path index | #9017 | closed |
| IDE-03 | Real Monaco lifecycle, Tokyo Night mounted, built-in off-by-default Vim controller | #9018 | closed |
| IDE-04 | Quick open, tabs/splits, navigation history, settings, keybindings, file operations | (daily-workbench packet) | closed |
| IDE-05 | Pierre review over eight versioned diff sources with a staleness law | (versioned review) | closed |
| IDE-06 | Document-local Monaco workers plus a persistent Effect-supervised project TypeScript service | (language) | closed |
| IDE-07 | Packaged daily-use "OpenAgents basic IDE" acceptance gate (macOS arm64) | #9022 | closed |
| IDE-08 | Agents native to the code graph: project-bound sessions, context tray, version-bound proposals, Pierre review, apply/rebase/undo, backlinks | #9036 | closed |
| IDE-09 | Cursor-class AI editing: completion, next-edit, inline transforms over the exact proposal graph, with disclosed effective model | #9037 | closed |
| IDE-10 | Terminal, tasks, tests, and Output over one project graph | #9038 | closed |

The code exists and is wired, not stubbed. `apps/openagents-desktop/src/ide/`
holds ~77 modules with paired tests: `project-contract.ts` and
`project-service.ts` (the graph), `path-index-service.ts` (Explorer),
`monaco-document-contract.ts` and `monaco-runtime-loader.ts` (editor),
`language-service.ts` and `language-host.ts` (TypeScript intelligence),
`agent-code-contract.ts`/`agent-code-service.ts`/`agent-code-host.ts` (the
IDE-08 agent loop), `cursor-*.ts` (IDE-09 AI editing),
`khala-editor-theme.ts`/`desktop-editor-themes.ts` (theming), and
`managed-sandbox-*.ts` (the SBX placement consumer).

### Delivered — the ACP layer (external agents on the shared graph)

The Agent Client Protocol work (ACP-1 through ACP-10, issues #8888–#8897, plus
the multi-agent parity epic #8898) is fully closed. This is the concrete
realization of "many agent runtimes over one graph": OpenAgents pins
`schema-v1.19.0` and hosts three packages —
`packages/agent-client-protocol` (the generated protocol authority),
`packages/agent-client-protocol-conformance` (wire-v1 conformance, peer
fixtures, fault matrices, live-release suite), and
`packages/agent-client-runtime-bridge` (bridges native ACP sessions into the
canonical runtime and event authority). Codex, Claude Code, Grok, and Cursor
CLIs are interchangeable peer profiles behind capability negotiation.

This is the piece Zed also has (`crates/acp_thread`), and it is where we are at
or ahead of Zed: our version carries a stronger authority boundary and a signed
conformance/release gate.

### The honest gap — measured, not guessed

The fresh programmatic comparison
([`docs/sol/2026-07-19-openagents-ide-zed-programmatic-ui-gap-analysis.md`](../sol/2026-07-19-openagents-ide-zed-programmatic-ui-gap-analysis.md),
Sol issue #9035) built both products from source, launched both in isolation,
and compared UI breadth across six evidence planes. Its verdict:

> OpenAgents is not at Zed-class UI breadth. It has a credible agent-first IDE
> base, but it does not yet have a complete desktop IDE interface.

Scored 11/20 (Acceptable) against Zed's 16/20 (Good). The result reads as a
barbell: we lead on the agent-first surface, Zed leads on classic IDE breadth.

Source-signal density (exact regex match counts — evidence density, not a
capability score):

| Signal | OpenAgents | Zed | Meaning |
| --- | ---: | ---: | --- |
| Agent context and proposal | 5,497 | 2,116 | OpenAgents leads on the agent-first model |
| Editor tabs and splits | 1,551 | 23,273 | Zed has far deeper pane/editor implementation |
| Language diagnostics | 1,029 | 7,820 | Zed has far broader language infrastructure |
| Search and commands | 85 | 690 | OpenAgents has clear search/palette gaps |
| Settings, theme, keymap | 1,891 | 17,129 | Zed has far deeper customization UI |
| Terminal, task, test, debug | 1,130 | 8,632 | Tasks and debugging are absent from OpenAgents UI |

The four **P0** gaps for a broad IDE claim: a queryable command palette (ours
renders a fixed list with no query input), complete project search (regex,
replace, include/exclude, scope), language coverage beyond TypeScript/JavaScript,
and tasks + debugging UI. The two loudest **P1** quality gaps: all 24 OpenAgents
visual states drift past the checked-in gate (a stale fixture, not necessarily a
regression), and the AI editing surface (`react-cursor.tsx`) references two CSS
classes with no matching stylesheet selector — a strong state model with no
production visual layout yet.

Strengths the analysis says to **preserve** — these are the "left lead," the
things Zed does not replace: typed agent proposals and selected operations,
exact evidence/lineage/checkpoint state, conversation-to-code backlinks, explicit
partial/degraded/stale/recovery states, remote agent control, browser preview
presets, and typed workspace mutations in the Pierre explorer.

## What we are working on / next

IDE-11 and IDE-12 are closed. IDE-13 is now the active frontier. Its schema,
bounded attachment model, main-owned coordinator, and confirmed Desktop
projection are implemented. Real placement acceptance is not complete.

| Packet | Outcome | Issue | State |
| --- | --- | --- | --- |
| IDE-11 | Debug via a supervised Effect DAP capability graph | #9039 | closed |
| IDE-12 | Complete Git, worktrees, review, delivery with exact-version receipts | #9040 | closed |
| IDE-13 | Portable project capability contract and real placement proof | #9041 | **open — foundation implemented. Acceptance gaps remain.** |

IDE-11 and IDE-12 together close the "integrated OpenAgents agent IDE" rung
(IDE-08–12). After they land, a release may honestly say "integrated agent IDE"
— but not "Cursor parity."

Running in parallel at P1 is the managed-sandbox program, epic #9023
(SBX-00 through SBX-10, issues #9024–#9034). Most SBX packets have landed as
default-off components. SBX-06 (#9027) already shipped the managed placement
consumer that IDE-13 and IDE-17 will later depend on. SBX-09 (#9033) remains the
live acceptance gate before any managed-execution release claim.

## What is planned but not started

IDE-14 through IDE-19 are open (#9042–#9047). IDE-13 is in progress:

- **IDE-13** — the contract, model, coordinator, and Desktop projection are
  implemented. Real owner-remote, OpenAgents-managed, provider, fault,
  packaged, and performance acceptance evidence remains open.
- **IDE-14** — safe mobile review, web supervision, and the immutable public
  `CodeShareBundle`.
- **IDE-15** — isolated extension and component ABI, deny-by-default, no trusted
  in-process VS Code extension host.
- **IDE-16** — preview, browser, design-to-code, and gated computer use.
- **IDE-17** — the full agent platform: Agents Window, side chats, best-of-N,
  background/cloud continuation, automations.
- **IDE-18** — data lifecycle/erasure convergence, Cursor migration import,
  six-target delivery, enterprise, complete accessibility and broader themes.
- **IDE-19** — the maintained Cursor-parity ledger and owner acceptance gate.
  Only IDE-19 plus the ProductSpec promise gate may say "Cursor parity."

The release rungs are named and gated so no rung can hide the next one's gaps:
Files foundation (landed) → basic IDE (IDE-00–07, accepted) → agent IDE
(IDE-08–12) → Cursor-parity candidate (IDE-13–18) → full parity (IDE-19).

## What still needs designing

These are the open design decisions the port has deliberately deferred. They are
the honest "we have not solved this yet" list for the episode.

From the Zed adaptation analysis (its "still needs an admitted implementation
decision" list), the ones still open:

- The first non-TypeScript **LSP implementation** and the local/remote placement
  split for language capabilities. Today the project-aware gate accepts only
  TypeScript/JavaScript. Every other language is an explicit gap.
- Whether main persists **canonical incremental text** or checkpointed snapshots
  around Monaco.
- The **worktree creation/archive retention and recovery policy** (Zed's
  `thread_worktree_archive` logic is the reference. IDE-12 owns our version).
- **Local lexical/semantic index** implementations and defaults. The port
  deliberately rejects "repository embeddings required for code understanding"
  and instead reserves a typed context-candidate portfolio (BM25-style lexical,
  recent edits, diagnostics, Git co-change) — but the concrete index and its
  custody/retention/disclosure contract are unbuilt.
- **Writable multi-file excerpt editing.** `ExcerptRef`/`ExcerptSet` identity is
  reserved read-only. The writable MultiBuffer analogue (Zed's strongest
  long-term idea) is explicitly deferred until coordinate mapping,
  stale-generation behavior, undo, and authority have dedicated proof.
- The **extension component ABI** and any collaboration consistency model.

From the fresh gap analysis, the design work each P0 needs before it becomes a
packet:

- **Command palette query** — add a query input, fuzzy ranking, result count,
  and no-result state on top of the existing command registry (keep the registry
  as authority. Do not let the palette invent a second command universe).
- **Project search** — regex/replace/globs/scope plus preview-before-replace and
  operation receipts, without letting a text replacement bypass workspace
  mutation authority.
- **Tasks then debug** — both need explicit execution authority, environment
  identity, cancellation, output, and evidence. IDE-10 landed the tasks/Output
  substrate. The visible task runner UI and IDE-11's DAP graph are the remaining
  design.
- **AI editing visual layout** — `react-cursor.tsx` needs a real production
  layout for request controls, candidate navigation, decision controls, errors,
  focus, and overflow, tested across completion/next-edit/answer/proposal states.
- **Visual gate repair** — the `files-rich-diff` fixture still seeds the old
  workspace-browser model instead of the current path-index projection. Update
  the fixture to the current state model, then admit new baselines in a separate
  visual change (do not update baselines only to turn the ratio green).

A product-authority decision (not an engineering one) is still owed on **Git
mutation**: our Git surface is intentionally read-only. IDE-12 proposes safe
staged mutation, but whether full Git delivery UI belongs in the product, or
stays a retained scope boundary, needs owner sign-off. The same "needs product
decision" flag applies to an editor extension marketplace, SSH workspace
editing, and live human collaboration — all Zed capabilities the gap analysis
explicitly refuses to treat as automatic backlog items.

## The honest narrative for the episode

The clean arc for "Porting Zed to Effect":

1. **The wrong way to port Zed** is to port GPUI, the rope, and the editor —
   importing an entire Rust application core and its GPL license. We say no to
   all of it.
2. **The right thing to take** is one idea: the editor and the agent share one
   typed project graph. That is what makes Zed's agent feel like part of the IDE
   instead of a chat box next to it.
3. **We rebuilt that graph in Effect** — `IdeProjectService` and its capability
   services, schema-first with branded refs and explicit generations — and put
   Monaco and Pierre on top as replaceable components.
4. **We went one better than Zed on authority.** Same refs, different privilege.
   An agent proposes. The project service applies. A harness never mutates
   Monaco. External agents ride the ACP layer without inheriting native session
   authority.
5. **We shipped the agent loop first** (IDE-08/09), which is the part we care
   about most and the part where our source-signal density already exceeds Zed's.
6. **We are honest about the gap.** We built a programmatic comparison harness,
   ran both products from source, and scored ourselves 11/20 to Zed's 16/20.
   Search, tasks, debugging, and language breadth are the real holes, and they
   are the next packets.

The tension that makes the episode true rather than a demo reel: a coherent
graph and a strong authority model are the hard, invisible 80%. The visible 20%
— a search box that takes a query, a debugger, a task list — is what a viewer
sees Zed do and OpenAgents not yet do. Both facts are real at the same time.
