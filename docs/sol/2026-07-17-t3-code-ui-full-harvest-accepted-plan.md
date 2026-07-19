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

- **adopt** when OpenAgents needs the same visible user job.
- **adapt** when the visible job remains but its state, provider, or authority
  must be OpenAgents-native.
- **covered** when a mounted OpenAgents component already meets the parity
  definition. Or
- **reject** only when the component is T3 branding, a T3 service/account
  boundary, light/system theming, or an authority OpenAgents intentionally does
  not expose. Rejection requires a visible OpenAgents alternative or a written
  product reason. Omission is not a disposition.

The source is MIT-licensed. Substantial copied code retains the required T3
copyright/license notice in the applicable third-party notice. Source identity,
component provenance, and adaptation notes stay reviewable by exact revision.

## Fidelity contract

“Exactly like T3, adapted to our styles” means:

- preserve T3's information architecture, component composition, density,
  geometry, hierarchy, interaction sequence, responsive mode, keyboard path,
  loading/empty/error/success families, and motion timing where the job matches.
- project those recipes through OpenAgents' Khala tokens, fonts, icon catalog,
  focus language, reduced-motion policy, and semantic radius taxonomy.
- preserve OpenAgents Queue/Steer/Stop, Full Auto, ProductSpec, AssuranceSpec,
  disclosure, redaction, and typed refusal semantics.
- connect every enabled action to an existing or newly admitted typed intent and
  host capability. A decorative control, fabricated result, raw renderer shell,
  ambient filesystem access, or string-routed tool selector is not parity. And
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
    OpenAgents placement and portable-session contracts. Do not copy T3 service
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

## Active packet — T3UI-12

Outcome: account for the exact pinned T3 component denominator and prove the
mounted catalog plus the strongest locally authorized packaged Desktop journey.

Owned paths:

- `apps/openagents-desktop/t3-component-census.json`
- `apps/openagents-desktop/src/t3-component-census.test.ts`
- `apps/openagents-desktop/visual-baselines/manifest.json`
- this plan, the packet receipt, and generated Sol document manifest

Required behavior:

- every non-test `.tsx` component below the pinned T3 component root has its
  exact source path and Git blob id recorded once.
- every row has one explicit adapted, covered, or rejected disposition,
  existing OpenAgents evidence, and valid mounted fixture references.
- the checked mounted fixture catalog is identical to the deterministic visual
  baseline lane. And
- local packaging proves staged-source isolation and packaged-content closure,
  while signing, notarization, and release promotion remain separate authorities.

Proof: exact 151-row source census oracle, existing-path and fixture checks,
zero-drift mounted visual catalog, unsigned-development packaged-app journey,
Desktop typecheck, full serial suite, production build, Electron fixture smokes,
Sol guards, and publishing.

Close rule: this packet closes only when the census has no undisposed component
and the local packaged candidate runs. It cannot claim signed-release or
cross-platform distribution evidence without those separate ceremonies.

### CLAIM

- actor/session: `codex-t3ui-12-20260717`
- base: `e58ae1ba2f`
- worktree/branch: `.worktrees/openagents-t3-ui-20260717` / detached `origin/main`
- scope: exact pinned census, mounted catalog, and local packaged journey
- claimed_at: `2026-07-17`

### CLAIM-STATUS

- implementation: complete and published. The exact 151-row census contains 99
  adapted, 46 covered, 6 explicitly rejected, and zero undisposed components
- focused proof: 216 Desktop files / 2,079 passing / 39 skipped, production and
  both source Electron smokes, the packaged canonical React journey, exact
  packaged Codex identity, and all 24 zero-drift visual frames pass
- packaged source: `1f42cd2998b352575f3092e0ae5b88e5190e1fad`. Staged ledger
  `sha256:6295cbd544c445835c3f1d1bf9484eedef8dac600373cee26a82ea63288b4b5c`
- receipt: [`2026-07-17-t3ui-12-census-packaged-evidence-receipt.md`](../fastfollow/receipts/2026-07-17-t3ui-12-census-packaged-evidence-receipt.md)
- residual: the unsigned-development package is not signed-release evidence.
  current signing/notarization/promotion, cross-platform artifacts, and one
  compatibility-renderer packaged editor-recovery miss remain explicit

## Explicit non-authority

This program grants no deployment, release, signing, credential use, paid
provider spend, settlement, public-promise, cross-tenant publication, external
source mutation, or invariant bypass. It does not authorize copying T3 account,
cloud, relay, remote-execution, or update authority into OpenAgents. Those jobs
must bind to admitted OpenAgents contracts or remain honestly unavailable.
