# Command Composer Acceptance Runbook

Date: 2026-06-30

Status: v1 accepted for the OpenAgents-owned command composer surfaces covered
by ADR-0013.

This runbook records the cross-surface behavior, privacy boundaries, and
acceptance evidence for the ProseMirror-inspired command composer. It is scoped
to the shared `@openagentsinc/composer-state` model, the shared
`@openagentsinc/ui` command-composer element, Khala Code desktop, and the public
OpenAgents `/chat` Khala surface.

## Behavior Contract

The v1 composer is source-first and textarea-native. Rich editing concepts are
modeled as typed state and transactions, but browser text entry remains a native
`textarea` so copy, paste, select-all, IME, spellcheck, mobile keyboards, and
platform edit menus keep working.

Supported v1 behavior:

* Markdown source input with preview rendering through the shared AI Elements
  response renderer.
* Block-level Markdown round-trip coverage for paragraphs, blockquotes, lists,
  fenced code, and unsupported source text that must be preserved rather than
  discarded.
* Inline preview coverage for strong, emphasis, inline code, and safe links.
* Typed attachment metadata for pasted/dropped images, files, text snippets, and
  large pasted text offers.
* Attachment upload lifecycle planning for desktop-local and web-hosted
  authority surfaces, including file-size/type policy checks, scan/parse worker
  task descriptors, content-addressed refs, thumbnail refs, retry attempts,
  removal receipts, and public-safe privacy receipts.
* Large pasted text handling as an attachment offer instead of silently rewriting
  the prompt body.
* Gapcursor/dropcursor hooks around attachment chips.
* Retry, remove, staged, uploading, ready, and error attachment states.
* Resize/expanded composer state, status strip, command controls, submit/stop
  affordance, accessibility mirror, autofocus, and focus-after-submit hooks.
* Native shortcuts preserved: platform copy/paste/select-all stay native;
  composer-owned submit/newline/attachment navigation is explicit and bounded.
* Reduced-motion behavior: CSS transitions are disabled under
  `prefers-reduced-motion`, response cursors are static under reduced motion,
  and Khala Code desktop passes the reduced-motion preference into the
  three-effect HUD projection.

Deferred from v1:

* Full rich inline editing, collaborative rebasing, and contentEditable or
  ProseMirror-view embedding: #7647.
No v1 visual-regression deferrals remain for the composer/HUD lane. The
automated Playwright smoke is warning-only for pre-push because it needs local
browser dependencies and two Vite dev servers, but its assertions are
deterministic and its artifacts are public-safe synthetic captures.

## Privacy Boundary

The composer state stores prompt source text in active app memory only. Public
projection code must never persist raw private prompt text, raw file bytes, or
unredacted attachment bodies.

Attachment state stores metadata and references:

* `name`, `mime`, `sizeBytes`, dimensions, status, source, digest, preview refs,
  and bounded `contentRef` values are allowed.
* Raw private file contents are not allowed in docs, fixtures, traces, receipts,
  or public projections.
* Large pasted text may be offered as a staged text attachment, but public-safe
  records must use metadata/digest/redacted summaries.
* Desktop-local attachment execution may hold `File` objects, object URLs, and
  pasted text in local app memory while hashing/registering the attachment.
  Those executor refs are never transcript or receipt fields; receipts expose
  only metadata, lifecycle event, upload attempt, digest, dimensions, and
  content-addressed `attachment.desktop-local.sha256.*` /
  `attachment_thumbnail.desktop-local.sha256.*` refs after readiness.
* Web-hosted attachments use the same state contract with `web-hosted` refs, but
  hosted storage and worker authority remain separate from the desktop-local
  executor.

No product-promise record was added for this work because the public product
claim did not broaden. The work replaced and hardened existing input surfaces.

## Cross-Surface Test Matrix

| Surface | Evidence | Coverage |
| --- | --- | --- |
| Pure state | `packages/composer-state/src/index.test.ts` | Typed transactions, undo/redo, resize, attachment insert/remove/retry/status, upload attempts, desktop-local/web-hosted upload plans, content-addressed refs, retry/removal receipts, gapcursor selection, shortcuts, input rules, Markdown round-trip, large text offer. |
| Shared UI | `packages/ui/test/command-composer.test.ts` | Native textarea contract, autofocus/focus-after-submit hooks, attachments, Markdown preview, error retry action, submitted-state editability, 100k-character prompt rendering. |
| Shared CSS | `packages/ui/src/ai-elements/command-composer.css` plus UI tests | Existing-border focus styling, reduced-motion transition cutoff, stable footer lanes for controls/status/resize/send. |
| Khala desktop | `clients/khala-code-desktop/tests/app-shell.test.ts` and `bun run --cwd clients/khala-code-desktop verify` | Chat-only shell, no dummy fixture messages, plural Khala voice, Markdown transcript rendering, native edit menu accelerators, pending-turn editability, desktop-local attachment hashing/receipts, attachments/large paste/HUD wiring, footer layout. |
| OpenAgents web | `apps/openagents.com/apps/web/src/main.test.ts`, `apps/openagents.com/apps/web/src/page/loggedOut/update.test.ts`, and local Playwright smoke | Public `/chat` uses the shared composer, native edit/focus hooks are present, preview/expanded state stays local, Enter submits, Shift+Enter remains textarea input. |
| three-effect HUD | `clients/khala-code-desktop/src/ui/main.ts`, `clients/khala-code-desktop/tests/app-shell.test.ts`, and issue #7643/#7645 smoke evidence | Khala desktop consumes `createCommandComposerHud`, mounts `#composer-hud`, projects focus/attachment/preview state, passes reduced-motion preference, and hides the HUD on renderer failure rather than blocking text entry. |
| Automated visual regression | `clients/khala-code-desktop/scripts/composer-visual-smoke.ts` and `clients/khala-code-desktop/tests/composer-visual-smoke.test.ts` | Launches Khala Code desktop and OpenAgents web Vite previews, captures desktop/mobile screenshots for Khala Code, `/chat`, and `/autopilot`, asserts nonblank canvas/HUD pixels, focus border framing, footer child non-overlap, viewport geometry, reduced-motion media behavior, and synthetic prompt privacy. |
| Privacy fixture guard | `apps/openagents.com/scripts/check-command-composer-privacy-fixtures.test.ts` | Deploy-gated scan of v1 composer docs/source/tests for raw private prompt/file and secret sentinel patterns. |

## Acceptance Commands

Run these from the repository root:

```bash
bun run --cwd packages/composer-state test
bun run --cwd packages/ui test
bun run --cwd packages/ui typecheck
bun run --cwd apps/openagents.com/apps/web test -- src/main.test.ts src/page/loggedOut/update.test.ts
bun run --cwd apps/openagents.com/apps/web typecheck
bun run --cwd apps/openagents.com test:command-composer-privacy-guard
bun test clients/khala-code-desktop/tests/composer-visual-smoke.test.ts
bun run --cwd clients/khala-code-desktop smoke:composer-visual
bun run --cwd clients/khala-code-desktop verify
bun run check:deploy
```

`smoke:composer-visual` writes public-safe screenshots and `summary.json` to
`var/khala-code-desktop/composer-visual-smoke` by default. The harness fills
only this synthetic prompt:
`Synthetic visual smoke prompt: summarize the public onboarding flow.`

The local pre-push hook exposes the same capture lane as warning-only:

```bash
OPENAGENTS_PRE_PUSH_COMPOSER_VISUAL=1 git push origin main
```

That hook path never blocks a push by itself. It is for browser-dependent
evidence collection; the deploy gate remains `check:deploy`.

## Issue Evidence

Issue closeouts should link the final commit and include:

* commit hash;
* the commands above and their pass/fail result;
* the composer visual smoke artifact directory or blocker;
* the desktop verifier result;
* the follow-up issue numbers for deferred work.
