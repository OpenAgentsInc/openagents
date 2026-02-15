# OpenAgents Laravel App – GCP (Cloud Run) Deploy Plan

Date: 2026-02-15
Scope: Deploy `apps/openagents.com` (Laravel 12 + Inertia + React) to Google Cloud in the same style as our existing Cloud Run services (Aperture, wallet executor). PHP and runtime are configured production-style (PHP-FPM + Nginx), analogous to what Laravel Forge does on VPS.

**Reference runbooks (same project/region/patterns):**
- `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`
- `docs/lightning/runbooks/L402_WALLET_EXECUTOR_DEPLOY_RUNBOOK.md`
- `docs/lightning/deploy/README.md`

---

## 1. Goal

- Run the Laravel web app on **Google Cloud Run** (project `openagentsgemini`, region `us-central1`).
- Use **PHP-FPM + Nginx** in a single container so PHP is configured the same way as on Forge/VPS (no `php artisan serve` in production).
- Use **Cloud SQL (Postgres)** for Laravel’s database; **Secret Manager** for `APP_KEY`, DB password, WorkOS, etc.
- Optional: **Redis** (Memorystore) for queues/cache; **queue workers** and **scheduler** as separate steps (Cloud Run Jobs or Cloud Scheduler).
- Support **SSE** for chat streaming (Nginx and Cloud Run must not buffer; see §4).

---

## 2. Alignment With Existing GCP Deploys

We already deploy to GCP as follows:

| Item | Existing (Aperture / wallet executor) | Laravel app (this plan) |
|------|----------------------------------------|--------------------------|
| **Project** | `openagentsgemini` | Same |
| **Region** | `us-central1` | Same |
| **Artifact Registry** | `us-central1-docker.pkg.dev/openagentsgemini/l402/...` | New repo e.g. `openagents-web` → `.../openagents-web/laravel:latest` |
| **Build** | `gcloud builds submit --config path/to/cloudbuild.yaml` from repo root | Same pattern; config and Dockerfile live under `apps/openagents.com/` |
| **Secrets** | Secret Manager; `--set-secrets` for env or file mounts | Same; `APP_KEY`, `DB_PASSWORD`, WorkOS, etc. |
| **Cloud Run** | `gcloud run deploy SERVICE --image=... --region=us-central1` | Same; service name e.g. `laravel-web` or `openagents-web` |
| **Custom domain** | `gcloud beta run domain-mappings create` (e.g. `l402.openagents.com`) | Same; e.g. `app.openagents.com` or target domain |
| **Cloud SQL** | Aperture uses `l402-aperture-db` (Postgres) | New instance or new DB for Laravel (e.g. `laravel-web-db`) |

No new project or region; we add a new Artifact Registry repo, a new Cloud Run service, and (recommended) a dedicated Cloud SQL instance or database for Laravel.

---

## 3. Runtime Stack (PHP / Nginx – “Forge-style”)

Laravel Forge typically uses on a VPS:

- **PHP 8.2+** (FPM)
- **Nginx** (reverse proxy + static files)
- **Postgres** or MySQL
- **Redis** (queues, cache, sessions)
- **Queue workers** (long-running processes)
- **Scheduler** (cron → `php artisan schedule:run`)

On Cloud Run we approximate this with a **single container** that runs **Nginx + PHP-FPM**:

- **One process group:** Nginx listens on `PORT` (8080); Nginx forwards `*.php` to PHP-FPM over a socket or TCP.
- **Static assets:** Built at image build time (`npm run build` → `public/build/`); Nginx serves `public/` directly.
- **No long-running queue in the same container:** Queue workers and scheduler are handled separately (see §7).

### 3.1 PHP Version and Extensions

- **PHP:** 8.2+ (match `composer.json`: `"php": "^8.2"`).
- **Extensions:** Include at least `pdo_pgsql`, `mbstring`, `openssl`, `tokenizer`, `xml`, `ctype`, `json`, `bcmath`, `fileinfo`, `zip`, `pcntl` (for queue), `gd` or `imagick` if needed. Use `php:8.2-fpm-bookworm` or similar as base and install extensions via `docker-php-ext-install` / PECL as needed.

### 3.2 Nginx Configuration

- Listen on `PORT` (Cloud Run sets `PORT=8080`); pass PHP requests to PHP-FPM (e.g. `fastcgi_pass 127.0.0.1:9000` or unix socket).
- **SSE / streaming:** For routes that stream (e.g. `/api/chat/stream`), disable buffering so responses are flushed immediately:
  - `proxy_buffering off;`
  - `fastcgi_buffering off;`
  - `fastcgi_read_timeout` large enough for long-lived streams.
- Root document: Laravel’s `public/` (so `index.php` and `public/build/` are correct).

### 3.3 Laravel App in Container

- **Build stage:** From repo root (or app dir), run `composer install --no-dev --optimize-autoloader`, then `npm ci && npm run build`, then copy app into a minimal runtime image.
- **Runtime:** No `.env` file with secrets; inject via **env vars** (from Cloud Run env and Secret Manager `--set-secrets`). Laravel reads `APP_KEY`, `DB_*`, `REDIS_*`, `WORKOS_*`, etc. from the environment.
- **Storage/cache:** Cloud Run filesystem is ephemeral. Use `storage/logs` and `storage/framework/cache` only for best-effort; prefer **Redis** for cache/sessions and **Cloud SQL** for durable state. If no Redis initially, file-based cache/session are acceptable for MVP with the caveat that they don’t persist across instances.

---

## 4. SSE and Streaming

The Laravel rebuild plan uses **SSE** for chat streaming. To avoid buffering:

1. **Nginx:** For the SSE route (e.g. `/api/chat/stream`), set `proxy_buffering off;` and `fastcgi_buffering off;`, and a high `fastcgi_read_timeout`.
2. **Cloud Run:** By default Cloud Run can buffer; ensure the response is actually streamed (chunked). Laravel’s streaming response should send chunks as they’re generated.
3. **Load balancer:** If we put a load balancer in front later, it must support streaming (no buffering of response body).

Document the exact SSE path(s) in Nginx so future changes don’t re-enable buffering.

---

## 5. GCP Resources to Create (Checklist)

### 5.1 APIs and Artifact Registry

- Enable (if not already): **Cloud Build**, **Artifact Registry**, **Cloud Run**, **Secret Manager**, **Cloud SQL Admin**.
- Create Artifact Registry repo, e.g.:
  - `openagents-web` in region `us-central1`.
  - Image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest`.

### 5.2 Cloud SQL (Postgres)

- **Option A:** New instance `laravel-web-db` (Postgres 15, us-central1). Dedicated DB and user for Laravel (e.g. database `laravel`, user `laravel`).
- **Option B:** New database and user on an existing instance (if acceptable for isolation/cost).
- Store DB password in Secret Manager (e.g. `laravel-web-db-password`).
- For production hardening: use **Private IP + VPC** or **Cloud SQL Auth Proxy**; Cloud Run can use **VPC connector** (we already have `oa-serverless-us-central1` for Aperture) to reach private IP, or use the instance’s public IP with authorized networks (simpler for first deploy).

### 5.3 Secrets (Secret Manager)

Create and populate (no values in repo):

| Secret name | Purpose |
|-------------|---------|
| `laravel-web-app-key` | Laravel `APP_KEY` (e.g. `base64:...`) |
| `laravel-web-db-password` | Cloud SQL Postgres password for Laravel user |
| `laravel-web-workos-client-id` | WorkOS client ID |
| `laravel-web-workos-api-key` | WorkOS API key (server-side) |

Add more as needed (e.g. Redis auth, third-party API keys).

### 5.4 Optional: Redis (Memorystore)

- For queues, cache, and sessions: create a **Memorystore for Redis** instance in `us-central1` and connect Cloud Run via VPC connector. If skipped for MVP, use `file` or `database` driver for cache/session with the caveat that they are not shared across instances.

---

## 6. Build and Deploy Steps

### 6.1 Dockerfile Location and Shape

- **Location:** e.g. `apps/openagents.com/Dockerfile` (or `apps/openagents.com/deploy/Dockerfile`).
- **Build context:** Repo root so we can copy `apps/openagents.com/` and run composer/npm from app dir. If the app has no monorepo dependencies outside `apps/openagents.com/`, context can be `apps/openagents.com/` and paths in Dockerfile adjusted.
- **Multi-stage:**
  1. **Stage 1 (builder):** PHP 8.2 + Composer + Node; copy app, `composer install --no-dev`, `npm ci && npm run build`; output: `vendor/`, `public/build/`, app code.
  2. **Stage 2 (runtime):** Slim image with PHP 8.2-FPM + Nginx; copy `vendor/`, `public/`, app code; set correct permissions for `storage` and `bootstrap/cache`; Nginx listens on `PORT`; start script runs both Nginx and PHP-FPM (e.g. a small shell script that starts php-fpm in background then `exec nginx -g 'daemon off;'`).

### 6.2 Cloud Build Config

- **Location:** e.g. `apps/openagents.com/deploy/cloudbuild.yaml` (or at repo root pointing at app dir).
- **Steps:** Build Docker image with tag `us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:${_TAG}` and `:latest`; push both. Use `_TAG=$(git rev-parse --short HEAD)` when invoking from repo root.
- **Build context:** Repo root if Dockerfile copies from monorepo; otherwise context = `apps/openagents.com/`.

Example (adjust paths to actual layout):

```yaml
# apps/openagents.com/deploy/cloudbuild.yaml
# Run from repo root:
#   gcloud builds submit --config apps/openagents.com/deploy/cloudbuild.yaml \
#     --substitutions _TAG="$(git rev-parse --short HEAD)" .
substitutions:
  _TAG: ${SHORT_SHA}
steps:
  - name: "gcr.io/cloud-builders/docker"
    args:
      - "build"
      - "-f"
      - "apps/openagents.com/Dockerfile"
      - "-t"
      - "us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:${_TAG}"
      - "-t"
      - "us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:latest"
      - "."
  - name: "gcr.io/cloud-builders/docker"
    args: ["push", "us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:${_TAG}"]
  - name: "gcr.io/cloud-builders/docker"
    args: ["push", "us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:latest"]
images:
  - "us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:${_TAG}"
  - "us-central1-docker.pkg.dev/${PROJECT_ID}/openagents-web/laravel:latest"
options:
  logging: CLOUD_LOGGING_ONLY
  substitutionOption: ALLOW_LOOSE
```

### 6.3 Deploy Cloud Run (Copy-Paste Reference)

Replace placeholders with actual values. Use **Cloud SQL connection** if using private IP or socket (e.g. `--add-cloudsql-instances=openagentsgemini:us-central1:laravel-web-db`); then in Laravel, set `DB_HOST` to `/cloudsql/openagentsgemini:us-central1:laravel-web-db` for socket, or to the instance IP for TCP.

```bash
export PROJECT=openagentsgemini
export REGION=us-central1
export IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest

gcloud run deploy laravel-web \
  --project "$PROJECT" \
  --region "$REGION" \
  --image "$IMAGE" \
  --platform managed \
  --allow-unauthenticated \
  --port 8080 \
  --memory 1Gi \
  --cpu 1 \
  --min-instances 0 \
  --max-instances 4 \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr" \
  --set-secrets "APP_KEY=laravel-web-app-key:latest,DB_PASSWORD=laravel-web-db-password:latest,WORKOS_CLIENT_ID=laravel-web-workos-client-id:latest,WORKOS_API_KEY=laravel-web-workos-api-key:latest" \
  --add-cloudsql-instances "openagentsgemini:us-central1:laravel-web-db"
```

If using **VPC connector** (e.g. to reach Redis or private Cloud SQL IP):

```bash
  --vpc-connector=oa-serverless-us-central1 \
  --vpc-egress=private-ranges-only
```

Set other env vars as needed: `DB_CONNECTION=pgsql`, `DB_HOST`, `DB_PORT`, `DB_DATABASE`, `DB_USERNAME`, `REDIS_HOST`, etc. For Cloud SQL socket, `DB_HOST` is the socket path above; for public IP, set `DB_HOST` to the instance IP and ensure the instance has an authorized network for Cloud Run’s egress IPs or use Private IP.

### 6.4 First-Time Migrations

Cloud Run containers are stateless. Run migrations **outside** the request path:

- **Option A:** One-off Cloud Run Job that runs `php artisan migrate --force` using the same image and env/secrets.
- **Option B:** CI step or manual: connect to Cloud SQL (e.g. Cloud SQL Proxy or authorized network) and run migrations from a dev machine or a small script in Cloud Build (second step that runs a migration container with same env).
- **Option C:** A dedicated “migrate” Cloud Run Job that is triggered on deploy (e.g. from Cloud Build); the job runs once and exits.

Do **not** run migrations in the web container’s startup script at scale (race conditions); run them once per deploy via Job or CI.

### 6.5 Custom Domain

- Map a domain (e.g. `app.openagents.com` or `openagents.com`) to the Cloud Run service:
  - `gcloud beta run domain-mappings create --service=laravel-web --domain=app.openagents.com --region=us-central1`
- Add the CNAME (or A/AAAA) records shown by `gcloud beta run domain-mappings describe ...` to your DNS.
- Ensure domain verification is done in Google Cloud if required.

---

## 7. Queue Workers and Scheduler (Post-MVP)

- **Queues:** Laravel queue workers are long-running. On Cloud Run we don’t run them inside the web container. Options:
  - **Cloud Run Jobs:** Run a container that executes `php artisan queue:work` (or a single job) and exits; trigger the Job on a schedule or from Pub/Sub for “run worker for N seconds.”
  - **Separate Cloud Run service:** A second service that runs `queue:work` with `--max-jobs=1` or similar and scales to zero when idle (more complex).
  - **GCE VM:** One small VM that runs `queue:work` and `schedule:run` (most Forge-like).
- **Scheduler:** Laravel’s `schedule:run` must be invoked every minute. Options:
  - **Cloud Scheduler:** HTTP target to a protected Laravel route (e.g. `POST /internal/schedule-run` with a secret token) that runs `Artisan::call('schedule:run')`.
  - **Cloud Run Job:** Scheduled Job that runs `php artisan schedule:run` every minute (possible but more moving parts).

Document the chosen approach in a short “Queue and scheduler” section in this doc or a follow-up runbook once implemented.

---

## 8. Verification

- **Health:** Expose a route (e.g. `/up` or `/health`) that returns 200 and optionally checks DB connectivity. Use it for Cloud Run startup probe if needed.
- **Smoke:** After deploy, open the Cloud Run URL (or custom domain) and confirm the app loads (e.g. login or welcome page).
- **SSE:** If chat streaming is implemented, verify the SSE endpoint streams chunks without buffering (e.g. with `curl -N` or browser devtools).
- **Logs:** `gcloud run services logs read laravel-web --region=us-central1 --limit=50`.

---

## 9. Security and Repo

- **No secrets in repo:** Only secret **names** and **procedures** (e.g. “create `laravel-web-app-key` in Secret Manager and add a version with the Laravel key”). Never commit `APP_KEY`, DB passwords, or WorkOS keys.
- **Gitignore:** Keep `.env` and any local override files ignored; use `.env.example` as a template with placeholder names only.
- **Rotation:** Document how to rotate `APP_KEY`, DB password, and WorkOS credentials (new secret version, then redeploy or restart revision).

---

## 10. Summary Checklist (Ordered)

1. **GCP:** Create Artifact Registry repo `openagents-web`; create Cloud SQL instance (or DB+user); create Secret Manager secrets and add initial versions.
2. **Repo:** Add `apps/openagents.com/Dockerfile` (PHP-FPM + Nginx, multi-stage, listen on `PORT`); add Nginx config with SSE-friendly settings for stream routes; add `apps/openagents.com/deploy/cloudbuild.yaml`.
3. **Build:** Run Cloud Build from repo root; confirm image is in Artifact Registry.
4. **Deploy:** Run `gcloud run deploy laravel-web ...` with env and secrets; if using Cloud SQL, add `--add-cloudsql-instances` or VPC and set `DB_*` accordingly.
5. **Migrations:** Run `php artisan migrate --force` once (Job or CI) before or right after first deploy.
6. **Domain:** Map custom domain to `laravel-web`; update DNS; verify TLS.
7. **Verify:** Hit health URL and main app; test SSE if applicable; check logs.
8. **Later:** Add queue worker and scheduler strategy (Cloud Run Jobs, Cloud Scheduler, or VM) and document in this doc or a runbook.

---

## 11. Related Docs

- **Laravel rebuild plan:** `docs/plans/active/laravel-rebuild.md` (architecture, streaming, tools).
- **Existing GCP runbooks:** `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`, `docs/lightning/runbooks/L402_WALLET_EXECUTOR_DEPLOY_RUNBOOK.md`.
- **Deploy index:** `docs/lightning/deploy/README.md` (build/push pattern for our Cloud Run services).
