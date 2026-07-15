# OpenAgents Desktop MVP phase 2 implementation log

- ProductSpec: `openagents-desktop-mvp-phase-2-react-codex-workbench.product-spec.md`
- Parent issue: [#8817](https://github.com/OpenAgentsInc/openagents/issues/8817)
- Started: 2026-07-14
- Rule: packets land sequentially on `main`; this log records the shipped
  boundary, verification, and remaining compatibility scope after each packet.

## MVP-02A — React projection boundary (#8818)

Status: implemented and verified.

Canonical Effect Native revision:
`086378e03b2546d39a85b6b74ac1269e8587b23b`.

Delivered:

- one React 19 root and one Scope-owned Effect stream subscription;
- stable synchronous snapshots consumed through `useSyncExternalStore`;
- explicit, mutually exclusive `react` and `compatibility` surface backends;
- ordinary semantic React lowerings for Stack, Text, Button, Card, Spacer, and
  Divider, preserving keys, bounded a11y, typed style tokens, and existing
  `IntentReporter` identities;
- public loading, failed, and incompatible states with React error recovery;
- shared canonical token/component stylesheet for both DOM backends;
- exact React/React DOM pins in canonical Effect Native and app-owned dedupe in
  Desktop;
- Tailwind default theme namespaces disabled; its semantic aliases derive only
  from canonical `--en-*` variables;
- an explicit Desktop compatibility selection until later packets cover the
  complete workbench subset; and
- invariant, vendor, import-boundary, and backend-selection guards.

Verification:

- `bunx tsc -b packages/render-dom --pretty false`
- `bun test packages/render-dom/test` — 113 passed
- vendored render-dom typecheck and focused React/vendor/Desktop boundary tests
- Desktop typecheck and complete suite — 133 files passed, 1,312 tests passed,
  39 skipped
- production build — `boot.js` 819.22 kB and `app.css` 11.68 kB
- built Electron smoke and reload path — all checks passed; lifecycle teardown
  reported zero active owners

Remaining compatibility boundary:

- Desktop intentionally selects `backend: "compatibility"` for its full
  catalog. MVP-02B through MVP-02E expand declared React lowerings by product
  slice before the integrated MVP-02F proof can select React for the complete
  retained workbench.

## MVP-02B — React workbench shell and session rail (#8819)

Status: implemented; the scoped React path is available with
`?renderer=react-shell`. The complete app stays on the compatibility default
until MVP-02F because the timeline, composer, decisions, and review surfaces
are intentionally owned by their following packets.

Canonical Effect Native revision:
`ec04d1a066d6f3ed0c67735ba451cfc90a343aa8`.

Delivered:

- a generic value parameter on the existing Scope-owned React external store,
  preserving the `View` default while allowing Desktop to consume the
  authoritative `DesktopShellState` without a React-owned domain store;
- ordinary React `WorkbenchShell`, `SessionRail`, and `ConversationHeader`
  components with one React root, Strict Mode, shared failure containment, and
  one Effect subscription;
- metadata-first globally ordered local and Codex session rows, deterministic
  deduplication, selection, search, load-more, and honest scanning/empty states;
- existing intent identities for new, select, resume/open, archive, two-step
  delete, recovery, and catalog pagination;
- lifecycle and bounded repository context in the conversation header, without
  provider/account/model controls or absolute paths;
- a below-980-pixel overlay rail with Escape close, focus restoration, visible
  focus treatment, keyboard row traversal, reduced-motion compatibility, and a
  760-by-520 supported minimum; and
- renderer-private CSS derived only from canonical Effect Native variables.

Runtime clarification:

- Phase 2 does not use or plan to add the Vercel AI SDK. React owns
  presentation only; the existing Codex Runtime Gateway, compatible app-server,
  Effect services, and typed intent registry retain streaming, tools, sessions,
  and command authority.
- The pinned T3 Code reference likewise declares no `ai` or `@ai-sdk/*`
  dependency. Its coding-provider path uses Effect client runtime/contracts and
  a generated `effect-codex-app-server` integration (plus other provider
  adapters), so React adoption does not imply AI SDK adoption.

Verification:

- canonical React store focused tests — 5 passed;
- canonical Effect Native TypeScript build — passed;
- canonical full suite — 639 passed, with two unrelated committed visual
  baseline mismatches (`counter-phone` and `counter-desktop`) retained as a
  transparent repository gate caveat;
- Desktop typecheck — passed;
- Desktop complete suite against the rebuilt artifact — 134 files passed,
  1,319 tests passed, 39 skipped;
- focused typography and release-preflight gates — 19 passed, including the
  production-bundle local-path rejection;
- production build — `boot.js` 888.28 kB and `app.css` 167.72 kB (the CSS now
  contains the checked-in shadcn utilities and locally bundled variable font
  faces);
- wide, narrow, and narrow-open-overlay headed visual proofs confirmed the
  scoped React shell, responsive rail, scrim, and initial search focus; and
- ProductSpec validation passed under both OpenAgents and upstream profiles
  (the repository ProductSpec suite passed 102 tests), and the built Electron
  smoke completed every compatibility-path check with zero active lifecycle
  owners after teardown.

## Owner-directed shadcn component extension (#8824)

Status: integrated into the React shell packet.

Preset:

- code `b3Zg9L0M8A`;
- `base-vega` style with zinc base, blue theme, cyan chart intent;
- Oxanium variable body font and Geist variable heading font;
- small radius, Lucide icons, subtle menu accent, default-translucent menu;
- pointer cursor enabled.

The exact Vite initializer initially rejected the custom Electron package
because it lacked a conventional `vite.config.ts`. Desktop now exposes one
conventional config shared by its production build and component tooling; the
same exact initializer then completed successfully. Modern package-import
aliases (`#components`, `#lib`, `#hooks`) keep generated source resolvable by
Vite, tests, and TypeScript without the deprecated `baseUrl` option.

The preset is implemented as a Khala extension, not a second theme:

- generated Button, Input, ScrollArea, and Separator source lives under
  `apps/openagents-desktop/src/components/ui`;
- the React shell prefers those components for its controls, search, scrolling,
  and separation;
- `shadcn-khala.css` retains preset fonts, shape, menu behavior, animation, and
  component utilities but maps background, foreground, surface, primary,
  secondary, muted, accent, destructive, border, focus, chart, radius, and
  sidebar semantics onto canonical `--en-*` roles; and
- a conformance oracle rejects independent `oklch(...)` or hex palette values
  in that extension.

The Vercel AI SDK remains absent. shadcn changes the React component source
layer only; Codex Runtime Gateway/app-server and Effect authorities are
unchanged.

The typography assurance contract was extended in the same packet: Oxanium is
the approved body/UI family, Geist is the approved heading family, system
families remain resilient body fallbacks, and generic monospace remains the
code fallback. Its recursive source oracle rejects any other family or font
shorthand. The shadcn palette remains a semantic alias layer over Khala's
canonical Effect Native variables rather than an independent zinc/blue theme.

## MVP-02C — Typed React conversation timeline (#8820)

Status: implemented on the scoped `?renderer=react-shell` path. The complete
app remains on the compatibility default until MVP-02F.

Delivered:

- ordinary React timeline and item components over the existing bounded
  `CodexHistoryItem` and local `DesktopNoteEntry` projections, with no provider
  event parser or React-owned transcript array;
- stable authoritative item-ref keys, sequence ordering, duplicate collapse,
  and assistant segments that remain separated by display-bearing non-text
  records;
- in-place tool invocation/result correlation at the invocation key;
- distinct text, reasoning, plan, tool, approval, usage, metadata, context,
  error, gap, redaction, lifecycle, terminal, and local-question treatments;
- exactly one newest authoritative terminal disposition when a recovered
  prefix contains superseded terminal lifecycle records;
- the existing bounded Markdown parser lowered to safe React text, headings,
  emphasis, lists, quotes, and code, with links remaining inert visible text
  and no HTML injection surface;
- per-item error containment that reports an unavailable presentation item
  without fabricating completion or dropping sibling rows;
- a pre-mutation `getSnapshotBeforeUpdate` anchor receipt plus pre-paint
  correction for variable-height prepends and offscreen height changes;
- manual-reader position preservation, bounded live-edge following, a shadcn
  new-activity affordance, top/bottom typed pagination intents, atomic session
  replacement, and bounded live-region summaries; and
- a 500-item maximum-page projection/render/update/teardown corpus. It passed
  without a virtualizer, retained exactly 500 keyed rows through an in-place
  stream update, and removed every row on unmount, so no virtualization
  dependency was added.

The React module receives only the same bounded/redacted typed projection the
compatibility renderer already receives. Effect services, Runtime Gateway,
history completeness, persistence, terminal truth, and intent identities are
unchanged; Vercel AI SDK remains absent.

Verification:

- Desktop typecheck — passed;
- focused timeline, shell, renderer-boundary, design-conformance, and release
  preflight suites — 74 tests passed;
- production build — `boot.js` 898.23 kB and `app.css` 171.84 kB;
- full Desktop suite — 136 files passed, 1,333 tests passed, 39 skipped; and
- ProductSpec validation passed under OpenAgents and upstream profiles, its
  repository suite passed 102 tests, and built Electron compatibility smoke
  passed with zero active lifecycle owners after teardown.
## MVP-02D — React composer, commands, and decisions (#8821)

Status: implemented on the scoped `?renderer=react-shell` path. Effect remains
the prompt, command, interaction, runtime, and reconciliation authority.

Delivered:

- a controlled shadcn Textarea composer that focuses a ready session, captures
  the first keystroke, blocks Enter while an IME composition is active,
  preserves Shift+Enter, and grows from 64px to a 180px internal-scroll cap;
- the existing Send, Stop, Steer, Queue, pending-mode, and input intents with
  an unavailable-lane explanation and no provider, model, account, reasoning,
  permission, attachment, plugin, MCP, or voice controls;
- a shadcn Command/Dialog palette over the canonical Desktop command registry,
  including registry-id search, platform chords, availability gating,
  no-results/status output, keyboard navigation, Escape dismissal, and focus
  restoration supplied by the reviewed source components;
- modal question, tool-approval, and plan-review surfaces correlated by the
  exact `questionRef`, with complete option descriptions, explicit close
  semantics, read-only unavailable state, pending/submitting/failed copy, and
  no presentation-side inference of acceptance or rejection;
- an Effect-owned in-flight marker before the typed answer bridge call. Rapid
  duplicate choice/submit intents therefore produce one bridge effect, a
  refused or failed bridge call returns to retryable pending with visible
  failure, and only a confirmed bridge handoff marks the local submission;
- resolved/expired/revoked/timeout/denied decision status retained in the
  typed timeline after the modal is gone; and
- renderer-boundary invariants expanded only for the React composer host and
  its ephemeral IME, focus, overlay, and palette-query mechanics.

Dependency and boundary receipt:

- `cmdk@1.1.1` is the only runtime dependency added by the generated shadcn
  Command source. Its input is the closed local command registry; it receives
  no host object, arbitrary command callback, provider payload, credential,
  filesystem path, or model output. Dialog framing remains the preset's Base
  UI source component. The component-source oracle permits only reviewed UI
  imports and continues to reject Effect, Electron, Node, and Desktop bridge
  authority under `src/components/ui`.
- Production output moved from MVP-02C's `boot.js` 898.23 kB / `app.css`
  171.84 kB to `boot.js` 1,007.56 kB / `app.css` 185.51 kB: +109.33 kB JS and
  +13.67 kB CSS before packaging compression. This is accepted for the MVP
  because it supplies the requested shadcn command/dialog keyboard and focus
  behavior; later bundle work may replace the implementation behind the same
  source-component and typed-intent boundary.

Focused verification passed Desktop typecheck plus 151 composer, decision,
timeline, shell, renderer-boundary, and command tests (11 skipped). It covers
first focus, height bounds, IME Enter, Shift+Enter, rapid duplicate send,
Send/Stop/Steer/Queue identity, canonical palette identity, modal approval,
failed decision presentation, and concurrent typed-bridge duplicate
admission. The full Desktop suite passed 137 files / 1,339 tests (39 skipped),
ProductSpec validated under both OpenAgents and upstream profiles and its
repository suite passed 102 tests, the production build emitted the bundle
sizes above, and built Electron compatibility smoke completed with zero active
lifecycle owners after teardown. The production audit passed the high-severity
gate; its one moderate finding is the pre-existing Expo/mobile `uuid` path,
not `cmdk` or this Desktop dependency path. The installed whole-surface React
turn/decision journey remains the explicit MVP-02F cutover receipt rather than
being inferred from the compatibility smoke.

## MVP-02E — React review and truthful recovery states (#8822)

Status: implemented on the scoped `?renderer=react-shell` path. Installed
whole-surface review/recovery proof remains part of MVP-02F.

Delivered:

- a shadcn-triggered repository review surface over the existing Effect-owned
  Git projection: adjacent drawer at 1120 CSS pixels and above, Base UI Sheet
  below that threshold, independent transcript/review scroll, explicit close,
  focus return, and reduced-motion behavior;
- relative-path file rows and exact diff requests using the existing correlated
  repository/status snapshot authority, without exposing repository refs,
  absolute roots, host objects, or ambient paths;
- semantic addition/deletion labels that do not rely on color, exact hunk and
  causal-item presentation, and a visible read-only boundary with no edit,
  apply, stage, discard, commit, branch, push, PR, terminal, or arbitrary Git
  affordance;
- stable typed refusal presentation for every `GitGithubErrorCode`, including
  stale snapshot, unsafe/conflicting state, invalid path, binary, secret-shaped,
  oversized, unavailable, authentication, and not-found results; the Effect
  Git loop now retains the refusal code separately from bounded display copy;
- a bounded `runtimeFailure` disposition on the Effect-owned shell state. Local
  runtime adapters preserve signed-out, incompatible-workflow, offline,
  interrupted, quota, rate-limit, and policy outcomes instead of asking React
  to classify error strings; unknown failures remain explicitly `failed`;
- React StatusNotice projection for those dispositions, canonical lane
  availability, unavailable workspace grants, durable stream gaps, and
  interrupted/errored history agents; only existing typed Settings/workspace
  actions are offered; and
- Alert, Badge, and Sheet source components generated from the locked shadcn
  preset and styled through the dark-zinc Khala token extension.

The recovery authority itself was not moved. Renderer reload, exact-prefix app
restart, at-most-once continuation or explicit interruption, repair before live
resubscription, bounded diagnostics, and Open in Codex custody remain the
existing Effect/main/runtime contracts. React only lowers their public typed
state. The compatibility backend remains the installed default until MVP-02F,
and specialist settings, diagnostics, file/editor, fleet, ProductSpec, child,
and terminal surfaces remain compatibility-owned.

The Vercel AI SDK remains intentionally absent: it would duplicate the current
Codex Runtime Gateway/app-server stream, tool, approval, and persistence
authority. shadcn/Base UI is the presentation layer; Effect remains state and
effects authority.

## MVP-02F — Default React cutover and integrated receipt (#8823)

Status: implementation cutover, signed release-boundary proof, and exact
commit-pinned revision-1 owner admission complete; release-proof issue remains
open for real-account, accessibility/performance, and independent-review
evidence.

Ordinary Desktop launches now install the React workbench by default. The
catalog renderer remains available only through the explicit exclusive
`?renderer=compatibility` selection and is used by the retained broad
specialist-surface smoke. A separate production-built `smoke:react` journey
proves one React root, first Chromium input, exact Effect intent payloads, new
local chat, typed Codex fixture turn, bounded real Git diff, backend
exclusivity, renderer reload, exact durable prefix restoration, Runtime
Gateway continuity, and zero-owner teardown.

The cutover smoke caught two genuine contract defects: value-bearing React
commands lacked `ComponentValueBinding`, and local chats reloaded from provider
history instead of their own durable store. Both were corrected at the
authority boundary and regression tests now resolve React refs with the real
Effect Native resolver rather than recording the unvalidated second argument.

The normalized receipt and explicit non-claims live in
`docs/mvp/2026-07-14-openagents-desktop-react-cutover-receipt.md`.

### ProductSpec admission and RC14 source freeze

The owner directly admitted the commit-pinned revision 1 of the OpenAgents
Desktop React Codex Workbench ProductSpec as the acceptance contract for this
MVP candidate. The immutable authority record is
`docs/mvp/2026-07-14-openagents-desktop-react-productspec-admission-receipt.md`,
bound to commit `de1180b2da937922c2a8724915cf761f8fb78617` and SHA-256
`b88456951753e5a69b9a2390ad18d0fdecd1e1fbfcf65f2f6ddd7a5f1f060d41`.

Later shadcn and explicit Vercel AI SDK scope additions had changed the
canonical file without changing its revision. That identity defect is now
reconciled by marking the current expanded document as proposed revision 2;
the admission is not silently transferred. The disposition authorizes RC14
construction and evaluation against the pinned revision 1. It does not accept
an installed candidate, waive evidence, admit an AssuranceSpec, or authorize
publication.

The RC14 source-bound assurance rerun also exposed an intermittent React shell
test race under the complete Desktop gate: a confirm-delete rerender could be
asserted after the test restored its DOM globals. The harness now owns every
root, wraps renders and interactions in React `act`, and unmounts before global
restoration. Typecheck passed, the focused shell test passed 20 consecutive
runs, and the complete assurance rerun then returned all 18 observations and
18 falsifiers green with the full Desktop build and smoke gate.

### Provider-originated decision closeout (#8821)

The decision boundary now follows the reference architecture in
`projects/repos/t3code/apps/server/src/provider/Layers/CodexSessionRuntime.ts`
and its orchestration integration harness: the provider originates a
correlated request, the runtime holds the pending continuation, the UI returns
the method-correct response for that exact request, and provider completion is
causally withheld until the answer arrives.

OpenAgents proves that lifecycle through the production-built Electron/React
path with a protocol-speaking app-server peer installed only at the process
spawn seam. It originates command-approval request `91`; the ordinary
main-process pending registry projects the decision through the existing
Fable-local event envelope, IPC bridge, Effect intent runtime, and React
dialog. Clicking `Approve` returns `{ decision: "accept" }` to request `91`.
The peer emits command completion, assistant text, usage, and turn completion
only afterward. The host-side receipt asserts the correlation and causal
ordering, while reload restoration and zero-owner teardown run after the
decision reconciles.

The peer itself is covered against the real app-server client, and the
production runtime suite separately proves the native request/answer mapping.
This is deterministic integrated evidence for the installed authority path;
it is not represented as a live-account or private-provider receipt, which
remains a separate #8823 gate.

Exact `origin/main` commit `a66b8d4ea7` was then packaged as the ARM64 RC13
candidate. Apple accepted the app and outer DMG; Gatekeeper, stapler,
`syspolicy_check`, DMG verification, and a read-only-DMG pristine-profile
React smoke passed. The real update client/applier also passed the exact
notarized RC12-to-RC13 interrupted-stage, atomic update, downgrade refusal,
rollback, diagnostics, uninstall, reinstall, and cleanup sequence without
deployment. The immutable artifact identity and honest remaining boundary live
in `docs/mvp/2026-07-14-openagents-desktop-react-rc13-release-receipt.md`.

### Reference-shaped dev loop and RC15 candidate

The desktop development command now follows the reference's productive split:
Vite owns the React renderer loop and Fast Refresh on one strict loopback
origin, while Electron retains the `openagents-app://renderer` security origin
and proxies assets only when unpackaged. The runner builds host artifacts,
waits for Vite to listen, launches Electron with an isolated `OpenAgents Dev`
profile, and owns both lifecycles. Production remains static and signed.

During exact release acceptance, the updater cleanup was hardened to detach
the whole disk device reported by `hdiutil attach` (with a forced-detach
fallback) rather than relying only on a temporary mount path. RC14 was not
promoted after that cleanup concern surfaced. The corrected exact
`origin/main` source at `9bb0bbb94909f5b0b3e371972335a7c4df850a44` became
RC15.

RC15 passed the complete Node 24 Desktop gate (140 files, 1,347 tests), all 18
assurance candidates and falsifiers, Apple notarization, post-staple
Gatekeeper/integrity checks, read-only-DMG React decision/reload smoke, and the
exact published-RC13 to RC15 interrupted-stage/update/downgrade-refusal/
rollback/diagnostics/uninstall/reinstall/cleanup sequence. No feed or artifact
was published. The exact receipt is
`docs/mvp/2026-07-14-openagents-desktop-react-rc15-release-receipt.md`.

### Post-RC15 conversation hierarchy correction

Owner review rejected the release candidate's event-log-shaped transcript.
The React timeline now follows the pinned reference's core hierarchy instead:
authored user and assistant messages are primary prose, tool/reasoning/approval
activity is a compact one-line work log with explicit disclosure, consecutive
settled work folds behind one `Worked · N activities` row, and an active run
keeps only its latest work visible beside a bounded working indicator. Raw tool
arguments and results are collapsed by default.

Session, context, metadata, and token-accounting records no longer render as
conversation messages, and a redacted reasoning placeholder is treated as
absent reasoning rather than a red failure card. Actual authored redaction,
gaps, interruptions, and failures remain visible. The session rail's shadcn
`ScrollArea` now owns the bounded flex height and viewport scrolling, fixing
the non-scrolling rail. Existing item-key streaming replacement, live-edge
following, manual-reader anchoring, paging, Markdown safety, and Effect-owned
state/intent boundaries remain intact.

This owner-directed correction changes AC-5 and is recorded as proposed
ProductSpec revision 3. It is post-RC15 source work; it does not relabel the
RC15 artifact or claim a new release candidate.
