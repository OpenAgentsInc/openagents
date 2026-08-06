# Mobile client command outbox

`openagents-mobile` now owns the durable substrate required to become a Convex controller without weakening its existing read-only safety boundary prematurely.

- Shared policy and wire schema: `packages/client-command-outbox/`
- Expo adapter: `apps/openagents-mobile/src/outbox/expo-sqlite-outbox-store.ts`
- Contract version: `openagents.client_command_outbox.v1`

SQLite runs in WAL mode with `synchronous=FULL`. Enqueue, command-ID collision checks, corrupt-row quarantine, receipt creation, and dequeue use transactions; corrupt rows retain only a SHA-256 digest and never raw possibly sensitive bytes. The last-known observation cache carries an observed timestamp so UI can render `cached`, `synchronizing`, or `live` with an age.

All queueable online and offline commands use the same local enqueue path. Drain requires a live Convex connection and live target shell. Live controls and destructive Git are unavailable offline and are never replayed. Expired or revised decisions become terminal `fresh_decision_required` receipts.

This phase deliberately exposes a substrate, not a hidden command path in the current mirror UI. The mobile controller frame and transcript/composer phases connect visible user actions to this outbox and authoritative Convex receipts.
