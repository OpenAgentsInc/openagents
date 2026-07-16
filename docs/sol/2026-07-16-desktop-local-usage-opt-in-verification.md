# Desktop local-usage opt-in: verification and rollout

Issue: [#8911](https://github.com/OpenAgentsInc/openagents/issues/8911)

## Status

The engineering path and owner-approved live proof are complete. The owner
approved the counts-only copy at `8809f79b56`; reporting remains intentionally
double-gated:

1. The signed-in user must explicitly turn on **Settings → Share local Codex usage**.
2. The API must have `DESKTOP_CODEX_USAGE_INGEST_ENABLED=1`.

User consent is off by default. The approved Settings control ships in ordinary
Desktop builds, starts off, and performs no credential read or network request
until the user opts in. The server rollout gate is now part of the sanctioned
production and staging configuration; the temporary proof deployment was
restored off after verification, before the integration deployment.

## 2026-07-16 live proof

- Production revision `openagents-monolith-00173-vqs` deployed successfully,
  passed its owned health/portal/tombstone smokes, and proved the enabled
  admission route was present and authenticated (`401` without a session).
- The approved control rendered in a current ordinary Desktop build. It began
  off, was explicitly enabled, and one real ordinary local Codex turn
  completed.
- The first attempt left the public counter at `8,463,301,501`. This was a
  truthful fail-closed result: the normal profile had no recoverable native
  OpenAgents session, so Desktop performed no admission, created no outbox
  entry, and sent no usage.
- The supported session path is GitHub OAuth through
  `https://auth.openagents.com/authorize`, PKCE loopback, server verification,
  then OS-encrypted vault persistence. The proof invoked the supported typed
  `session.sign_in` runtime-gateway command and completed the browser/PKCE flow;
  it did not expose credentials and did not require a new renderer account-link
  surface.
- Three exact ledger rows matched three public-counter deltas exactly once:
  ordinary `14,096` (`8,463,301,501` → `8,463,315,597`), restart/retry `14,303`
  (`8,463,315,597` → `8,463,329,900`), and Full Auto continuation `176,026`
  (`8,463,329,900` → `8,463,505,926`).
- The observable HTTP sequence was admission `201` then usage `200` for the
  ordinary turn; admission `201`, induced usage `503`, and one successful `200`
  retry after restart; and admission `201` then usage `200` for the Full Auto
  continuation. Replaying an accepted report produced no second counter delta.
- Turning sharing off purged the outbox immediately. The following local turn
  produced no admission or usage request and no counter movement attributable
  to that turn.
- Teardown stopped the isolated Desktop app and proof proxies. The temporary
  production gate was restored off, with unauthenticated admission and usage
  both returning `404`; a fresh independent check reproduced both `404`s and
  found no proof app, HTTP proxy, or SQL helper still running.

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
shipped default-off Settings UI, and consent dispatch.

## Re-running the owner-reviewed live proof

Perform only after approving the Settings copy and enabling the server rollout
gate in the sanctioned Cloud Run deployment configuration.

1. Record the current value from `GET /api/public/khala-tokens-served`.
2. Launch the current Desktop build with a normal profile. Do not use
   isolated-app proof mode; it deliberately has no session.
3. Invoke the supported typed `session.sign_in` runtime-gateway command. Finish
   GitHub authorization in the browser and wait for the local PKCE callback to
   report `session_ready`. Never copy credentials into a harness or receipt.
4. Open **Settings → Share local Codex usage**, read the disclosure, and turn
   it on.
5. Send one ordinary local Codex turn with a known non-zero exact SDK usage.
6. Wait for completion, then confirm the public counter advances by that turn's
   `input + output + reasoning` total exactly once.
7. Restart Desktop and confirm the consent setting persists. Induce a temporary
   API failure for one admitted turn, restore the API, and confirm the durable
   outbox retries it exactly once.
8. Turn sharing off, complete another local turn, and confirm there is no
   admission request, no usage request, and no counter movement from that turn.
9. Repeat steps 4–6 for one Full Auto continuation. It uses the same Codex lane
   admission and completion seam and must advance exactly once.

Attach only public-safe counter deltas, response status/schema, and turn refs to
the issue. Never attach bearer values, outbox bytes, profile paths, prompts, or
responses.
