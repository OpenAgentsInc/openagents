# OpenAgents Desktop React Codex Workbench — revision 3 assurance gates

Date: 2026-07-14

ProductSpec: `openagents-desktop-mvp-phase-2-react-codex-workbench.product-spec.md`
revision 3. Owner disposition: accepted for RC16 candidate construction and
verification; publication remains separately gated.

## Reference boundary

The read-only T3 Code snapshot at
`projects/repos/t3code@c1ec1915fc16f3dc1ec5d47d9a97f6210a574526` is the
interaction reference for authored-message hierarchy, compact work logs,
settled-turn disclosure, active streaming presentation, composer continuity,
and session navigation. OpenAgents does not copy its state authority or make it
a runtime dependency. Effect and Effect Native retain state, intent, protocol,
theme, and host authority.

## Conversation gates

1. **Authored messages are primary.** User messages render as restrained
   right-aligned bubbles. Assistant messages render as unboxed Markdown prose.
   Labels, timestamps, and event controls are absent at rest and available only
   as secondary inspection affordances.
2. **The transcript is not an event ledger.** Session, context, metadata,
   connection, lifecycle-success, and token-accounting events never render as
   conversation messages. Token data remains available through the bounded
   message inspector.
3. **Internal work is compact.** Reasoning, tools, approvals, and collaboration
   render as one-line work entries with label, bounded preview, and textual
   status. Raw inputs and results are collapsed by default and bounded when
   expanded.
4. **Settled work folds; active work stays legible.** Consecutive completed work
   folds behind one `Worked · N activities` disclosure. During a run, prior work
   folds while the latest active item and a quiet working indicator remain
   visible.
5. **Streaming is stable.** Same-key deltas update in place, do not duplicate
   rows, follow only at the live edge, preserve a manual reader's position, and
   announce bounded activity without reading every token or stealing focus.
6. **Loss and failure remain truthful.** Actual authored redaction, gaps,
   interruptions, and failures remain visible. A reasoning-only persistence
   redaction marker is absence of reasoning, not a red failure card.

## Shell and interaction gates

7. **The session rail scrolls at its real boundary.** The shadcn `ScrollArea`
   owns a bounded flex height and viewport scrolling; session rows do not create
   a competing nested scroller. Selection and keyboard traversal remain stable.
8. **The composer remains continuous.** Streaming and work disclosure do not
   remount the composer, drop the first keystroke, break IME composition, or
   change Send/Stop/Steer/Queue command identity.
9. **Accessibility is behavioral.** Keyboard operation, disclosure state,
   focus restoration, screen-reader names, reduced motion, 24 CSS-pixel minimum
   targets, WCAG 2.2 AA contrast, and the 760 × 520 minimum window are required.
10. **Khala color authority is preserved.** The approved dark Khala semantic
    tokens remain canonical. shadcn/Base UI consume token aliases and introduce
    no independent zinc, state, typography, radius, or motion system.

## Release gates

RC16 must pass the complete Node 24 Desktop verification suite, the focused
revision-3 hierarchy oracles, the 18-obligation admitted Phase 1 assurance run,
production build, signed hardened-runtime packaging, Apple notarization,
stapler/Gatekeeper/deep-signature checks, mounted-DMG React smoke, monotonic
RC15-to-RC16 update with interruption recovery, downgrade refusal, rollback,
diagnostic export, uninstall, reinstall, cleanup, and zero-owner teardown.

The receipt must bind the exact source commit, ProductSpec digest, artifact
bytes and SHA-256, Apple submission result, test counts, residuals, and all
AC-1 through AC-14 dispositions. A passing candidate does not publish a feed,
tag, GitHub release, artifact, or public claim.
