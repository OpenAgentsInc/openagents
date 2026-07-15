# Aiur

Aiur is the owner-only OpenAgents administration surface. It runs exclusively
on Google Cloud Run as service `openagents-aiur` in `us-central1`.

The Node server in `src/cloudrun/server.ts` serves the built application and
the owner-gated auth/proxy routes. `AIUR_OWNER_USER_IDS` is mounted from Google
Secret Manager and fails closed when absent. The browser never receives the
upstream OpenAuth bearer used by the same-origin proxy.

## Commands

```sh
pnpm --dir apps/aiur run typecheck
pnpm --dir apps/aiur run test
pnpm --dir apps/aiur run dev
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  pnpm --dir apps/aiur run deploy
```

There is no alternate edge-worker deployment or database authority. Runtime
data remains on the shared OpenAgents Cloud SQL services.
