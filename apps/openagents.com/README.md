# openagents.com (Laravel)

**Core web control-plane app** for OpenAgents: Laravel 12 + Inertia + React (TypeScript), WorkOS auth, Laravel Boost.

- **Plan:** `docs/plans/active/laravel-rebuild.md` (from repo root: `../../docs/plans/active/laravel-rebuild.md`)
- **Verification:** `composer test`, `composer lint`, `npm run build`

## Local development

### Option A: Laravel Herd (recommended for `.test` domain)

From this directory (`apps/openagents.com`):

```bash
herd link openagents.com   # one-time: use link, not park, so Herd uses public/ as doc root
herd secure openagents.com # one-time: TLS for https://openagents.com.test
```

Set in `.env`: `APP_URL=https://openagents.com.test`

Then in one terminal: `npm run dev` (Vite). Herd serves PHP; open **https://openagents.com.test** in the browser.

### Option B: Artisan serve

```bash
composer run dev
```

Or: `php artisan serve` in one terminal and `npm run dev` in another. Use **http://localhost:8000** and `APP_URL=http://localhost:8000` in `.env`.

## Production deploy

Deploy via `deploy/deploy-production.sh` so Cloud Build must succeed before Cloud Run rollout:

```bash
cd apps/openagents.com
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-web deploy/deploy-production.sh
```

The script runs `npm install` in the app directory before uploading to Cloud Build so `package-lock.json` is in sync with `package.json` (Cloud Build uses `npm ci`, which requires an exact match). If `package-lock.json` changes, commit and push it after a successful deploy.

This path is enforced as:

1. Cloud Build Docker image build (Dockerfile runs `npm run build` in the `node_build` stage).
2. If Vite build fails, image build fails and deploy stops.
3. Only a successful image is deployed to Cloud Run.

## Stack

- Laravel 12, Inertia, React (TS), Vite
- WorkOS auth
- Laravel Wayfinder (typed routes/actions)
- Pest for PHP tests

## Staging smoke checks

Run deploy smoke checks against staging:

```bash
OPENAGENTS_BASE_URL="https://staging.openagents.com" ./deploy/smoke/health.sh
SMOKE_SECRET="$(gcloud secrets versions access latest --secret openagents-web-staging-smoke-secret --project openagentsgemini)" OPENAGENTS_BASE_URL="https://staging.openagents.com" OA_SMOKE_SECRET="$SMOKE_SECRET" ./deploy/smoke/stream.sh
OPENAGENTS_BASE_URL="https://staging.openagents.com" OA_SMOKE_ADMIN_EMAIL="chris@openagents.com" ./deploy/smoke/paywall-e2e.sh
```

`paywall-e2e.sh` is intentionally staging-safe and refuses non-staging URLs unless `OA_SMOKE_ALLOW_NON_STAGING=1` is set.
