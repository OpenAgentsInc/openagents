# Desktop local-usage opt-in: verification and rollout

Issue: [#8911](https://github.com/OpenAgentsInc/openagents/issues/8911)

## Status

The engineering path is complete but intentionally double-gated:

1. Desktop must be launched with `OPENAGENTS_DESKTOP_USAGE_CONSENT_CONTROL=1`.
2. The signed-in user must explicitly turn on **Settings → Share local Codex usage**.
3. The API must have `DESKTOP_CODEX_USAGE_INGEST_ENABLED=1`.

All three are off by default. No production metric movement is claimed by this
change. The Settings copy still requires owner approval before the review-only
Desktop gate or server gate is enabled in an ordinary release.

## Authority and privacy contract

- Desktop asks the authenticated API to pre-admit a bounded `{turnRef, model}`
  before the local Codex turn starts. Admission is owner-bound, durable in the
  owned Postgres KV, expires after 24 hours, and is limited to 120 admissions
  per signed-in owner per hour.
- A completion is rejected unless its admission belongs to the authenticated
  owner and matches the exact turn and model. The token ledger remains
  idempotent on the owner/turn identity.
- The durable Desktop outbox contains only the opaque admission, turn ref,
  model, timestamps, exact token split, and retry state. It never contains an
  access token, refresh token, owner id, prompt, response, file, path, account
  name, or provider credential. The file is mode `0600` and atomically written.
- Network/5xx/401/408/429 failures back off from 30 seconds to 30 minutes.
  Successful or terminally invalid reports are deleted. Turning consent off or
  resetting preferences immediately deletes every queued report.
- Admission and reporting are fail-soft: telemetry can never block a local or
  Full Auto turn.

## Automated verification

From repository root:

```sh
pnpm exec vp test --run --max-concurrency 1 \
  apps/openagents-desktop/src/desktop-codex-usage-outbox.test.ts \
  apps/openagents-desktop/src/desktop-codex-usage-reporter.test.ts \
  apps/openagents-desktop/tests/desktop-preferences.test.ts \
  apps/openagents-desktop/src/renderer/settings.test.ts \
  apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx \
  apps/openagents.com/workers/api/src/desktop-codex-usage-routes.test.ts
pnpm --dir apps/openagents-desktop run typecheck
pnpm --dir apps/openagents.com/workers/api run typecheck
pnpm run check:fast
```

The focused suite proves default-off zero traffic, server-verified session
gating, pre-admission, owner/turn/model binding, exact safe payload shape,
idempotency, credential-free persistence, restart retry, preference migration,
hidden-until-review Settings UI, and consent dispatch.

## Owner-reviewed live proof

Perform only after approving the Settings copy and enabling the server rollout
gate in the sanctioned Cloud Run deployment configuration.

1. Record the current value from `GET /api/public/khala-tokens-served`.
2. Launch the current Desktop build with
   `OPENAGENTS_DESKTOP_USAGE_CONSENT_CONTROL=1` and use a normal signed-in
   profile. Do not use isolated-app proof mode; it deliberately has no session.
3. Open **Settings → Share local Codex usage**, read the disclosure, and turn
   it on.
4. Send one ordinary local Codex turn with a known non-zero exact SDK usage.
5. Wait for completion, then confirm the public counter advances by that turn's
   `input + output + reasoning` total exactly once.
6. Restart Desktop and confirm the consent setting persists. Induce a temporary
   API failure for one admitted turn, restore the API, and confirm the durable
   outbox retries it exactly once.
7. Turn sharing off, complete another local turn, and confirm there is no
   admission request, no usage request, and no counter movement from that turn.
8. Repeat steps 3–5 for one Full Auto continuation. It uses the same Codex lane
   admission and completion seam and must advance exactly once.

Attach only public-safe counter deltas, response status/schema, and turn refs to
the issue. Never attach bearer values, outbox bytes, profile paths, prompts, or
responses.
