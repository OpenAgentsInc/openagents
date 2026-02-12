# L402 Aperture Deploy – Runbook and Reference

**This document is the single source of truth for the OpenAgents L402 paywall gateway: how it works, what was built, how to use it, and how to change it.** The codebase is public; no secrets or sensitive values are stored in the repo.

---

## 1. What This System Is

- **L402** is the protocol for “pay with Lightning” over HTTP: the server returns `402 Payment Required` with an invoice and macaroon; the client pays the invoice, gets a preimage, and retries with `Authorization: L402 macaroon=… preimage=…` to access the resource.
- **Aperture** (Lightning Labs) is the L402 reverse proxy: it sits in front of your backend, talks to an LND node to create invoices and verify payments, and proxies authenticated requests to upstream services.
- **This deploy** runs Aperture on **Google Cloud Run**, backed by:
  - **Voltage** (hosted LND) for the LND connection (invoices, auth).
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
    [Voltage LND (gRPC)]          [Cloud SQL Postgres]
    openagents.m.voltageapp.io    l402-aperture-db
    :10009                        (tokens, migrations)
              |                           ^
              | TLS + macaroon            | DSN in config
              | (from Secret Manager)     |
              +---------------------------+
```

- **Cloud Run** runs the Aperture binary with:
  - **Config** from Secret Manager (`l402-aperture-config`) mounted at `/voltage-cfg/config.yaml`.
  - **Voltage TLS cert** and **invoice macaroon** from Secret Manager mounted at `/voltage-tls/tls.cert` and `/voltage-mac/invoice.macaroon`.
- **Aperture** connects to Voltage over the public internet (gRPC + TLS) and to Cloud SQL Postgres (currently via instance **public IP**; see §6 for production hardening).
- **Routes** (host/path → upstream, price) are defined in the same config; today there is a bootstrap placeholder service; real routes come from `apps/lightning-ops` compile and can be merged in.

---

## 3. What Was Built (GCP and Repo)

### 3.1 GCP project: `openagentsgemini`

| Resource type | Name / location | Purpose |
|---------------|-----------------|---------|
| **Secret Manager** | `l402-voltage-tls-cert` | Voltage node TLS cert (PEM); mounted as file in Cloud Run. |
| **Secret Manager** | `l402-voltage-invoice-macaroon` | Invoice macaroon from Voltage; mounted as file. |
| **Secret Manager** | `l402-aperture-config` | Full Aperture YAML (authenticator, postgres, services). **Contains no secrets in the repo**; the *deployed* version has the DB password injected when adding a new secret version (see §5). |
| **Secret Manager** | `l402-aperture-db-password` | Cloud SQL Postgres password for user `aperture`. Used only when building a new config secret version. |
| **Cloud SQL** | Instance `l402-aperture-db` (Postgres 15, us-central1) | Aperture’s database (tokens, migrations). Database name `aperture`, user `aperture`. |
| **Artifact Registry** | Repo `l402` in `us-central1` | Holds the Aperture image `aperture:latest` (built from Lightning Labs source, Go 1.24). |
| **Cloud Run** | Service `l402-aperture`, region `us-central1` | Runs Aperture; receives traffic and mounts the secrets above. |

**Live URL:** `https://l402-aperture-157437760789.us-central1.run.app`

### 3.2 Repo artifacts (all public-safe)

| Path | Purpose |
|------|---------|
| `docs/lightning/scripts/aperture-voltage-config.yaml` | **SQLite** base config (authenticator + bootstrap service). Used for local/testing; Cloud Run uses Postgres. |
| `docs/lightning/scripts/aperture-voltage-config-postgres.yaml` | **Postgres** config *template*: authenticator, postgres block (host, port, user, `password: REPLACE_PASSWORD`, dbname, etc.), bootstrap service. **You must inject the real DB password when creating a new Secret Manager version** (see §5). |
| `docs/lightning/scripts/voltage-api-fetch.sh` | Fetches node list, node details, and TLS cert from Voltage API; writes to `output/voltage-node/` (gitignored). Requires `VOLTAGE_API_KEY` (env or repo root `.env.local`). |
| `docs/lightning/deploy/Dockerfile.aperture` | Multi-stage build: Aperture from Lightning Labs source (Go 1.24), minimal runtime image. |
| `docs/lightning/deploy/cloudbuild-aperture.yaml` | Cloud Build config to build and push the Aperture image to Artifact Registry (optional; can build locally with Docker). |
| `docs/lightning/VOLTAGE_TO_L402_CONNECT.md` | How Voltage fits into L402, what you need from Voltage, and how to connect it to Aperture. |
| `docs/lightning/STAGING_GATEWAY_RECONCILE_RUNBOOK.md` | Staging reconcile and env vars for lightning-ops. |
| `apps/lightning-ops/` | Compiler that produces route config from Convex paywall state; output can be merged into Aperture config. |

**Gitignore:** `output/`, `output/voltage-node/`, and repo root `.env.local` are ignored so that TLS certs, macaroons, and API keys are never committed.

---

## 4. Where Secrets Live (No Values in Repo)

All sensitive values live **only** in GCP Secret Manager (and, for local use, in `.env.local` or env vars). The repo contains **names and procedures only**.

| Secret name | What it holds | Who uses it |
|-------------|----------------|-------------|
| `l402-voltage-tls-cert` | Voltage node TLS certificate (PEM file contents). | Cloud Run mounts as `/voltage-tls/tls.cert`; Aperture config references this path. |
| `l402-voltage-invoice-macaroon` | Invoice macaroon file contents from Voltage. | Cloud Run mounts as `/voltage-mac/invoice.macaroon`; Aperture `macdir` points to `/voltage-mac`. |
| `l402-aperture-config` | Full Aperture YAML used at runtime. **The version deployed** includes the Postgres password (injected when adding a new version; see §5). The *template* in the repo uses `REPLACE_PASSWORD`. | Cloud Run mounts as `/voltage-cfg/config.yaml`; Aperture is started with `--configfile=/voltage-cfg/config.yaml`. |
| `l402-aperture-db-password` | Cloud SQL Postgres password for user `aperture`. | Used only when creating a new version of `l402-aperture-config` (script substitutes it into the YAML). Never mounted into the container directly. |

**Rotation:**

- **Voltage TLS / macaroon:** Replace the secret version in Secret Manager (e.g. `gcloud secrets versions add l402-voltage-tls-cert --data-file=./new-tls.cert`), then deploy a new Cloud Run revision (or rely on `:latest` and redeploy).
- **DB password:** Change the password in Cloud SQL for user `aperture`, then create a new secret version of `l402-aperture-db-password` with that value, then build a new `l402-aperture-config` version with the new password and redeploy.
- **Voltage API key:** Used only by `voltage-api-fetch.sh` (and optionally in `.env.local`). Rotate in the Voltage dashboard; never commit.

---

## 5. How to Use It

### 5.1 Public URL and health

- **Service URL:** `https://l402-aperture-157437760789.us-central1.run.app`
- A request to `/` without L402 auth typically returns **400** or similar (no matching service or auth required); that indicates Aperture is up.
- To actually get a 402 and then access a paywalled route, you must request a **host/path that matches a configured service** in Aperture. The current config has only a bootstrap service (host `l402-bootstrap.openagents.local`); add or merge routes from `apps/lightning-ops` for real paywalled paths.

### 5.2 L402 flow (conceptual)

1. Client requests a paywalled URL.
2. Aperture responds with `402 Payment Required` and `WWW-Authenticate: L402 macaroon="…" invoice="…"`.
3. Client pays the invoice (e.g. via Lightning wallet), gets the preimage.
4. Client retries with `Authorization: L402 macaroon="…" preimage="…"`.
5. Aperture verifies and proxies to the upstream.

### 5.3 Staging reconcile (lightning-ops)

To verify 402 issuance and proxy against this gateway:

- Set env vars (see `docs/lightning/STAGING_GATEWAY_RECONCILE_RUNBOOK.md`):
  `OA_LIGHTNING_OPS_CHALLENGE_URL`, `OA_LIGHTNING_OPS_PROXY_URL` (and optionally `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`) to the Cloud Run URL and the paywalled path you configured.
- Run: `./scripts/staging-reconcile.sh` (or the smoke command from `apps/lightning-ops`).

---

## 6. How to Edit in the Future

### 6.1 Changing Aperture config (authenticator, services, postgres host)

1. **Edit the template** in the repo:
   - **Postgres (production):** `docs/lightning/scripts/aperture-voltage-config-postgres.yaml`
     Do **not** commit a real password; keep `password: "REPLACE_PASSWORD"` in the template.
2. **Build the config that will go to Secret Manager:**
   - Get the DB password:
     `APERTURE_DB_PASS=$(gcloud secrets versions access latest --secret=l402-aperture-db-password --project=openagentsgemini)`
   - Produce a single YAML file with the password injected (exactly one `password:` key under `postgres`). Example (lines 1–20 of template, then your password line, then lines 22–end of template):
     `{ sed -n '1,20p' docs/lightning/scripts/aperture-voltage-config-postgres.yaml; printf '  password: "%s"\n' "$APERTURE_DB_PASS"; sed -n '22,$p' docs/lightning/scripts/aperture-voltage-config-postgres.yaml } > /tmp/config.yaml`
   - Add a new secret version:
     `gcloud secrets versions add l402-aperture-config --data-file=/tmp/config.yaml --project=openagentsgemini`
   - Delete the temp file: `rm /tmp/config.yaml`
3. **Redeploy Cloud Run** so the new revision picks up the latest config (Cloud Run uses `:latest` for the secret):
   `gcloud run deploy l402-aperture ...` (see §7 for full command).

### 6.2 Adding or changing routes (services)

- **Option A:** Edit the same Postgres config template (`aperture-voltage-config-postgres.yaml`), add or change entries under `services:`, then follow §6.1 to add a new config secret version and redeploy.
- **Option B:** Use the output of `apps/lightning-ops` compiler (routes only) and merge it into the config (e.g. in a CI or deploy script) so the deployed config = base (authenticator + postgres) + compiled routes.

### 6.3 Changing Voltage credentials (TLS or macaroon)

- Replace the secret contents in Secret Manager (new version for `l402-voltage-tls-cert` or `l402-voltage-invoice-macaroon`).
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
  --set-secrets=/voltage-tls/tls.cert=l402-voltage-tls-cert:latest,/voltage-mac/invoice.macaroon=l402-voltage-invoice-macaroon:latest,/voltage-cfg/config.yaml=l402-aperture-config:latest \
  --command=/aperture \
  --args=--configfile=/voltage-cfg/config.yaml \
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
| “invalid authenticator configuration” | Config file was not loaded or `lndhost`/`tlspath`/`macdir` are missing. Ensure deploy uses `--args=--configfile=/voltage-cfg/config.yaml` and the mounted config contains the full `authenticator` block. |
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

- **Voltage → L402:** `docs/lightning/VOLTAGE_TO_L402_CONNECT.md`
- **Deploy (image + config):** `docs/lightning/deploy/README.md`
- **Staging reconcile:** `docs/lightning/STAGING_GATEWAY_RECONCILE_RUNBOOK.md`
- **L402 plan:** `docs/lightning/L402_AGENT_PAYWALL_INFRA_PLAN.md`
- **lightning-ops compiler:** `apps/lightning-ops/README.md`
