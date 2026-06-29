# Sites Self-Serve Builder UI

Date: 2026-06-05

Issue: `#202` / `OPENAGENTS-SITES-VIBE-011`

## What Changed

The customer order detail page now has a Site builder panel for Site-backed
orders. It lets the customer start or reconnect a durable builder session from
the order page, then inspect the customer-safe builder projection from one
place.

The panel shows:

- builder session status, next action, prompt summary, friendly updated time,
  preview link when a preview URL exists, and current live Site URL when one is
  active;
- phase timeline from the public builder-session projection;
- replayed customer-visible event stream records with a refresh action;
- customer-visible generated file tree and a read-only file preview;
- recent customer-visible builder messages;
- the existing revision feedback composer remains directly below the builder
  workflow so follow-up comments still enter the normal revision queue.

## Boundaries

The UI only consumes browser-session APIs that already enforce customer
ownership and visibility boundaries:

- `POST /api/sites/builder-sessions`
- `GET /api/sites/builder-sessions/:id`
- `GET /api/sites/builder-sessions/:id/events`
- `GET /api/sites/builder-sessions/:id/files`
- `GET /api/sites/builder-sessions/:id/files/tree`
- `GET /api/sites/builder-sessions/:id/files/read`

The backend public builder-session projection now also includes the active
preview record when available:

- `activePreview.id`
- `activePreview.status`
- `activePreview.previewUrl`
- `activePreview.updatedAt`

No operator-only artifact refs, source refs, internal events, or private
runner/provider material are projected to customers.

## Remaining Follow-Up

This slice intentionally does not add customer-controlled deploy approval,
saved-version creation, or source export. Those remain in the later VibeSDK
parity issues:

- `OPENAGENTS-SITES-VIBE-012`: SDK
- `OPENAGENTS-SITES-VIBE-013`: GitHub/source export
- `OPENAGENTS-SITES-VIBE-014`: app library controls

## Verification

- `bun run --cwd apps/web typecheck`
- `bun run --cwd apps/web test src/page/loggedIn/view.scene.test.ts`
- `bun run --cwd workers/api test src/sites-builder-sessions.test.ts src/agent-site-routes.test.ts`
