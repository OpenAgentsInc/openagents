# T3 Code UI full harvest — accepted plan and work-packet ledger

- Class: accepted plan and implementation admission
- Date: 2026-07-17
- Owner authority: current owner conversation
- Program: T3 Code UI full harvest
- Status: active
- Source: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- License: MIT
- Target: `apps/openagents-desktop`
- Baseline: `OpenAgentsInc/openagents@544bb2a2e00421984d415fc1d81d70e530bef05f`

## Owner direction

> For our desktop app pull in all components from T3 Code, hook them up to
> tool calls, make them look exactly like T3's app while adapting them to our
> styles, define the Fast Follow program, and execute it immediately.

This is separate target authority under the Fast Follow invariant. It admits a
complete target-native port of the current T3 Code UI jobs, component families,
interaction states, responsive compositions, and keyboard paths into the
mounted OpenAgents Desktop React workbench. GitHub issues are reserved for
concrete reproducible bugs, so this accepted plan and its bounded packet ledger
are the implementation authority.

“All components” is a completion requirement, not permission for an unbounded
copy. Every component in the pinned T3 `apps/web/src/components`, `browser`,
`cloud`, and UI-support inventory must receive an explicit terminal disposition:

- **adopt** when OpenAgents needs the same visible user job;
- **adapt** when the visible job remains but its state, provider, or authority
  must be OpenAgents-native;
- **covered** when a mounted OpenAgents component already meets the parity
  definition; or
- **reject** only when the component is T3 branding, a T3 service/account
  boundary, light/system theming, or an authority OpenAgents intentionally does
  not expose. Rejection requires a visible OpenAgents alternative or a written
  product reason; omission is not a disposition.

The source is MIT-licensed. Substantial copied code retains the required T3
copyright/license notice in the applicable third-party notice. Source identity,
component provenance, and adaptation notes stay reviewable by exact revision.

## Fidelity contract

“Exactly like T3, adapted to our styles” means:

- preserve T3's information architecture, component composition, density,
  geometry, hierarchy, interaction sequence, responsive mode, keyboard path,
  loading/empty/error/success families, and motion timing where the job matches;
- project those recipes through OpenAgents' Khala tokens, fonts, icon catalog,
  focus language, reduced-motion policy, and semantic radius taxonomy;
- preserve OpenAgents Queue/Steer/Stop, Full Auto, ProductSpec, AssuranceSpec,
  disclosure, redaction, and typed refusal semantics;
- connect every enabled action to an existing or newly admitted typed intent and
  host capability. A decorative control, fabricated result, raw renderer shell,
  ambient filesystem access, or string-routed tool selector is not parity; and
- count only the current mounted React product. Dormant components, static
  fixtures, and legacy Effect-Native views are implementation inputs, not
  completion evidence.

The authoritative gap and component-family ledger is
[`2026-07-17-t3-code-openagents-desktop-ui-gap-analysis.md`](../teardowns/2026-07-17-t3-code-openagents-desktop-ui-gap-analysis.md),
especially sections 14–16. The full-product orchestration gaps remain in the
companion
[`2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md`](../teardowns/2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md).

## Ordered program

1. **T3UI-01 — transcript message composition:** finish the primary user and
   assistant rows, long-message treatment, metadata/actions, typed work and
   notice families, settled/streaming distinctions, and scroll-stable folds.
2. **T3UI-02 — transcript navigation and scale:** add turn-level folds and
   duration, virtualized stable rows, minimap navigation, new-turn anchoring,
   persistent/free-scroll modes, history prepend retention, and 500-row proof.
3. **T3UI-03 — composer parity:** port contextual nodes, file/folder/skill and
   command menus, compact responsive controls, inline decisions, plan follow-up,
   images, and mode states while preserving Queue/Steer and Full Auto.
4. **T3UI-04 — project/worktree shell:** port project-grouped threads, statuses,
   sorting/manual order, multi-select, header branch/worktree/Open controls, and
   collision-safe project/worktree creation and selection.
5. **T3UI-05 — surface manager and tab strip:** port T3's tab lifecycle,
   add/activate/close/close-others/close-right/maximize/resize behavior, persist
   layout, and expose only capability-backed surfaces.
6. **T3UI-06 — files and rich diff:** mount changed-file tree, file browser,
   file tabs, search, exact rich diff, annotations, and read/edit/save conflict
   paths through one-shot WorkContext authorities.
7. **T3UI-07 — terminal workbench:** mount generation-owned persistent PTY tabs,
   labels, resize, replay, exit, close, and context attachment with bounded
   lifecycle cleanup and no renderer shell capability.
8. **T3UI-08 — browser preview:** mount local-server discovery, URL/file
   preview, browser chrome, device sizing, failures, automation cursor,
   recording evidence, and typed annotation round trips.
9. **T3UI-09 — settings and primitive family:** converge routed settings,
   reusable rows, reset/skeleton/conflict states, provider/source-control/
   keybinding/diagnostic/account panels, and menu/popover/select/combobox/table/
   toast/tooltip/sheet primitives through the OpenAgents design system.
10. **T3UI-10 — remote/mobile/connect surfaces:** adapt T3 pairing, environment,
    relay, SSH prompt, connection, and mobile-client management UI to admitted
    OpenAgents placement and portable-session contracts; do not copy T3 service
    authority or credentials.
11. **T3UI-11 — responsive, accessibility, and performance closure:** prove
    wide/standard/minimum window modes, off-canvas/sheet behavior, focus order,
    keyboard alternatives, contrast, reduced motion, large diff/thread/tab
    budgets, disconnects, and every meaningful unavailable/error state.
12. **T3UI-12 — component census and installed evidence:** dispose every pinned
    T3 component, capture the mounted fixture catalog, run visual comparison,
    perform the packaged Desktop journey, and publish the signed-build evidence.

Packets are sequential only where they share hot state or authority. Independent
primitive and fixture work may proceed concurrently under collision-safe claims.
No packet may claim “all components” or T3 parity until T3UI-12 proves the exact
pinned census has no undisposed component and the mounted product satisfies the
parity definition.

## Active packet — T3UI-01

Outcome: make the primary transcript read and behave like T3 before expanding
side panels: complete the ordinary user/assistant message composition and keep
reader position stable when variable-height transcript details open or close.

Owned paths:

- `apps/openagents-desktop/src/renderer/react-timeline.tsx`
- `apps/openagents-desktop/src/renderer/react-timeline.test.tsx`
- `packages/ui/src/desktop-workbench.css`
- this plan, the Sol roadmap, and generated Sol document manifest

Required behavior:

- user rows keep T3's right-aligned 80% bubble and hover/focus metadata row;
- prompts beyond eight lines or 600 characters collapse with fade and an
  accessible `Show full message` / `Show less` control;
- settled assistant rows expose copy, timestamp, and exact history details in a
  quiet hover/focus metadata row, while streaming rows do not offer a stale copy;
- opening/closing long prompts and settled work groups preserves the reader's
  visual position when they are away from the live edge;
- live-edge following, manual-scroll hold, Jump to latest, prepend anchoring,
  typed older/newer paging, same-key streaming replacement, stable item keys,
  `content-visibility`, and the existing 500-row budget remain proved;
- two or more user turns expose a compact keyboard-operable minimap whose
  labels include user and assistant previews and whose navigation releases
  live-edge following without stealing focus; and
- typed plan, reasoning, command, file, tool, agent, approval, notice, refusal,
  redaction, working, and waiting rows remain distinct rather than flattening
  into assistant prose.

Proof: focused transcript projection/rendering/scroll/performance tests,
Desktop typecheck, the Sol document guards, production build/fixture smoke where
available, and rendered short/long/streaming/long-thread visual inspection.

Close rule: this packet closes ordinary message composition, scroll-stable
variable-height disclosure, and basic turn minimap navigation only. Turn
folds/duration, measured full virtualization, rich contexts/images/revert,
all-component harvest, installed journey, signed release, and T3 parity remain.

### CLAIM

- actor/session: `codex-t3ui-01-20260717`
- base: `544bb2a2e00421984d415fc1d81d70e530bef05f`
- worktree/branch: `.worktrees/openagents-t3-ui-20260717` / detached `origin/main`
- scope: transcript message composition and scroll-stable disclosures
- claimed_at: `2026-07-17`

### CLAIM-STATUS

- implementation: long-prompt collapse, terminal-assistant metadata,
  scroll-stable prompt/work disclosure, and basic turn minimap are mounted
- focused proof: 48 transcript tests and 19 Sol document tests passed; shared
  UI typecheck and Desktop production build passed
- visual proof: 16 states captured; 10 passed the checked-in threshold and six
  exposed stale transcript/composer baseline pixels, which were not rewritten
- Desktop typecheck passed after reconciliation with the landed conversation-
  catalog status fix; the final full sweep passed 1,989 with 39 skipped
- receipt: [`2026-07-17-t3ui-01-transcript-message-composition-receipt.md`](../fastfollow/receipts/2026-07-17-t3ui-01-transcript-message-composition-receipt.md)
- residual: exact turn folds/duration, measured full virtualization, anchored
  new-turn space, minimap viewport state, rich contexts/images/revert, and
  installed long-thread evidence remain T3UI-02 work

## Explicit non-authority

This program grants no deployment, release, signing, credential use, paid
provider spend, settlement, public-promise, cross-tenant publication, external
source mutation, or invariant bypass. It does not authorize copying T3 account,
cloud, relay, remote-execution, or update authority into OpenAgents. Those jobs
must bind to admitted OpenAgents contracts or remain honestly unavailable.
