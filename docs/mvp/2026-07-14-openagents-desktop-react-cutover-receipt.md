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

This is an implementation and deterministic integration receipt. It is not a
signed/notarized artifact receipt, a real-account acceptance record, an owner
ProductSpec admission, an independent visual-review acceptance, or publication
authority. Those external gates remain open on issue #8823.

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

## External gates not claimed

The following #8823 close-rule evidence is not produced by this packet:

- signed and notarized installed macOS ARM64 artifact identity;
- clean install/reinstall/update/rollback/downgrade receipts;
- a real ordinary-session Codex run with private evidence handling;
- owner/admission disposition for ProductSpec revision 1;
- independent visual and interaction reviewer acceptance;
- full VoiceOver/keyboard-only and measured WCAG contrast receipt;
- admitted-device warm-launch/session-switch/input-latency/memory percentiles;
- release publication or public-language authorization.

These remain inconclusive, not green and not waived.
