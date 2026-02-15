# Laravel Rebuild: GCP Deploy Work Log (2026-02-15)

This log records work done to stand up `apps/openagents.com/` (Laravel 12 + Inertia + React) on Google Cloud.

## Repo Baseline

- Repo: `OpenAgentsInc/openagents`
- Branch: `main`
- Current HEAD (at time of writing): `d833c1c605399946430ca03c565cf7284469a394`
- Recent relevant commits:
  - `d833c1c60` apps/openagents.com: add Cloud Run deploy assets
  - `695203153` docs: expand Laravel rebuild roadmap
  - `effff4184` artisan
  - `b14ff55f3` laravel new

### Local Working Tree Notes

At the time this log file was created, there were uncommitted edits to:

- `apps/openagents.com/Dockerfile`
- `apps/openagents.com/deploy/cloudbuild.yaml`
- `apps/openagents.com/docs/GCP_DEPLOY_PLAN.md`

These edits are part of the ongoing effort to unblock Cloud Build and complete Phase 0 deploy.

## Local Verification (Laravel App)

Commands run (and passing as of today, before the Cloud Build unblock work):

```bash
cd apps/openagents.com
composer test
npm run lint
npm run types
npm run build
```

## GCP Context

- Account: `chris@openagents.com`
- Project: `openagentsgemini`
- Region target: `us-central1`

## Resources Created / Modified (GCP)

### Artifact Registry

- Docker repo created: `openagents-web` (location: `us-central1`)
- Existing Docker repo already present: `l402` (location: `us-central1`)

### Cloud SQL (Postgres)

We reused the existing instance:

- Instance: `l402-aperture-db` (Postgres 15, `us-central1-f`)

And created:

- Database: `openagents_web`
- User: `openagents_web`
  - Password stored in Secret Manager (not committed to repo)

### Secret Manager

Secrets created for the Laravel web app:

- `openagents-web-app-key`
- `openagents-web-db-password`
- `openagents-web-workos-client-id`
- `openagents-web-workos-api-key`
- `openagents-web-workos-redirect-url`

Notes:

- Secret *values* were never committed to git.
- The deploy strategy uses Cloud Run `--set-secrets ...` bindings.

## Deploy Assets Added To Repo

The Laravel app includes Cloud Run deploy assets (added in `d833c1c60`):

- `apps/openagents.com/.dockerignore`
- `apps/openagents.com/Dockerfile`
- `apps/openagents.com/deploy/cloudbuild.yaml`
- `apps/openagents.com/deploy/start.sh`
- `apps/openagents.com/deploy/nginx/nginx.conf`
- `apps/openagents.com/docs/GCP_DEPLOY_PLAN.md` (Forge-style Cloud Run topology)

## Build / Deploy Attempts and Current Blocker

### Cloud Build Submission Size (Monorepo Context)

Initial build attempts submitted from repo root uploaded the entire monorepo (very large, ~GiB scale).

Resolution:

- Update build instructions to submit using the Laravel app folder as context:

```bash
gcloud builds submit \
  --config apps/openagents.com/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/openagents.com
```

### PHP Version vs `composer.lock`

Composer install in Docker required PHP >= 8.4 (based on the lockfile contents).

Resolution:

- Use `php:8.4-*` images in Docker build stages.

### Composer Scripts During Image Build

Composer scripts can assume `artisan` exists, which breaks layer caching and early vendor installs.

Resolution:

- Install vendor deps with `--no-scripts` in the vendor/deps stage.
- Run Laravel discovery later in the runtime stage:

```bash
php artisan package:discover --ansi
```

### Wayfinder / Vite Build Failure (Current Blocker)

Cloud Build is currently blocked during `npm run build` in Docker.

Observed error:

- `[@laravel/vite-plugin-wayfinder] Error generating types: Command failed: php artisan wayfinder:generate --with-form`

Work done so far:

- Build JS assets in a stage that has both PHP and Node available, because the Vite Wayfinder plugin invokes `php artisan wayfinder:generate`.
- Write a minimal build-time `.env` (with a random `APP_KEY`) before `npm run build`.

Remaining work:

- Surface the actual `artisan` stderr output so we know why it fails under Vite (it may be failing due to missing env/config during build).
- Fix the underlying cause, then complete Cloud Build + Cloud Run deploy.

## Next Steps (Phase 0)

1. Unblock Docker build in Cloud Build (Wayfinder/Vite step).
2. Push a built image to Artifact Registry (`openagents-web`).
3. Deploy Cloud Run:
   - Service: `openagents-web`
   - Jobs: `openagents-migrate`, `openagents-queue`, `openagents-scheduler` (as needed)
4. Run migrations via Cloud Run job.
5. Validate a staging URL serves the Laravel app.

## 2026-02-15: Docker Build Unblocked (Wayfinder + Bootstrap Cache)

### Symptom: Vite build fails via Wayfinder plugin

During Docker build, `npm run build` failed with:

- `[@laravel/vite-plugin-wayfinder] Error generating types: Command failed: php artisan wayfinder:generate --with-form`

The plugin calls `this.error(...)` with a stringified `Error`, which hides artisan stderr.

### Root cause #1: Invalid Blade compiled view cache path

Running `php artisan wayfinder:generate --with-form -vvv` inside the Docker build stage revealed:

- `InvalidArgumentException: Please provide a valid cache path.`

This originates from the Blade compiler requiring a valid `config('view.compiled')`, which defaults to a realpath under:

- `storage/framework/views`

In the image build, those directories did not exist.

Complication:

- We initially used `mkdir -p storage/framework/{cache,sessions,views}` in Docker `RUN` layers.
- Docker uses `/bin/sh` by default (dash on Debian) which **does not support brace expansion**, so the directories were not created as intended.

Fix:

- Create directories explicitly in the build stage before Vite runs:

```dockerfile
RUN mkdir -p storage/framework/cache storage/framework/sessions storage/framework/views bootstrap/cache
```

### Root cause #2: Stale bootstrap cache referencing dev-only provider

After Wayfinder was unblocked, the runtime stage failed during:

- `php artisan package:discover`

with:

- `Class "Laravel\Boost\BoostServiceProvider" not found`

Root cause:

- `bootstrap/cache/packages.php` (copied from the build context) contained a provider for `laravel/boost`.
- `laravel/boost` is a dev dependency and is not present in the `--no-dev` vendor install used for the runtime image.
- Laravel tries to load cached providers during bootstrap, so the command fails before discovery can regenerate the cache.

Fix:

- Delete `bootstrap/cache/packages.php` and `bootstrap/cache/services.php` before running any artisan commands in both:
  - the build stage (before Vite/Wayfinder)
  - the runtime stage (before `package:discover`)

### Result

- `docker build` now completes successfully locally for `apps/openagents.com/`.
- Next step is to re-run Cloud Build with the updated Dockerfile, then deploy `openagents-web` on Cloud Run.
