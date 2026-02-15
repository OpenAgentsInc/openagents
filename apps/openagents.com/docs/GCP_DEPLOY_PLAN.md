# OpenAgents Laravel App – GCP (Cloud Run) Deploy Plan

Date: 2026-02-15
Scope: Deploy `apps/openagents.com` (Laravel 12 + Inertia + React) to Google Cloud.

This plan intentionally mirrors "Forge-style" separation (web, workers, scheduler, websockets) but maps it onto **Cloud Run services + Cloud Run jobs**.

**Reference runbooks (same project/region/patterns):**
- `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`
- `docs/lightning/runbooks/L402_WALLET_EXECUTOR_DEPLOY_RUNBOOK.md`
- `docs/lightning/deploy/README.md`

---

## 1. Goal

- Run the Laravel web app on **Google Cloud Run** (project `openagentsgemini`, region `us-central1`).
- Use a production runtime: **Nginx + PHP-FPM** (no `php artisan serve` in prod).
- Use **Cloud SQL (Postgres)** for durable state.
- Use **Redis (Memorystore)** for cache/queues/sessions and (later) Reverb multi-instance coordination.
- Support:
  - **SSE** (chat streaming)
  - **WebSockets** (Reverb)
- Keep builds simple: **one image** with different Cloud Run entrypoints.

---

## 2. Cloud Run Topology (Forge → Cloud Run)

Forge separation works because each component has different performance and lifecycle needs. Cloud Run should mirror that.

### 2.1 Services / Jobs

| Component | Cloud Run target | Purpose | Notes |
|---|---|---|---|
| Web | **Service** `openagents-web` | Inertia UI + REST APIs + SSE streaming | Nginx + PHP-FPM. No long-running worker loops. |
| WebSockets | **Service** `openagents-reverb` | `php artisan reverb:start` | Give it its own timeout, scaling, and domain. |
| DB migrations | **Job** `openagents-migrate` | `php artisan migrate --force` | Run once per deploy. |
| Queue draining | **Job** `openagents-queue` | `php artisan queue:work ...` | Use `--stop-when-empty` and max-time so job exits. |
| Scheduler | **Job** `openagents-scheduler` | `php artisan schedule:run` | Trigger every minute via Cloud Scheduler. |

### 2.2 Domain Strategy

Recommended:

- `app.openagents.com` → `openagents-web`
- `ws.openagents.com` → `openagents-reverb`

Avoid proxying WS through the web Nginx unless forced. Separate domains reduce buffering/upgrade-header footguns.

---

## 3. Redis on GCP (Memorystore) + Cloud Run Networking

Redis is used for:

- `CACHE_STORE=redis` (recommended)
- `QUEUE_CONNECTION=redis` (recommended)
- `SESSION_DRIVER=redis` (recommended)
- Reverb coordination when we run more than 1 Reverb instance

### 3.1 Preferred: Direct VPC Egress

Cloud Run now supports **Direct VPC egress**.

Mechanically this means:

1. Create (or reuse) a VPC and a /26+ subnet in `us-central1`.
2. Create Memorystore Redis in the same VPC/subnet.
3. Deploy Cloud Run services with:
   - `--network=<VPC_NAME>`
   - `--subnet=<SUBNET_NAME>`

Then set env vars:

- `REDIS_HOST=<redis-private-ip>`
- `REDIS_PORT=6379`
- `CACHE_STORE=redis`
- `QUEUE_CONNECTION=redis`
- `SESSION_DRIVER=redis` (optional)

### 3.2 Acceptable: Serverless VPC Connector (If Already Standard)

If we already standardized on a Serverless VPC connector for other services, we can keep using:

- `--vpc-connector=<connector>`
- `--vpc-egress=private-ranges-only`

Direct VPC egress is the better default when setting up fresh.

---

## 4. Reverb on Cloud Run

Reverb is a long-running WebSocket server.

Cloud Run gotchas:

- **Timeout:** WebSocket connections are still subject to Cloud Run request timeouts (max 60 minutes). Deploy Reverb with `--timeout=3600` and ensure clients reconnect.
- **Scaling:** If Cloud Run scales instances, clients will disconnect/reconnect. Design for reconnect.
- **Multi-instance:** For >1 Reverb instance, use Redis for coordination.

### 4.1 Reverb Config (Env/Secrets)

Secrets to create in Secret Manager:

- `openagents-reverb-app-id`
- `openagents-reverb-key`
- `openagents-reverb-secret`

Web service env (broadcasting client config):

- `BROADCAST_CONNECTION=reverb`
- `REVERB_APP_ID=...`
- `REVERB_APP_KEY=...`
- `REVERB_APP_SECRET=...`
- `REVERB_HOST=ws.openagents.com`
- `REVERB_PORT=443`
- `REVERB_SCHEME=https`

Reverb service env:

- same `REVERB_*`
- plus Redis env if scaling:
  - `REDIS_HOST`, `REDIS_PORT`, etc.

Note: `apps/openagents.com/` does not yet have Reverb wired/config published. This plan assumes we will add `laravel/reverb` and publish/configure it before turning on the Reverb service.

---

## 5. Container Strategy: One Image, Multiple Entrypoints

Build **one** container image and deploy it with different commands:

- Web service: runs **Nginx + PHP-FPM**
- Reverb service: runs `php artisan reverb:start --host=0.0.0.0 --port=8080`
- Queue job: runs `php artisan queue:work ...`
- Scheduler job: runs `php artisan schedule:run`
- Migrate job: runs `php artisan migrate --force`

Cloud Run supports overriding `--command` and `--args` per service/job.

---

## 6. Runtime Stack (Nginx + PHP-FPM)

### 6.1 PHP Version + Extensions

- PHP: 8.4+ (match `composer.json`)
- Common extensions: `pdo_pgsql`, `pdo_sqlite`, `mbstring`, `openssl`, `tokenizer`, `xml`, `ctype`, `json`, `bcmath`, `fileinfo`, `zip`, `pcntl`

### 6.2 Nginx Config Requirements

- Listen on `PORT` (Cloud Run sets `PORT=8080`).
- Root: `public/`.
- PHP: FastCGI to PHP-FPM.
- **SSE streaming:** disable buffering for streaming routes:
  - `fastcgi_buffering off;`
  - `proxy_buffering off;`
  - large `fastcgi_read_timeout`

---

## 7. GCP Resources

### 7.1 APIs

Enable if missing:

- Cloud Run
- Cloud Build
- Artifact Registry
- Secret Manager
- Cloud SQL Admin
- Memorystore (Redis) (when ready)

### 7.2 Artifact Registry

Create a Docker repo:

- `openagents-web` (region `us-central1`)

Example image tags:

- `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:<git-sha>`
- `...:latest`

### 7.3 Cloud SQL (Postgres)

Option A (recommended for isolation): new instance `openagents-web-db`.

Option B (fastest): add a database + user on existing instance (`l402-aperture-db`).

### 7.4 Secrets (Secret Manager)

Create/populate:

- `openagents-web-app-key` (Laravel `APP_KEY`)
- `openagents-web-db-password` (DB user password)
- `openagents-web-workos-client-id`
- `openagents-web-workos-api-key`

Plus Reverb secrets when Reverb is enabled.

---

## 8. Build and Deploy (Commands)

### 8.1 Cloud Build

Use a Cloud Build config like `apps/openagents.com/deploy/cloudbuild.yaml` and build from repo root:

```bash
gcloud builds submit \
  --config apps/openagents.com/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  .
```

### 8.2 Deploy Web Service

```bash
export PROJECT=openagentsgemini
export REGION=us-central1
export IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest

gcloud run deploy openagents-web \
  --project "$PROJECT" \
  --region "$REGION" \
  --image "$IMAGE" \
  --allow-unauthenticated \
  --port 8080 \
  --memory 1Gi \
  --cpu 1 \
  --min-instances 0 \
  --max-instances 4 \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr" \
  --set-secrets "APP_KEY=openagents-web-app-key:latest,DB_PASSWORD=openagents-web-db-password:latest,WORKOS_CLIENT_ID=openagents-web-workos-client-id:latest,WORKOS_API_KEY=openagents-web-workos-api-key:latest" \
  --add-cloudsql-instances "openagentsgemini:us-central1:<INSTANCE>"
```

If using Direct VPC egress:

- add: `--network=<VPC_NAME> --subnet=<SUBNET_NAME>`

### 8.3 Deploy Reverb Service

```bash
gcloud run deploy openagents-reverb \
  --project openagentsgemini \
  --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest \
  --allow-unauthenticated \
  --port 8080 \
  --timeout 3600 \
  --min-instances 1 \
  --max-instances 10 \
  --concurrency 200 \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr" \
  --set-secrets "APP_KEY=openagents-web-app-key:latest,REVERB_APP_ID=openagents-reverb-app-id:latest,REVERB_APP_KEY=openagents-reverb-key:latest,REVERB_APP_SECRET=openagents-reverb-secret:latest" \
  --command php \
  --args artisan,reverb:start,--host=0.0.0.0,--port=8080
```

### 8.4 Cloud Run Jobs

Migrations (run once per deploy):

```bash
gcloud run jobs create openagents-migrate \
  --project openagentsgemini \
  --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr" \
  --set-secrets "APP_KEY=openagents-web-app-key:latest,DB_PASSWORD=openagents-web-db-password:latest" \
  --add-cloudsql-instances "openagentsgemini:us-central1:<INSTANCE>" \
  --command php \
  --args artisan,migrate,--force

gcloud run jobs execute openagents-migrate --project openagentsgemini --region us-central1
```

Scheduler (trigger every minute via Cloud Scheduler):

```bash
gcloud run jobs create openagents-scheduler \
  --project openagentsgemini \
  --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr" \
  --set-secrets "APP_KEY=openagents-web-app-key:latest" \
  --command php \
  --args artisan,schedule:run
```

Queue draining job (recommended pattern):

```bash
gcloud run jobs create openagents-queue \
  --project openagentsgemini \
  --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest \
  --set-env-vars "APP_ENV=production,APP_DEBUG=0,LOG_CHANNEL=stderr" \
  --set-secrets "APP_KEY=openagents-web-app-key:latest" \
  --command php \
  --args artisan,queue:work,--sleep=1,--tries=3,--max-time=840,--stop-when-empty
```

---

## 9. Multi-Tenant: User Sites on Subdomains (Separate Installs)

We will eventually want **user sites** deployable at `<tenant>.openagents.com`, each with its own "Laravel install".

There are two credible approaches:

### 9.1 Separate Cloud Run Service Per Tenant (Matches "Own Install")

- A "control plane" provisions a new Cloud Run service per tenant using the same base image.
- Each tenant gets:
  - its own `APP_KEY` secret
  - its own Cloud SQL database (or its own Cloud SQL instance for high-value tenants)
  - its own domain mapping (`<tenant>.openagents.com` → that tenant’s service)
  - optional: its own Redis namespace/prefix (or dedicated Redis for large tenants)

Pros:

- Hard isolation between tenants (blast radius).
- Per-tenant scaling/quotas.
- Matches the mental model of "their own install".

Cons:

- Many Cloud Run services/domain mappings.
- Requires automation (Terraform or an internal provisioner).

### 9.2 Single App Multi-Tenancy (Not "Separate Install")

- One service handles all tenants and routes by Host header.
- Tenant isolation is implemented in-app (DB schemas/prefixes) with a tenancy framework.

This is operationally simpler but does **not** match "own install".

**Recommendation:** start with 9.1 for the user-site product vision.

Minimum automation plan for 9.1:

1. Provision database + user.
2. Create secrets.
3. Deploy Cloud Run service from base image with per-tenant env/secrets.
4. Create domain mapping.
5. Run migrations for that tenant.

We should decide early whether this is done via:

- Terraform (preferred for drift control), or
- a provisioner service (calls GCP APIs).

---

## 10. Verification

- Health endpoint (e.g. `/up` or `/health`).
- Basic HTTP smoke: the welcome page loads.
- Streaming smoke (when chat exists): `curl -N` shows incremental SSE frames.
- Logs:
  - `gcloud run services logs read openagents-web --region us-central1 --limit 100`

---

## 11. Security Notes

- Never commit secret values.
- Only commit secret **names** and setup procedures.
- Prefer Secret Manager `--set-secrets` rather than env values in deploy scripts.

