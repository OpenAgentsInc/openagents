# OpenAgents Desktop Vercel AI SDK in-place reset audit

- Class: historical-analysis
- Status: current point-in-time decision analysis
- Snapshot: 2026-07-18
- Dispatch: no; this audit does not authorize the product reset it evaluates
- Owner: OpenAgents Desktop AI SDK reset analysis
- Target: `apps/openagents-desktop/`
- Companion roadmap:
  [`2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md`](./2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md)
- Effect source-conversion audit:
  [`2026-07-18-vercel-ai-sdk-source-derived-effect-conversion-audit.md`](./2026-07-18-vercel-ai-sdk-source-derived-effect-conversion-audit.md)

## Question

Instead of building Full Auto in the separate
`apps/electron-ai-sdk-test/` fixture, should OpenAgents strip the current
`apps/openagents-desktop/` application down to its Electron shell and replace
its runtime and front end with Vercel AI SDK Harness, `@ai-sdk/react`, and AI
Elements?

The desired replacement remains deliberately narrow:

- Codex starts;
- Codex and Claude Code alternate through typed host-owned handoffs;
- both work in one shared workspace;
- a bounded objective, done condition, and turn cap own the run; and
- the product exposes only the launcher, read-only run transcript, Stop, and
  terminal result needed for that loop.

## Executive conclusion

The in-place reset is technically viable. It is not the fastest safe way to
prove the loop, and it is not a normal migration.

It would be a product reset that keeps the Electron application identity and a
small part of its host/distribution machinery while retiring most of the
current Desktop application, its runtime authority, its UI state model, and
its enforced guarantees.

The key distinction is:

- **Fastest working proof:** build the separate AI SDK test app described by
  the companion roadmap.
- **Smallest eventual Desktop codebase:** reset the current app in place after
  accepting the contract retirements in this audit.
- **Worst option:** partially insert `useChat` and Harness into the current
  Desktop while leaving Effect Native state, Runtime Gateway conversation
  authority, provider lanes, and `FullAutoRun` as competing owners.

If the owner truly wants “Codex and Claude Full Auto, nothing else” to replace
the current Desktop product, a clean in-place reset is more coherent than a
long compatibility migration. But that decision must explicitly retire the
current product contracts and amend repository invariants before code is
deleted. This audit does not grant that authority.

## What exists today

The current Desktop is not an empty Electron shell or an obsolete chat demo.
At this snapshot it is release candidate `0.1.0-rc.20` with:

- 461 files under `src/`;
- 186 `*.test.ts` / `*.test.tsx` files under `src/` alone;
- a 7,934-line main-process composition;
- a 1,225-line closed preload bridge;
- a 5,707-line typed Effect Native shell model;
- a 2,005-line renderer boot composition;
- a 1,333-line canonical conversation timeline;
- a dedicated React Full Auto launcher and read-only run surface;
- 36 explicit `Contract:` / `Contracts:` headings in `GUARANTEES.md`; and
- packaging, signing, update, rollback, restart, visual-baseline, Playwright,
  and release-acceptance machinery.

The separate AI SDK fixture has six source files. Its main process, renderer,
preload, and stylesheet total 821 lines. That contrast explains the appeal of
the reset, but it also shows why this is deletion of product authority rather
than replacement of one provider SDK.

### Current Full Auto already implemented

The active app already owns:

- a durable `FullAutoRun` with objective, done condition, workspace, turn cap,
  lifecycle history, and one-active-run admission;
- distinct running, pausing, paused, retrying, stalled, completed, failed,
  stopped, and cap-reached states;
- main-owned exactly-once continuation and restart reconciliation;
- liveness, backoff, stall classification, and attention signals;
- private reports and public-safe receipts;
- manual cross-provider handoff while paused;
- provider admission/auth/capability rechecks and rollback on refusal;
- durable handoff receipts and bounded handoff envelopes;
- failure-triggered provider fallback rotation;
- a dedicated launcher and read-only run view; and
- a six-mission Codex/Claude acceptance corpus.

What it does not implement is the exact desired rule “Codex and Claude
alternate after every successful turn.” Current handoff is owner-directed and
current automatic rotation is failure-triggered. The Vercel design is smaller
because it makes strict two-lane alternation the whole product rather than one
policy inside a general provider/run system.

## Three different proposals hidden inside “switch to Vercel AI SDK”

### 1. Adapter replacement

Keep current `FullAutoRun`, provider-lane SPI, Runtime Gateway, Effect Native
state, reports, Sync projections, and UI. Replace only the Codex/Claude runtime
adapters with `HarnessAgent`.

This preserves the most value and deletes the least code. It does not satisfy
the request to strip the app down, and AI SDK would remain an adapter rather
than the whole runtime/front-end architecture.

### 2. Surface replacement

Keep the durable host authorities and replace the renderer conversation state
and components with AI SDK UI messages, `useChat`, and AI Elements.

This creates two state systems unless the current Effect Native application
authority is retired. It is the highest-risk middle state and is not
recommended.

### 3. In-place product reset

Keep only the hardened Electron host, application identity, build/package
pipeline, release/update safety, and the minimum closed preload needed for one
Full Auto stream. Delete the rest and install the bounded runtime from the
companion roadmap.

This is the proposal the rest of this audit evaluates.

## Proposed retained shell

An honest reset does not start by deleting all of `apps/openagents-desktop/`.
It first names the small set of host responsibilities worth retaining.

| Retain | Reason |
| --- | --- |
| Electron window creation and custom trusted renderer origin | Already hardened and packaged. |
| `contextIsolation`, renderer sandbox, Node-off, webview-off, permission/navigation/window-open denials | AI SDK does not replace the Electron trust boundary. |
| Main-owned local CLI discovery and credentials | Provider authority must remain outside the renderer. |
| Vite/React/Tailwind build and Electron packaging | Already matches most AI Elements prerequisites. |
| Application identity, signing, notarization, release selection, update, and rollback machinery | Preserves the distribution investment if the same product identity is intentionally retained. |
| Isolated dev/smoke profiles | Prevents test runs from contesting installed-app state. |
| Public-safe diagnostics for startup/runtime failure | Required to operate the replacement. |
| A reduced visual and built-Electron acceptance harness | Needed to prove the reset is an actual desktop app, not only a React test. |

Everything retained must have a direct dependency from the new two-provider
product. “It might be useful later” is not a reason to keep a subsystem.

## Proposed deleted application

The literal reset retires these current application authorities rather than
adapting them:

- Effect Native application state, intents, renderer catalog, and compatibility
  backend;
- Runtime Gateway conversation and agent-timeline protocols;
- Khala Sync conversation mode and mobile continuation;
- local-first identity, PKCE account linking, and native session custody;
- current Codex app-server control plane, history importer, recovery journal,
  and provider-native timeline projection;
- the general provider-lane registry and ACP/Cursor/Grok lanes;
- Pylon/Fleet surfaces and control paths;
- ProductSpec/AssuranceSpec internal workrooms;
- ordinary chat, recent sessions, history search, rename/fork, composer queue,
  attachments, commands, approvals, and interactive questions;
- Files, workspace browser/editor/search, Git/GitHub review, terminal, preview,
  voice, MCP, plugin, and diagnostics product surfaces;
- current Full Auto registry, control API, liveness, reports, analyzer, Sync
  projection, remote mobile controls, routing policy, and acceptance driver;
- the current preload capability catalog; and
- tests and documentation whose only purpose is to enforce those retired
  surfaces.

Do not leave these compiled but hidden. A reset that only removes navigation
keeps the complexity and authority collisions while losing the user value.

Old owner data must not be deleted. The replacement may ignore prior stores,
but a reset must preserve them in place or archive them through an explicit,
recoverable migration. “No longer read” is different from “erase.”

## Proposed Vercel replacement

The replacement inside `apps/openagents-desktop/` would copy the bounded
architecture of the companion roadmap rather than the current fixture's exact
transport.

### Main process

- one `HarnessAgent` for Codex;
- one `HarnessAgent` for Claude Code;
- one shared `LocalAiSdkSandboxProvider`;
- one fixed shared working directory per run;
- separate native Harness resume state per lane;
- one typed `full_auto_handoff` host tool;
- one serialized Codex-first alternator;
- one in-memory active run with objective, done condition, cap, Stop, and
  terminal state; and
- AI SDK `UIMessage` stream composition for provider and host events.

### Preload and transport

Do not copy the fixture's “reveal a loopback URL to the renderer” design into
the installed app.

Implement a custom AI SDK `ChatTransport` over a small closed preload contract:

- `start(spec)`;
- `stop(runRef)`;
- `snapshot(runRef)`; and
- `subscribe(runRef, listener)` returning an owned unsubscribe function.

Main validates the trusted frame and schema before every action. Preload and
renderer receive no provider credential, absolute workspace path, generic IPC
channel, process handle, or arbitrary local endpoint.

AI SDK explicitly supports custom transports and custom UI-message streams, so
this keeps `useChat` without weakening the existing Electron boundary.

### Renderer

- React 19 root;
- `useChat` with the IPC-backed transport;
- a launch form for objective, done condition, and cap;
- a read-only run view after Start;
- selected AI Elements for conversation, message, response, reasoning, tool,
  loader, and scroll behavior;
- one visible host-owned handoff receipt between turns; and
- one Stop action and one terminal result.

AI Elements is a source registry built on shadcn/ui, React 19, and Tailwind 4,
which fits much of the existing renderer toolchain. Its documented happy path
targets Next.js, not Electron/Vite. The selected components therefore require
an explicit Electron/Vite integration test rather than an assumption of
drop-in support.

## Authority blockers

### P0 — Current repository law forbids the replacement architecture

`INVARIANTS.md` currently requires the Desktop application, component, state,
projection, and typed-intent model to remain Effect Native. It also explicitly
forbids a Vercel AI SDK/model-stream authority from becoming the mounted
product authority.

An in-place reset must update that invariant in the same change as the new
architecture and add replacement boundary tests. Implementing first and
calling the invariant obsolete afterward is not allowed.

### P0 — The reset invalidates current public/test-backed guarantees

`GUARANTEES.md` is the package's declared behavioral truth. A literal reset
invalidates most of its 36 contract sections, including conversation
continuity, restart recovery, Full Auto durability, renderer authority,
identity/Sync, release behavior, and current visible surfaces.

The reset cannot leave those claims in place. Each guarantee must be retained
with a replacement oracle, retired with an explicit tombstone/reason, or moved
out of the product before the reset is called complete.

### P1 — AI SDK Harness is still experimental

Vercel describes `HarnessAgent` and the initial Codex/Claude adapters as
experimental and warns that breaking changes should be expected. The fixture
already mitigates this with exact package pins. Moving the installed Desktop
onto it would require an explicit compatibility gate around every dependency
update and a retained last-known-good package set.

### P1 — Partial migration creates duplicate state authority

`useChat` owns a renderer message list and stream lifecycle. Current Desktop
already owns durable run, thread, timeline, command, and provider state through
Effect Native and Runtime Gateway contracts. Mounting both without retiring one
creates disagreement over message IDs, terminal state, Stop, reconnect, and
which provider acts next.

There should be one cutover commit range, not a long-lived dual-renderer or
dual-run state.

### P1 — The default web transport weakens the Desktop boundary

The AI SDK default transport posts to `/api/chat`. The fixture uses a loopback
HTTP server and exposes its endpoint to the renderer. The production Desktop
contract deliberately hides loopback URLs and generic runtime transports.

The reset must use a custom closed IPC-backed `ChatTransport` or explicitly
change the trust boundary. Reusing the fixture endpoint unchanged is not an
acceptable shortcut.

### P1 — Resetting the package changes the meaning of an existing release

The app is already tagged and packaged as OpenAgents Desktop. Shipping the
minimal experiment through the same application/update identity would replace
existing behavior for installed users and strand prior local data.

Before implementation the owner must choose one:

1. preserve the same identity and intentionally ship a product reset with
   recoverable old data;
2. use a new bundle/update identity for the minimal product; or
3. do not reset the current app.

That choice cannot be inferred from a code preference.

### P2 — AI Elements is outside its documented Electron/Vite path

The AI Elements setup guide names Next.js 14+ as its supported prerequisite.
The components are source-installed shadcn components and the Desktop already
has React 19, Tailwind 4, and shadcn plumbing, so adaptation is plausible. It
is still an integration to prove, not a supported-environment assumption.

Use only the selected component sources and cover imports, hydration, keyboard
behavior, scrolling, and production bundling in the real Electron renderer.

### P2 — Deletion removes mature UX and release oracles

The current app already tests focus, keyboard actions, reduced motion,
responsive layout, visual baselines, built-Electron behavior, restart, and
release artifacts. Most of those tests will no longer compile after a literal
reset.

Retire obsolete assertions, but rebuild equivalent oracles for every retained
interaction and host guarantee before deleting the old suites. A much smaller
test count is acceptable; an untested replacement is not.

## Interface audit snapshot

This score describes the current Desktop surface as a starting point for the
reset. It is not a score for an unbuilt AI SDK UI.

- Migration findings: 2 P0, 4 P1, 2 P2, 0 P3.
- Anti-pattern verdict: pass. The current Full Auto surface looks like a
  serious product tool rather than a generic AI chat template.

| # | Dimension | Score | Key finding |
| --- | --- | --- | --- |
| 1 | Accessibility | 3/4 | Full Auto fields are labelled, states use ARIA, controls are semantic, and reduced motion is enforced; the replacement must retain these oracles. |
| 2 | Performance | 2/4 | Rendering is bounded and tested, but the app carries a very large shell and the Full Auto view polls every three seconds. |
| 3 | Responsive design | 3/4 | Structural breakpoints, bounded transcript width, narrow drawers, and reduced chrome exist; the sidebar disappears below 760 px without a replacement rail. |
| 4 | Theming | 4/4 | The current renderer rigorously maps Tailwind/shadcn roles to one Effect Native token authority. |
| 5 | Anti-patterns | 4/4 | The current Full Auto surface is task-first, semantic, and restrained; it avoids ornamental AI-product styling. |
| **Total** |  | **16/20** | **Good — the reset should preserve its interaction quality even if it discards the architecture.** |

### Positive findings to preserve

- Full Auto is already a dedicated run surface, not a composer toggle.
- Objective and done condition are explicit and labelled.
- Run state, provider, turn count, Stop, and terminal details are visible.
- The existing visual system is dense, restrained, token-driven, and readable.
- Browser privileges and renderer authority are tightly bounded.
- Built-Electron, Playwright, visual-baseline, restart, and release tests prove
  more than a DOM snapshot.

### UI risks in the reset

- AI Elements must not bring a second palette, typography system, or arbitrary
  radius scale into the app.
- Generic AI Elements tool cards must not hide the provider and turn that own
  each event.
- `useChat` loading state is not the Full Auto lifecycle; the host's run status
  must remain visible and separate.
- The transcript cannot become the only evidence of handoff or completion.
- Removing the current shell must not remove focus management, keyboard Stop,
  reduced-motion handling, readable error states, or narrow-window behavior.

## Decision matrix

| Path | Time to first live Codex-Claude proof | Destructive scope | Final complexity | Contract risk | Fit for “nothing else” |
| --- | --- | --- | --- | --- | --- |
| Separate AI SDK fixture | Fastest | Low | Two apps until a later decision | Low | Excellent for proof, not final product consolidation |
| Harness adapter inside current Desktop | Medium | Low | High; current product remains | Medium | Poor |
| AI SDK renderer over current authorities | Slow | Medium | Highest; duplicate state/projection layers | High | Poor |
| Literal in-place product reset | Slowest safe landing | Very high | Lowest after completion | Very high | Excellent if current product is intentionally retired |

The in-place reset can feel faster while coding because it deletes far more
than it adapts. It is slower to land honestly because policy, data, release,
and guarantee retirement are part of the change.

## Safe cutover shape if the owner chooses the reset

This is an evaluation sequence, not dispatched work.

### Gate 0 — Owner reset decision

Record an explicit decision that:

- the current Desktop product behavior may be retired;
- the exact retained Electron/release responsibilities are accepted;
- the existing application identity is retained or replaced;
- old user data is preserved but may become unread by the new app;
- production Effect Native/model-stream invariants may be rewritten; and
- the new public promise is only the bounded Codex-Claude Full Auto product.

Without this gate, stop.

### Gate 1 — Prove the replacement before deletion

Complete the companion roadmap's three-turn Codex -> Claude -> Codex shared
workspace mission with pinned dependencies. This is the falsifier for the
reset. If the small app cannot prove the loop, deleting the current app creates
no value.

### Gate 2 — Freeze the retained host

Create a mechanical retain list for:

- Electron hardening;
- custom renderer origin;
- build/package/sign/notarize;
- update/rollback;
- isolated development and smoke profiles; and
- the minimum diagnostics and test harness.

Everything else is presumptively deleted.

### Gate 3 — Change policy and contracts first-class

In the same scoped program:

- amend `INVARIANTS.md`;
- replace `GUARANTEES.md` with the new bounded guarantee set;
- retire or tombstone superseded behavior contracts;
- update the Desktop ProductSpec and public promise boundary;
- remove stale Sol authority claims; and
- add tests for the new closed IPC transport, renderer import boundary, and
  provider-private state isolation.

### Gate 4 — Replace, do not layer

Land the minimal main/controller/preload/renderer together. Do not leave
current provider lanes or Runtime Gateway conversation state as fallback
authorities. Do not keep hidden surfaces compiled for possible later reuse.

### Gate 5 — Acceptance and release

Require:

- fixture alternation, missing/duplicate handoff, Stop, cap, and terminal tests;
- the live three-turn shared-file proof;
- built-Electron security smoke;
- keyboard, focus, narrow-window, reduced-motion, and screen-reader-state
  checks;
- clean-machine Codex and Claude authentication proof;
- package/sign/notarize/update/rollback gates; and
- explicit confirmation that prior user data was preserved.

## Replacement guarantee floor

Even the stripped app needs a small enforceable guarantee set:

1. exactly one active run;
2. Codex acts first and only the host alternates providers;
3. both lanes share one exact workspace;
4. provider-native session state remains separate;
5. every completed turn has exactly one valid typed handoff;
6. Stop prevents the next dispatch;
7. cap, complete, blocked, failed, and stopped are distinct;
8. the renderer receives no provider credentials or general host authority;
9. AI SDK messages are projection, not run authority;
10. browser privileges and external navigation remain denied;
11. the UI remains usable by keyboard and under reduced motion; and
12. package/update behavior remains fail-closed if the current distribution
    identity is retained.

Anything beyond this list requires a new admitted product decision. This keeps
the reset honest rather than immediately rebuilding the old app.

## Recommended disposition

Do not reset `apps/openagents-desktop/` before the separate fixture proves the
exact loop. The fixture is the cheap technical falsifier.

After that proof, make a binary owner choice:

- keep the current Desktop product and integrate Harness only at the provider
  adapter seam; or
- intentionally retire the current Desktop product and execute the literal
  in-place reset.

Do not pursue the middle path of an AI SDK renderer over current Desktop
authorities. It preserves nearly all current complexity while adding a second
message, stream, component, and lifecycle system.

If “Full Auto between Codex and Claude, nothing else” is the durable product
decision—not only an experiment—the literal reset is architecturally cleaner.
Its cost is the explicit deletion of the current product, not difficulty using
Vercel AI SDK.

## Recommended interface actions if reset is authorized

1. **P0 — `$impeccable distill`:** reduce the retained Desktop surface to the
   launcher, run header, read-only transcript, handoff receipts, Stop, and
   terminal state.
2. **P1 — `$impeccable shape`:** define the exact launch/running/terminal state
   transitions before installing AI Elements.
3. **P1 — `$impeccable harden`:** prove the IPC transport, error states,
   keyboard behavior, reduced motion, narrow windows, and provider failure.
4. **P2 — `$impeccable polish`:** reconcile selected AI Elements to the one
   retained product theme after behavior is complete.

Re-run the interface audit after the replacement exists; the current 16/20
score is a floor to preserve, not proof of the future surface.

## Source basis

This audit inspected:

- the current `apps/openagents-desktop/` package, source inventory, package
  scripts, dependencies, renderer, Full Auto implementation, tests,
  `README.md`, and `GUARANTEES.md`;
- [`INVARIANTS.md`](../../INVARIANTS.md);
- [`2026-07-10-openagents-desktop-product-architecture.md`](./2026-07-10-openagents-desktop-product-architecture.md);
- the companion
  [`Electron AI SDK Codex-Claude Full Auto rewrite roadmap`](./2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md);
- the current `apps/electron-ai-sdk-test/` implementation and installed pinned
  AI SDK Harness source;
- [Vercel's AI SDK 7 release](https://vercel.com/blog/ai-sdk-7);
- [Vercel's HarnessAgent announcement](https://vercel.com/changelog/program-agent-harnesses-with-ai-sdk);
- [AI SDK custom transport documentation](https://ai-sdk.dev/docs/ai-sdk-ui/transport);
- [AI SDK UI-message stream documentation](https://ai-sdk.dev/docs/reference/ai-sdk-ui/create-ui-message-stream); and
- [AI Elements setup documentation](https://elements.ai-sdk.dev/docs/setup).
