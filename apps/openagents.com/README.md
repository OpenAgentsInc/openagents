# openagents.com (Laravel)

**Incoming core web app** for OpenAgents: Laravel 12 + Inertia + React (TypeScript), WorkOS auth, Laravel Boost. This app is the target replacement for the current Effuse/Cloudflare/Convex web stack.

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

This path is enforced as:

1. Cloud Build Docker image build (Dockerfile runs `npm run build` in the `node_build` stage).
2. If Vite build fails, image build fails and deploy stops.
3. Only a successful image is deployed to Cloud Run.

## Stack

- Laravel 12, Inertia, React (TS), Vite
- WorkOS auth
- Laravel Wayfinder (typed routes/actions)
- Pest for PHP tests
