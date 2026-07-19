# OpenAgents Desktop React Codex Workbench — revision 4 assurance gates

Date: 2026-07-15

ProductSpec: `openagents-desktop-mvp-phase-2-react-codex-workbench.product-spec.md`
revision 4. Lifecycle: proposed. Revision 3 remains the accepted RC16
construction identity. This revision has no inherited admission, release, or
publication authority.

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
4. **Settled work folds. Active work stays legible.** Consecutive completed work
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
   owns a bounded flex height and viewport scrolling. Session rows do not create
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
11. **Navigation history stays authoritative.** Back and Forward reflect one
    bounded Effect-owned stack, record only successful opens, deduplicate
    adjacent destinations, preserve forward history until a successful branch,
    and traverse local, Codex-history, coding-session, and admitted workspace
    destinations without React or `window.history` ownership. Disabled means
    no reachable target. Enabled clicks dispatch exactly one typed intent.

## Post-RC16 interaction gates

12. **Launch and WorkContext are coherent.** Ordinary launch fills the active
    display work area without fullscreen, adopts launcher cwd, focuses the
    composer, and centers the current directory plus one accessible Change
    action in an empty chat. Cancel/failure retains context. Confirmation
    updates every workspace consumer.
13. **The shell is exact, not aspirational.** The primary rail is exactly New
    session, Chat, Project home, and Settings. Compact rows do not move on
    hover. Status/timestamps stay inline. The command palette contains only
    currently available registry actions and real recent sessions.
14. **Images remain bounded capabilities.** Picker, paste, and drop accept only
    PNG/JPEG/WebP/GIF, at most eight and 10 MB each. Removable previews are
    named, image-only idle send works, failure restores, Steer/Queue stay
    text-only, and renderer props carry neither arbitrary paths nor base64.
15. **Codex custody is app-owned and private.** The packaged pinned runtime is
    the sole turn/maintenance authority under minimal PATH. Settings are
    Codex-only, identity stays fake-and-blurred until explicit reveal, config
    repair is narrowly verified, and advisory/update failure is honest and
    non-focus-stealing.
16. **High-volume sources are cadence-bounded.** Provider text, PTY output, and
    workspace events retain exact order/content while avoiding 1:1 state
    publication. Ten-thousand-event candidate and falsifier corpora cover
    ignored trees, 256-ref overflow, bounded queues/tails, hidden-view scoping,
    teardown, input latency, idle CPU, and RSS growth.
17. **Programmatic and visible steering are congruent.** Every expected-working
    action maps to one canonical typed identity and one visible accessible
    keyboard or pointer route. Headed Electron proof drives real Chromium
    focus/accessibility/input semantics suitable for external Computer Use.
    DOM mutation and test-only state injection are not visible-route proof.

## Proposed AssuranceSpec binding

The parser-valid proposal bound to revision 4 is
`openagents-desktop-mvp-phase-2-react-codex-workbench.rev4-proposed.assurance-spec.md`.
Its obligations are proof design only until separately reviewed and admitted.
The admitted Phase 1 AssuranceSpec and RC16 receipts remain immutable history
and cannot be retargeted to this ProductSpec.

## Release gates

Any candidate built from revision 4 must pass the complete Node 24 Desktop verification suite, the focused
revision-3 hierarchy oracles, the 18-obligation admitted Phase 1 assurance run,
production build, signed hardened-runtime packaging, Apple notarization,
stapler/Gatekeeper/deep-signature checks, mounted-DMG React smoke, monotonic
prior-candidate update with interruption recovery, downgrade refusal, rollback,
diagnostic export, uninstall, reinstall, cleanup, and zero-owner teardown.
For the historical accepted path this includes the exact RC15-to-RC16 update.
revision 4 must bind its own monotonic prior-candidate lifecycle receipt.

The receipt must bind the exact source commit, revision-4 ProductSpec and proposed/admitted AssuranceSpec digests, artifact
bytes and SHA-256, Apple submission result, test counts, residuals, and all
AC-1 through AC-20 dispositions. A passing candidate does not publish a feed,
tag, GitHub release, artifact, or public claim.
