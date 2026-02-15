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

## 2026-02-15: Cloud Build + Cloud Run Deploy Completed (Phase 0)

### Cloud Build (Artifact Registry)

Built and pushed the Laravel image to Artifact Registry (repo `openagents-web`, region `us-central1`):

```bash
gcloud builds submit \
  --config apps/openagents.com/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/openagents.com
```

Tags currently present for the image:

- `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:393fec274`
- `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest`

(These resolve to the same digest at time of writing.)

### Cloud Run Service: `openagents-web`

Deployed Cloud Run service:

- Project: `openagentsgemini`
- Region: `us-central1`
- Service: `openagents-web`
- Image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest`
- Cloud SQL: `openagentsgemini:us-central1:l402-aperture-db`
- Postgres DB/user: `openagents_web` / `openagents_web`

Service URL:

- `https://openagents-web-ezxz4mgdsq-uc.a.run.app`

Health check:

```bash
curl -i https://openagents-web-ezxz4mgdsq-uc.a.run.app/up
```

### Cloud Run Job: `openagents-migrate`

The first deploy returned HTTP 500 on `/` because the Laravel starter kit uses DB-backed sessions and the `sessions` table did not exist yet.

Created and executed a Cloud Run Job to run migrations (no shell access needed):

```bash
gcloud run jobs create openagents-migrate \
  --project openagentsgemini \
  --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr,DB_CONNECTION=pgsql,DB_HOST=/cloudsql/openagentsgemini:us-central1:l402-aperture-db,DB_DATABASE=openagents_web,DB_USERNAME=openagents_web" \
  --set-secrets "APP_KEY=openagents-web-app-key:latest,DB_PASSWORD=openagents-web-db-password:latest" \
  --set-cloudsql-instances "openagentsgemini:us-central1:l402-aperture-db" \
  --command php \
  --args artisan,migrate,--force

gcloud run jobs execute openagents-migrate \
  --project openagentsgemini \
  --region us-central1 \
  --wait
```

Execution (example): `openagents-migrate-94477`

Note: GA `gcloud` does not currently ship `gcloud run jobs executions logs read`. For execution logs:

```bash
gcloud beta run jobs executions logs read openagents-migrate-94477 \
  --project openagentsgemini \
  --region us-central1
```

### Post-migration Verification

After migrations:

```bash
curl -i https://openagents-web-ezxz4mgdsq-uc.a.run.app/
```

Now returns HTTP 200 (welcome page).

### Local Verification (Laravel app)

```bash
cd apps/openagents.com
composer test
npm run lint
npm run types
npm run build
```

All passing at time of writing.

## 2026-02-15: Phase 1 (Streaming Chat MVP)

### Backend (Laravel)

Changes in `apps/openagents.com/`:

- Installed `laravel/ai` and published config:
  - `apps/openagents.com/config/ai.php`
  - Default provider is now controlled by `AI_DEFAULT` (defaults to `openrouter`).
- Added AI env placeholders:
  - `apps/openagents.com/.env.example` now includes `AI_DEFAULT` and `OPENROUTER_API_KEY`.
- Added AI conversation persistence tables (copied vendor migration into app so it is tracked):
  - `apps/openagents.com/database/migrations/2026_01_11_000001_create_agent_conversations_table.php`

Minimal agent + endpoints:

- Minimal agent:
  - `apps/openagents.com/app/AI/Agents/AutopilotAgent.php`
  - Uses `RemembersConversations` so prior messages are included in context and stored via the built-in `ConversationStore` middleware.
- Chat page route + API route:
  - `apps/openagents.com/routes/web.php`
  - `GET /chat/{conversationId?}` renders the chat UI and loads persisted messages.
  - `POST /api/chat?conversationId=...` streams SSE using Vercel AI SDK data stream protocol.
- CSRF:
  - `apps/openagents.com/bootstrap/app.php` exempts `api/chat` from CSRF validation so the Vercel client transport can POST without Laravel’s CSRF header requirements.

Controllers:

- `apps/openagents.com/app/Http/Controllers/ChatPageController.php`
  - If no `conversationId` is present, creates a conversation row and redirects to `/chat/{id}`.
  - Loads prior messages from DB and passes them to the Inertia page.
- `apps/openagents.com/app/Http/Controllers/ChatApiController.php`
  - Validates `conversationId` belongs to the current user.
  - Uses the last user message from the Vercel UI message array as the next prompt.
  - Streams via `AutopilotAgent::stream(...)->usingVercelDataProtocol()`.

### Frontend (Inertia + React)

- Added Chat page:
  - `apps/openagents.com/resources/js/pages/chat.tsx`
  - Uses `useChat` from `@ai-sdk/react` + `DefaultChatTransport` from `ai`.
  - Manages input locally and calls `sendMessage({ text })`.
  - Renders messages by combining `parts` (text/reasoning) to plain text.
- Added nav entry:
  - `apps/openagents.com/resources/js/components/app-sidebar.tsx` now links to `/chat`.

### Tests

- Added a feature test proving stream protocol and persistence work (no external API key needed):
  - `apps/openagents.com/tests/Feature/ChatStreamingTest.php`
  - Uses `Laravel\Ai\Ai::fakeAgent(...)` and asserts the SSE includes:
    - `data: {"type":"start"...}`
    - `data: {"type":"text-delta"...}`
    - `data: {"type":"finish"...}`
    - `data: [DONE]`
  - Asserts 2 rows are written to `agent_conversation_messages`.

### Verification

```bash
cd apps/openagents.com
composer test
npm run lint
npm run types
npm run build
```
