# OpenAgents Desktop React cutover receipt

Date: 2026-07-14

ProductSpec: `openagents-desktop-mvp-phase-2-react-codex-workbench`

ProductSpec revision: `1`

ProductSpec SHA-256: `f5116b388df35752aacb5c8fe87bad6f60f92b5363e4e13fadc3b998d365c862`

## Disposition

The ordinary source-built Desktop launch now selects the React Codex
workbench. `?renderer=compatibility` is the one explicit exclusive fallback
for non-converted specialist surfaces and the broad legacy acceptance oracle.
The two backends never render the same authoritative surface together.

This is an implementation and deterministic integration receipt. The exact
signed/notarized artifact and reversible release lifecycle are recorded in
`2026-07-14-openagents-desktop-react-rc13-release-receipt.md`. Neither receipt
is a real-account acceptance record, an owner ProductSpec admission, an
independent visual-review acceptance, or publication authority. Those external
gates remain open on issue #8823.

## Integrated default-backend proof

`pnpm --dir apps/openagents-desktop run smoke:react` builds the production
renderer, launches real Electron with fixture-only provider processes, and
proves:

- exactly one `[data-en-react-surface="true"]` and no compatibility shell root;
- the first real Chromium keydown/input reaches the focused shadcn Textarea and
  persists through the Effect-owned `DesktopInputChanged` transition;
- a new app-local chat admits one Codex fixture turn through the existing local
  runtime, rendering user, model, reasoning, tool-start, tool-result, and
  assistant records through React;
- the React review control requests and displays a real bounded status/diff
  from the smoke repository, leaks no absolute path, and exposes none of Stage,
  Discard, Commit, Push, or Terminal;
- Runtime Gateway v11 remains live with the expected capability catalog;
- renderer reload remounts React, resumes the same durable local thread, and
  restores the exact six-item prefix without the compatibility backend; and
- teardown returns the active lifecycle-owner count to zero.

The built smoke found and fixed two integration defects that component tests
could not expose:

1. React helpers constructed payload-less `IntentRef`s, so Effect Native
   correctly decoded value-bearing commands as `null`. Every React dispatcher
   now declares `ComponentValueBinding()` before supplying the runtime value,
   and tests resolve the ref exactly as the production reporter does.
2. the local chat adapter created threads in the app-owned durable store but
   reloaded its list/detail from provider history. It now lists and resumes the
   same app-local store; provider history remains a separate read-only source.

The broad compatibility smoke remains green through the explicit backend and
continues to cover the retained settings, diagnostics, files/editor, Fleet,
ProductSpec, child, terminal, update, and other specialist surfaces.

## State and authority ledger

- React owns elements, focus, overlay state, IME state, and scroll anchoring.
- Effect owns `DesktopShellState`, typed intents, admission, retries, runtime
  dispositions, Git refusal codes, and every effect.
- Main/preload retain runtime, filesystem, Git, persistence, account, and
  process authority.
- shadcn/Base UI source components use the dark-zinc Khala token extension.
- Vercel AI SDK remains absent. The existing Codex Runtime Gateway/app-server
  remains the sole stream, tool, decision, and persistence authority.

## Compatibility residuals

The explicit compatibility backend still owns specialist settings,
diagnostics, file/editor, Fleet, ProductSpec workroom, child-management,
terminal, update, and other catalog surfaces outside the Phase 2 basic Codex
management scope. React is the default only for the scoped workbench: session
rail, typed conversation, composer, decisions, status/recovery, and read-only
review.

## Release-boundary follow-up

Exact RC13 subsequently passed Developer ID signing, app and outer-DMG Apple
notarization/stapling, artifact-bound Gatekeeper preflight, read-only-DMG React
smoke with a pristine user-data root, and the reversible RC12-to-RC13
update/rollback/downgrade-refusal/reinstall/cleanup driver. See
`2026-07-14-openagents-desktop-react-rc13-release-receipt.md` for the immutable
digest, Apple submission, and bounded public-safe evidence.

## Pinned assurance execution

The admitted `openagents-codex-workroom-mvp` AssuranceSpec revision 2 was
re-executed after the React cutover against its exact frozen ProductSpec
revision 6 binding. The session remained unchanged against both pins:

- AssuranceSpec:
  `sha256:66e1b49d3089b141a9bd5fb6221d002a0d364259ab719a46a254a507fb0dee72`;
- ProductSpec:
  `sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1`;
- manifest:
  `sha256:afd25a5d9f9a8442773d3d18dbda1b4feae4a29e4181a9afc8f1d9cc72cbdb17`;
  and
- current evidence index:
  `sha256:36cd0908f517501648c990d10271831856f7e5fb2bc85643e515acb7f9e61053`.

The runtime lock rejected an initial invocation under Node 25.8.2 before it
could produce evidence. The admitted Node 24.13.1 runtime was then used for
the complete execution. That run exposed a React test-lifecycle race in the
decision surface: a failed assertion could let a scheduled React task outlive
the test DOM. The harness now registers every root, wraps rendering and input
in React `act`, and unmounts every root before restoring globals. The exact
failed-bridge copy and explicit `DesktopApprovalApproved` intent oracle were
retained. The focused file passed 20 consecutive repetitions before the full
rerun.

Final bounded result:

- 18 of 18 candidate observations confirmed;
- 18 of 18 falsifiers refuted;
- full Desktop typecheck, production build, and Electron smoke passed;
- 139 test files passed; 1,343 tests passed and 39 retired tests remained
  skipped; and
- evidence axes remained distinct for every obligation: admission `admitted`,
  readiness `executable`, observation `CONFIRMED`, infrastructure `ready`,
  stability `stable`, freshness `current`, disposition `accepted`, and
  exception `none`.

The schema-valid claim audit echoes the requested claim but does not
semantically evaluate it or grant publication authority. The full Desktop gate
is separately `green`; it is not collapsed into criterion observations.

This is re-execution of the admitted first-workroom contract plus the current
Desktop gate. The Phase 2 React ProductSpec still has no separately admitted
AssuranceSpec, so this result is not a Phase 2 admission or a substitute for
the external gates below.

## Authoritative decision follow-up

After the RC13 receipt was produced, the installed React smoke gained a
provider-originated decision proof modeled on the orchestration boundary in
the T3 reference implementation. A protocol-speaking app-server peer is
injected at the production spawn seam and originates command-approval request
`91`. The normal main-process runtime parks that request, projects it through
the typed local event and IPC path, and the React dialog dispatches `Approve`
through the installed Effect intent runtime. The provider records the exact
correlated `{ decision: "accept" }` response and withholds command completion,
assistant output, usage, and turn completion until that response arrives.

The built smoke additionally waits for decision reconciliation, final output,
reload restoration, and zero runtime owners. This closes the deterministic
integrated decision obligation; it does not claim a live-account or
private-provider run.

## External gates not claimed

The following #8823 close-rule evidence is not produced by this packet:

- a real ordinary-session Codex run with private evidence handling;
- owner/admission disposition for ProductSpec revision 1;
- independent visual and interaction reviewer acceptance;
- full VoiceOver/keyboard-only and measured WCAG contrast receipt;
- admitted-device warm-launch/session-switch/input-latency/memory percentiles;
- release publication or public-language authorization.

These remain inconclusive, not green and not waived.
