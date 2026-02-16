# OpenAgents.com Production Env + Secrets (Cloud Run)

This runbook documents how to manage production configuration for `apps/openagents.com` on Google Cloud Run without storing sensitive values in git.

## Scope
- Service: `openagents-web`
- Project: `openagentsgemini`
- Region: `us-central1`
- Primary custom domain target: `https://next.openagents.com`

## Configuration model
Use two buckets of runtime config:

1. Non-secret env vars via `--update-env-vars`
2. Secret-backed env vars via `--update-secrets` (Secret Manager)

Do not put secret values in `.env` files in git, docs, or shell history where avoidable.

## 1) Update non-secret env vars
Use this pattern to update production-safe values.

```bash
gcloud run services update openagents-web \
  --project openagentsgemini \
  --region us-central1 \
  --update-env-vars \
APP_ENV=production,APP_DEBUG=0,APP_URL=https://next.openagents.com,ASSET_URL=https://next.openagents.com,SESSION_DOMAIN=.openagents.com,LOG_CHANNEL=stderr
```

Common non-secret vars in this deployment:
- `APP_ENV`, `APP_DEBUG`
- `APP_URL`, `ASSET_URL`
- `SESSION_DOMAIN`
- `LOG_CHANNEL`
- `CACHE_STORE`, `QUEUE_CONNECTION`, `SESSION_DRIVER`
- `REDIS_HOST`, `REDIS_PORT`
- `DB_CONNECTION`, `DB_HOST`, `DB_DATABASE`, `DB_USERNAME`
- `AI_DEFAULT`

## 2) Rotate secret values in Secret Manager
Add a new secret version (example shown for one key):

```bash
printf '%s' '<new-value>' \
| gcloud secrets versions add <secret-name> \
  --project openagentsgemini \
  --data-file=-
```

Examples of existing secret names:
- `openagents-web-app-key`
- `openagents-web-db-password`
- `openagents-web-workos-client-id`
- `openagents-web-workos-api-key`
- `openagents-web-workos-redirect-url`
- `openagents-web-openrouter-api-key`
- `openagents-web-smoke-secret`

## 3) Bind/update secret env refs on Cloud Run
After rotating secret versions, point the service env refs at latest:

```bash
gcloud run services update openagents-web \
  --project openagentsgemini \
  --region us-central1 \
  --update-secrets \
APP_KEY=openagents-web-app-key:latest,DB_PASSWORD=openagents-web-db-password:latest,WORKOS_CLIENT_ID=openagents-web-workos-client-id:latest,WORKOS_API_KEY=openagents-web-workos-api-key:latest,WORKOS_REDIRECT_URL=openagents-web-workos-redirect-url:latest,OPENROUTER_API_KEY=openagents-web-openrouter-api-key:latest,OA_SMOKE_SECRET=openagents-web-smoke-secret:latest
```

## 4) Verify active runtime config
Inspect active env var keys and latest revision:

```bash
gcloud run services describe openagents-web \
  --project openagentsgemini \
  --region us-central1 \
  --format='yaml(spec.template.spec.containers[0].env,status.latestReadyRevisionName,status.url)'
```

## 5) Verify health and stream smoke
Read smoke secret and run smoke checks:

```bash
SMOKE_SECRET="$(gcloud secrets versions access latest --secret openagents-web-smoke-secret --project openagentsgemini)"
OPENAGENTS_BASE_URL="https://next.openagents.com" OA_SMOKE_SECRET="$SMOKE_SECRET" apps/openagents.com/deploy/smoke/health.sh
OPENAGENTS_BASE_URL="https://next.openagents.com" OA_SMOKE_SECRET="$SMOKE_SECRET" apps/openagents.com/deploy/smoke/stream.sh
```

Expected:
- Health: `ok: https://next.openagents.com/up`
- Stream: `ok: stream done (...)`

## 6) Domain/cert notes
For Cloud Run managed TLS on `next.openagents.com`:
- Use Cloud Run domain mapping for `next.openagents.com` -> `openagents-web`
- DNS record must be:
  - `next CNAME ghs.googlehosted.com.`
- If using Cloudflare, keep it DNS-only during certificate issuance.

Check mapping status:

```bash
gcloud beta run domain-mappings describe \
  --project openagentsgemini \
  --region us-central1 \
  --domain next.openagents.com
```

Ready state requires:
- `Ready=True`
- `CertificateProvisioned=True`

## 7) Operational cautions
- Do not use `--set-env-vars` unless intentionally replacing all env vars.
  - Prefer `--update-env-vars`.
- Do not use `--set-secrets` unless intentionally replacing all secret bindings.
  - Prefer `--update-secrets`.
- Every env/secrets update creates a new Cloud Run revision.
- Keep `APP_URL` and `ASSET_URL` aligned to the active public domain to avoid URL generation drift.

## 8) Minimal production change checklist
1. Rotate or add secret version(s)
2. `gcloud run services update ... --update-secrets ...`
3. `gcloud run services update ... --update-env-vars ...`
4. `gcloud run services describe ...` verify revision/env refs
5. Run smoke checks on the target domain
