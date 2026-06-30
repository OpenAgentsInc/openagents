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
* Hosted/local attachment upload storage, scan/parse workers, and public-safe
  attachment receipts: #7648.
* Automated visual regression captures for the desktop/web composer and
  three-effect HUD: #7649.

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

No product-promise record was added for this work because the public product
claim did not broaden. The work replaced and hardened existing input surfaces.

## Cross-Surface Test Matrix

| Surface | Evidence | Coverage |
| --- | --- | --- |
| Pure state | `packages/composer-state/src/index.test.ts` | Typed transactions, undo/redo, resize, attachment insert/remove/retry/status, gapcursor selection, shortcuts, input rules, Markdown round-trip, large text offer. |
| Shared UI | `packages/ui/test/command-composer.test.ts` | Native textarea contract, autofocus/focus-after-submit hooks, attachments, Markdown preview, error retry action, submitted-state editability, 100k-character prompt rendering. |
| Shared CSS | `packages/ui/src/ai-elements/command-composer.css` plus UI tests | Existing-border focus styling, reduced-motion transition cutoff, stable footer lanes for controls/status/resize/send. |
| Khala desktop | `clients/khala-code-desktop/tests/app-shell.test.ts` and `bun run --cwd clients/khala-code-desktop verify` | Chat-only shell, no dummy fixture messages, plural Khala voice, Markdown transcript rendering, native edit menu accelerators, pending-turn editability, attachments/large paste/HUD wiring, footer layout. |
| OpenAgents web | `apps/openagents.com/apps/web/src/main.test.ts`, `apps/openagents.com/apps/web/src/page/loggedOut/update.test.ts`, and local Playwright smoke | Public `/chat` uses the shared composer, native edit/focus hooks are present, preview/expanded state stays local, Enter submits, Shift+Enter remains textarea input. |
| three-effect HUD | `clients/khala-code-desktop/src/ui/main.ts`, `clients/khala-code-desktop/tests/app-shell.test.ts`, and issue #7643/#7645 smoke evidence | Khala desktop consumes `createCommandComposerHud`, mounts `#composer-hud`, projects focus/attachment/preview state, passes reduced-motion preference, and hides the HUD on renderer failure rather than blocking text entry. |
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
bun run --cwd clients/khala-code-desktop verify
bun run check:deploy
```

Optional browser smoke for `/chat`:

1. Start `bun run --cwd apps/openagents.com/apps/web dev -- --host 127.0.0.1`.
2. Open `/chat`.
3. Verify the composer autofocuses.
4. Type Markdown, submit with Enter, and confirm focus returns to the textarea.
5. Toggle Preview and Expand.
6. Check the footer: status, resize, and Send must not overlap.
7. Stop the dev server.

## Issue Evidence

Issue #7645 should link the final commit and include:

* commit hash;
* the commands above and their pass/fail result;
* the local browser smoke result for `/chat`;
* the desktop verifier result;
* the follow-up issue numbers for deferred work.
