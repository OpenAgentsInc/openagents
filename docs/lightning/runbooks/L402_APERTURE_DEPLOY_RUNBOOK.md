# L402 Aperture Deploy – Runbook and Reference

**This document is the single source of truth for the OpenAgents L402 paywall gateway: how it works, what was built, how to use it, and how to change it.** The codebase is public; no secrets or sensitive values are stored in the repo.

---

## Current state (as of 2026-02-14)

- **Canonical URL:** `https://l402.openagents.com` — custom domain mapped to Cloud Run (`l402-aperture`, us-central1). DNS: CNAME `l402` → `ghs.googlehosted.com`.
- **Staging route:** Aperture config includes a **staging** service (host `l402.openagents.com`, path `^/staging(?:/.*)?$`). Requests to `https://l402.openagents.com/staging` return 402 and support the full L402 flow.
- **EP212 demo routes:** Aperture config includes:
- `https://l402.openagents.com/ep212/premium-signal` (`price: 10`, under-cap success route)
  - `https://l402.openagents.com/ep212/expensive-signal` (`price: 250`, over-cap quote route)
- **Script defaults:** `apps/lightning-ops/scripts/staging-reconcile.sh` sets `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`, `OA_LIGHTNING_OPS_CHALLENGE_URL`, and `OA_LIGHTNING_OPS_PROXY_URL` to the canonical URL and `/staging` when unset. You only need to set `OA_LIGHTNING_OPS_CONVEX_URL` and `OA_LIGHTNING_OPS_SECRET` to run staging reconcile.
- **Config deployed:** Secret `l402-aperture-config` (latest) is live; Cloud Run revision uses it. To change routes or config, see §6.

### Backend note (important)

This runbook originally documented a **Voltage-backed** LND connection. We are migrating to a **self-hosted GCP Bitcoin Core + LND** foundation and treating Voltage references as deprecated.

Target backend for the gateway:

- Seller LND backend: `oa-lnd` (GCP VM on VPC `oa-lightning`, gRPC `10.42.0.3:10009`)
- Cloud Run must use the Serverless VPC Access connector `oa-serverless-us-central1` to reach the VM (private ranges egress).

---

## 1. What This System Is

- **L402** is the protocol for “pay with Lightning” over HTTP: the server returns `402 Payment Required` with an invoice and macaroon; the client pays the invoice, gets a preimage, and retries with `Authorization: L402 macaroon=… preimage=…` to access the resource.
- **Aperture** (Lightning Labs) is the L402 reverse proxy: it sits in front of your backend, talks to an LND node to create invoices and verify payments, and proxies authenticated requests to upstream services.
- **This deploy** runs Aperture on **Google Cloud Run**, backed by:
  - **Self-hosted GCP LND** (`oa-lnd`) for the LND connection (invoice issuance + verification).
  - **Cloud SQL Postgres** for Aperture’s token/state (SQLite does not work on Cloud Run’s filesystem).
- **OpenAgents** uses this as the seller/paywall side; the compiler in `apps/lightning-ops` produces route config that can be merged into Aperture’s config.

---

## 2. Architecture (Text)

```
[Client] --> HTTPS --> [Cloud Run: l402-aperture]
                            |
                            | reads config + secrets from mounts
                            v
              +-------------+-------------+
              |                           |
              v                           v
     [GCP VM: oa-lnd (gRPC)]      [Cloud SQL Postgres]
     10.42.0.3:10009              l402-aperture-db
     (invoice macaroon)           (tokens, migrations)
              |                           ^
              | TLS + macaroon            | DSN in config
              | (from Secret Manager)     |
              +---------------------------+
```

- **Cloud Run** runs the Aperture binary with:
  - **Config** from Secret Manager (`l402-aperture-config`) mounted at `/aperture-cfg/config.yaml`.
  - **LND TLS cert** and **invoice macaroon** from Secret Manager mounted at `/lnd-tls/tls.cert` and `/lnd-mac/invoice.macaroon`.
- **Aperture** connects to `oa-lnd` over the VPC (gRPC + TLS via Serverless VPC Access) and to Cloud SQL Postgres (currently via instance **public IP**; see §6 for production hardening).
- **Routes** (host/path → upstream, price) are defined in the same config. The deployed config has **bootstrap** (host `l402-bootstrap.openagents.local`) and **staging** (host `l402.openagents.com`); more routes can be merged from `apps/lightning-ops` compile.

---

## 3. What Was Built (GCP and Repo)

### 3.1 GCP project: `openagentsgemini`

| Resource type | Name / location | Purpose |
|---------------|-----------------|---------|
| **Secret Manager** | `l402-gcp-lnd-tls-cert` | `oa-lnd` TLS cert (PEM); mounted as file in Cloud Run. |
| **Secret Manager** | `l402-gcp-lnd-invoice-macaroon` | Invoice macaroon from `oa-lnd`; mounted as file. |
| **Secret Manager** | `l402-aperture-config` | Full Aperture YAML (authenticator, postgres, services). **Contains no secrets in the repo**; the *deployed* version has the DB password injected when adding a new secret version (see §5). |
| **Secret Manager** | `l402-aperture-db-password` | Cloud SQL Postgres password for user `aperture`. Used only when building a new config secret version. |
| **Cloud SQL** | Instance `l402-aperture-db` (Postgres 15, us-central1) | Aperture’s database (tokens, migrations). Database name `aperture`, user `aperture`. |
| **Artifact Registry** | Repo `l402` in `us-central1` | Holds the Aperture image `aperture:latest` (built from Lightning Labs source, Go 1.24). |
| **Cloud Run** | Service `l402-aperture`, region `us-central1` | Runs Aperture; receives traffic and mounts the secrets above. |

**Canonical URL:** `https://l402.openagents.com` (custom domain; use this for env and docs.)
**Alternate URL:** `https://l402-aperture-157437760789.us-central1.run.app`

### 3.2 Repo artifacts (all public-safe)

| Path | Purpose |
|------|---------|
| `docs/lightning/scripts/aperture-voltage-config.yaml` | **SQLite** base config (legacy/local only). Cloud Run uses Postgres. |
| `docs/lightning/scripts/aperture-voltage-config-postgres.yaml` | **Legacy** Postgres config template targeting Voltage LND (kept for reference). |
| `docs/lightning/scripts/aperture-gcp-config-postgres.yaml` | **Active** Postgres config template targeting **GCP `oa-lnd`**. **Inject the real DB password when creating a new Secret Manager version** (see §6.1). |
| `docs/lightning/scripts/voltage-api-fetch.sh` | Legacy helper for Voltage API (no longer required for GCP self-hosted LND). |
| `docs/lightning/deploy/Dockerfile.aperture` | Multi-stage build: Aperture from Lightning Labs source (Go 1.24), minimal runtime image. |
| `docs/lightning/deploy/cloudbuild-aperture.yaml` | Cloud Build config to build and push the Aperture image to Artifact Registry (optional; can build locally with Docker). |
| `docs/lightning/reference/VOLTAGE_TO_L402_CONNECT.md` | How Voltage fits into L402, what you need from Voltage, and how to connect it to Aperture. |
| `docs/lightning/runbooks/STAGING_GATEWAY_RECONCILE_RUNBOOK.md` | Staging reconcile and env vars for lightning-ops. |
| `apps/lightning-ops/` | Compiler that produces route config from Convex paywall state; output can be merged into Aperture config. |

**Gitignore:** `output/`, `output/voltage-node/`, and repo root `.env.local` are ignored so that TLS certs, macaroons, and API keys are never committed.

### 3.3 Custom domain (in use)

**Subdomain in use:** `l402.openagents.com` — mapped via Cloud Run domain mapping; CNAME `l402` → `ghs.googlehosted.com`. Use this as the canonical gateway URL everywhere.

To point it at this Cloud Run service:

1. **In Google Cloud**
   - Open [Cloud Run](https://console.cloud.google.com/run) → select service **l402-aperture** (region `us-central1`) → **Manage custom domains**.
   - Click **Add mapping** and enter **`l402.openagents.com`**.
   - Complete any **verification** step (TXT or CNAME for domain ownership if required).
   - Note the **mapping** records Google shows (e.g. CNAME target or A/AAAA values).

2. **In DNS (where `openagents.com` is hosted)**
   - Add the records Google specifies for `l402.openagents.com` (verification + mapping).
   - If using Cloudflare: add the CNAME (or A/AAAA); you can proxy (orange cloud) or DNS-only (grey).
   - Wait for DNS propagation; Cloud Run will then provision TLS for the domain.

3. **After it’s live**
   - Use `https://l402.openagents.com` for `OA_LIGHTNING_OPS_CHALLENGE_URL` / `OA_LIGHTNING_OPS_PROXY_URL` and any docs (see `STAGING_GATEWAY_RECONCILE_RUNBOOK.md`).

**Via gcloud CLI (fully managed Cloud Run):** Use the **beta** command group; the non-beta `gcloud run domain-mappings` is for Cloud Run for Anthos only. Ensure `gcloud components install beta` if needed.

```bash
# Set project and region
gcloud config set project openagentsgemini

# Create the domain mapping (region = us-central1 for l402-aperture)
gcloud beta run domain-mappings create \
  --service=l402-aperture \
  --domain=l402.openagents.com \
  --region=us-central1

# List mappings (optional)
gcloud beta run domain-mappings list --region=us-central1

# Get DNS records to add at your registrar (CNAME or A/AAAA)
gcloud beta run domain-mappings describe \
  --domain=l402.openagents.com \
  --region=us-central1
```

Then add the records shown in the describe output to your DNS for `openagents.com`. If Google requires domain verification first, you may need to verify ownership (e.g. via [Search Console](https://search.google.com/search-console) or the Cloud Console domain verification flow) before the mapping becomes active; the describe output will show status and any required records.

---

## 4. Where Secrets Live (No Values in Repo)

All sensitive values live **only** in GCP Secret Manager (and, for local use, in `.env.local` or env vars). The repo contains **names and procedures only**.

| Secret name | What it holds | Who uses it |
|-------------|----------------|-------------|
| `l402-gcp-lnd-tls-cert` | `oa-lnd` TLS certificate (PEM file contents). | Cloud Run mounts as `/lnd-tls/tls.cert`; Aperture config references this path. |
| `l402-gcp-lnd-invoice-macaroon` | Invoice macaroon file contents from `oa-lnd`. | Cloud Run mounts as `/lnd-mac/invoice.macaroon`; Aperture `macdir` points to `/lnd-mac`. |
| `l402-aperture-config` | Full Aperture YAML used at runtime. **The version deployed** includes the Postgres password (injected when adding a new version; see §6). The *template* in the repo uses `REPLACE_PASSWORD`. | Cloud Run mounts as `/aperture-cfg/config.yaml`; Aperture is started with `--configfile=/aperture-cfg/config.yaml`. |
| `l402-aperture-db-password` | Cloud SQL Postgres password for user `aperture`. | Used only when creating a new version of `l402-aperture-config` (script substitutes it into the YAML). Never mounted into the container directly. |

**Rotation:**

- **LND TLS / macaroon:** Replace the secret version in Secret Manager (e.g. `gcloud secrets versions add l402-gcp-lnd-tls-cert --data-file=./new-tls.cert`), then deploy a new Cloud Run revision.
- **DB password:** Change the password in Cloud SQL for user `aperture`, then create a new secret version of `l402-aperture-db-password` with that value, then build a new `l402-aperture-config` version with the new password and redeploy.
- **Voltage API key:** Deprecated (only used by legacy `voltage-api-fetch.sh`). Do not add new dependencies on this.

---

## 5. How to Use It

### 5.1 Public URL and health

- **Canonical:** `https://l402.openagents.com` — use for `OA_LIGHTNING_OPS_CHALLENGE_URL` / `OA_LIGHTNING_OPS_PROXY_URL` and product config.
- **Fallback:** `https://l402-aperture-157437760789.us-central1.run.app`
- A request to `/` without L402 auth typically returns **400** or similar (no matching service or auth required); that indicates Aperture is up.
- To actually get a 402 and then access a paywalled route, you must request a **host/path that matches a configured service** in Aperture. The deployed config includes:
  - **bootstrap** (host `l402-bootstrap.openagents.local`)
  - **staging** (host `l402.openagents.com`, path `^/staging(?:/.*)?$`) — used for `OA_LIGHTNING_OPS_*` defaults; request `https://l402.openagents.com/staging` to get 402.
  - **ep212-demo-under-cap** (host `l402.openagents.com`, path `^/ep212/premium-signal$`, `price: 10`)
  - **ep212-demo-over-cap** (host `l402.openagents.com`, path `^/ep212/expensive-signal$`, `price: 250`)

### 5.2 L402 flow (conceptual)

1. Client requests a paywalled URL.
2. Aperture responds with `402 Payment Required` and `WWW-Authenticate: L402 macaroon="…" invoice="…"`.
3. Client pays the invoice (e.g. via Lightning wallet), gets the preimage.
4. Client retries with `Authorization: L402 macaroon="…" preimage="…"`.
5. Aperture verifies and proxies to the upstream.

### 5.3 Staging reconcile (lightning-ops)

To verify 402 issuance and proxy against this gateway:

- Set env vars (see `docs/lightning/runbooks/STAGING_GATEWAY_RECONCILE_RUNBOOK.md`):
  `OA_LIGHTNING_OPS_CHALLENGE_URL`, `OA_LIGHTNING_OPS_PROXY_URL` (and optionally `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`) to the canonical gateway URL (`https://l402.openagents.com`) and the paywalled path you configured.
- Run: `./scripts/staging-reconcile.sh` (or the smoke command from `apps/lightning-ops`).

### 5.4 EP212 route verification smoke

Use the dedicated `lightning-ops` command for the two episode routes:

```bash
cd apps/lightning-ops
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="https://<wallet-executor-host>" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="<optional-bearer>" \
npm run smoke:ep212-routes -- --json --mode live
```

Expected summary fields:

- `routeA.challengeStatusCode = 402` and `routeA.paidStatusCode = 200`
- `routeA.paidAmountMsats > 0`
- `routeB.challengeStatusCode = 402`
- `routeB.blocked = true` with `routeB.denyReasonCode = amount_over_cap`
- `routeB.payerCallsBefore === routeB.payerCallsAfter` (no payment attempted on over-cap block)

### 5.5 EP212 buyer full-flow smoke

Use the EP212 buyer harness to validate sats4ai compatibility + cache behavior + OpenAgents route success + over-cap policy block in one command.

```bash
cd apps/lightning-ops
npm run smoke:ep212-full-flow -- --json --mode mock
```

For production dry runs:

```bash
cd apps/lightning-ops
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="https://<wallet-executor-host>" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="<optional-bearer>" \
OA_LIGHTNING_OPS_EP212_SATS4AI_URL="https://sats4ai.com/api/l402/text-generation" \
OA_LIGHTNING_OPS_EP212_ROUTE_A_URL="https://l402.openagents.com/ep212/premium-signal" \
OA_LIGHTNING_OPS_EP212_ROUTE_B_URL="https://l402.openagents.com/ep212/expensive-signal" \
npm run smoke:ep212-full-flow -- --json --mode live
```

Detailed checklist: `docs/lightning/runbooks/EP212_L402_BUYER_REHEARSAL_RUNBOOK.md`.

---

## 6. How to Edit in the Future

### 6.1 Changing Aperture config (authenticator, services, postgres host)

1. **Edit the template** in the repo:
   - **Postgres (production):** `docs/lightning/scripts/aperture-gcp-config-postgres.yaml`
     Do **not** commit a real password; keep `password: "REPLACE_PASSWORD"` in the template.
2. **Build the config that will go to Secret Manager:**
   - Get the DB password:
     `APERTURE_DB_PASS=$(gcloud secrets versions access latest --secret=l402-aperture-db-password --project=openagentsgemini)`
   - Produce a single YAML file with the password injected (exactly one `password:` key under `postgres`):

     ```bash
     python3 - <<'PY' > /tmp/config.yaml
     import os, sys
     tpl_path = "docs/lightning/scripts/aperture-gcp-config-postgres.yaml"
     pw = os.environ.get("APERTURE_DB_PASS", "").strip()
     if not pw:
       raise SystemExit("missing APERTURE_DB_PASS")
     tpl = open(tpl_path, "r", encoding="utf-8").read()
     # Replace only the placeholder value; do not commit the rendered file.
     sys.stdout.write(tpl.replace("REPLACE_PASSWORD", pw))
     PY
     ```
   - Add a new secret version:
     `gcloud secrets versions add l402-aperture-config --data-file=/tmp/config.yaml --project=openagentsgemini`
   - Delete the temp file: `rm /tmp/config.yaml`
3. **Redeploy Cloud Run** so the new revision picks up the latest config (Cloud Run uses `:latest` for the secret):
   `gcloud run deploy l402-aperture ...` (see §7 for full command).

### 6.2 Adding or changing routes (services)

- **Option A:** Edit the same Postgres config template (`aperture-gcp-config-postgres.yaml`), add or change entries under `services:`, then follow §6.1 to add a new config secret version and redeploy.
- **Option B:** Use the output of `apps/lightning-ops` compiler (routes only) and merge it into the config (e.g. in a CI or deploy script) so the deployed config = base (authenticator + postgres) + compiled routes.

### 6.3 Changing LND credentials (TLS or macaroon)

- Replace the secret contents in Secret Manager (new version for `l402-gcp-lnd-tls-cert` or `l402-gcp-lnd-invoice-macaroon`).
- Redeploy Cloud Run (or trigger a new revision) so the new secret version is used.

### 6.4 Rebuilding the Aperture image

- **Local:** From repo root,
  `cd docs/lightning/deploy && docker buildx build --platform linux/amd64 -f Dockerfile.aperture -t us-central1-docker.pkg.dev/openagentsgemini/l402/aperture:latest --push .`
- **Cloud Build (if permitted):**
  `gcloud builds submit --config docs/lightning/deploy/cloudbuild-aperture.yaml --substitutions=_TAG=$(git rev-parse --short HEAD) docs/lightning/deploy`
- Then redeploy Cloud Run with the same image tag (or `:latest`).

### 6.5 Cloud SQL: production hardening

- **Current setup:** Aperture connects to Cloud SQL via the instance **public IP**; authorized network `0.0.0.0/0` is set for ease of testing. This is **not** recommended for production.
- **Better options:**
  - **Private IP + VPC:** Put Cloud SQL on a private IP and connect Cloud Run via Serverless VPC Access (or Direct VPC egress). Then set `host` in the config to the private IP and restrict authorized networks.
  - **Unix socket (Cloud Run + Cloud SQL):** Use `--add-cloudsql-instances=openagentsgemini:us-central1:l402-aperture-db` and set `host` to `/cloudsql/openagentsgemini:us-central1:l402-aperture-db`. Aperture’s current DSN builder uses a URL format that breaks on the colons in that path; a code change (e.g. key=value DSN or URL-encoding the host) would be needed to use the socket and then you can remove the open authorized network.

---

## 7. Operational Commands (Copy-Paste Reference)

**Deploy Cloud Run (current production; no Cloud SQL socket):**

```bash
gcloud run deploy l402-aperture \
  --image=us-central1-docker.pkg.dev/openagentsgemini/l402/aperture:latest \
  --region=us-central1 \
  --vpc-connector=oa-serverless-us-central1 \
  --vpc-egress=private-ranges-only \
  --set-secrets=/lnd-tls/tls.cert=l402-gcp-lnd-tls-cert:latest,/lnd-mac/invoice.macaroon=l402-gcp-lnd-invoice-macaroon:latest,/aperture-cfg/config.yaml=l402-aperture-config:latest \
  --command=/aperture \
  --args=--configfile=/aperture-cfg/config.yaml \
  --allow-unauthenticated \
  --memory=1Gi --cpu=1 --port=8080 \
  --min-instances=0 --max-instances=2
```

**View logs (last 50):**

```bash
gcloud run services logs read l402-aperture --region=us-central1 --limit=50
```

**View logs for a specific revision (e.g. after a failed deploy):**

```bash
gcloud logging read 'resource.labels.revision_name="l402-aperture-REVISION_NAME"' \
  --project=openagentsgemini --limit=20 --format="value(timestamp,textPayload)"
```

---

## 8. Troubleshooting

| Symptom | What to check |
|--------|----------------|
| Container fails to start (no listen on 8080) | Logs: config parse error (e.g. duplicate `password` key), Postgres connection failure (wrong host/port/password or SSL), or LND connection failure. Use the logging command above with the failing revision name. |
| “invalid authenticator configuration” | Config file was not loaded or `lndhost`/`tlspath`/`macdir` are missing. Ensure deploy uses `--args=--configfile=/aperture-cfg/config.yaml` and the mounted config contains the full `authenticator` block. |
| “unable to open database file” (SQLite) | We use Postgres in production; if you switched to SQLite for local test, use a writable path. On Cloud Run, SQLite cannot open DB files (CANTOPEN). |
| Postgres “dial unix /tmp/...” | Aperture builds a URL DSN; the Cloud SQL socket path contains colons and breaks the URL. Use the instance public IP in config (and authorized network) until the DSN is fixed for socket. |
| 402 / proxy not working for a path | Ensure that path matches a `services` entry (hostregexp, pathregexp) in the deployed config and that the route has the correct upstream and price. |

---

## 9. Security and Public Repo

- **Never commit:** Voltage API key, TLS cert contents, macaroon contents, Cloud SQL password, or any secret value. The repo only references secret **names** and **procedures** (e.g. “inject password when adding config secret version”).
- **Local only (gitignored):** `output/voltage-node/` (TLS cert, node.json, macaroon), `.env.local` (e.g. `VOLTAGE_API_KEY`). Do not remove these from `.gitignore`.
- **Rotation:** If any secret might have been exposed, rotate it in the source (Voltage dashboard, Cloud SQL user, Secret Manager) and update any config or deploy that uses it.

---

## 10. Related Docs

- **Voltage → L402:** `docs/lightning/reference/VOLTAGE_TO_L402_CONNECT.md`
- **Deploy (image + config):** `docs/lightning/deploy/README.md`
- **Staging reconcile:** `docs/lightning/runbooks/STAGING_GATEWAY_RECONCILE_RUNBOOK.md`
- **L402 plan:** `docs/lightning/plans/L402_AGENT_PAYWALL_INFRA_PLAN.md`
- **lightning-ops compiler:** `apps/lightning-ops/README.md`

---

## 11. What you need to do (operator)

**Single reference for all operator steps:** `docs/lightning/status/20260212-0753-status.md` **§12) Operator checklist: what you need to do now.**

Summary: run staging reconcile with only `OA_LIGHTNING_OPS_CONVEX_URL` and `OA_LIGHTNING_OPS_SECRET`; gateway URLs default to `https://l402.openagents.com` and `/staging`. For CI, product/EP212 wiring, and changing Aperture routes, see the status doc §12.
