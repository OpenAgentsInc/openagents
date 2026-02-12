# Connecting Your Voltage Node to L402

You have a Voltage node set up; this doc summarizes how it plugs into the OpenAgents L402 (seller/paywall) stack and where to configure it.

## 1. Where Voltage Fits in the Architecture

From `docs/lightning/L402_AGENT_PAYWALL_INFRA_PLAN.md` and `docs/lightning/20260212-0753-status.md`:

- **Voltage = LND backend for the L402 gateway.** It is used for the **seller** path: issuing 402 challenges (invoices) and validating payments. It is **not** used for the desktop/buyer path (that uses Spark or local LND).
- **Aperture** (Lightning Labs’ Go proxy) is the component that talks to LND. It:
  - Connects to your LND node (e.g. Voltage) over **gRPC (port 10009)**.
  - Uses LND to create invoices and verify preimages for L402.
- **OpenAgents `apps/lightning-ops`** compiles **paywall routes** (host/path, upstream URL, pricing) into a deterministic `aperture.yaml`. It does **not** compile or inject the LND connection details; those are deployment/runtime configuration for Aperture.

So: **Voltage connects to L402 by being the LND instance that Aperture uses.** You configure Aperture’s “authenticator” to point at your Voltage node; OpenAgents’ compiler only feeds Aperture the route/paywall rules.

---

## 2. What You Need From Voltage

From the plan (§5.1–5.2) and Lightning Labs’ Aperture [sample-conf.yaml](https://github.com/lightninglabs/aperture/blob/master/sample-conf.yaml):

1. **Node metadata**
   - **LND gRPC host:port** (e.g. your Voltage node’s host and port **10009**).
   - **Network**: `mainnet` (or `testnet`/`signet` if you’re on testnet).

2. **Credentials (store in secret manager, never in repo)**
   - **TLS cert** – LND’s `tls.cert` (or Voltage’s equivalent).
   - **Macaroon** – For the gateway, use a **scoped invoice-only** macaroon, not admin. Plan: bootstrap with admin only long enough to bake scoped creds, then remove admin from runtime.

---

## 3. Aperture Configuration for Voltage (LND Connection)

Aperture’s config has an **`authenticator`** block that defines the LND connection. For a **direct LND** (Voltage) connection the relevant part looks like:

```yaml
authenticator:
  network: "mainnet"   # or testnet/signet
  disable: false
  # Direct LND (Voltage) connection:
  lndhost: "<voltage-grpc-host>:10009"
  tlspath: "/path/to/mounted/tls.cert"
  macdir: "/path/to/mounted/macaroons"
```

- **`lndhost`**: Your Voltage node’s gRPC address (e.g. `your-node.voltage.cloud:10009` or whatever Voltage gives you).
- **`tlspath`**: Path where the TLS cert is mounted in the Aperture container (e.g. from GCP Secret Manager → Cloud Run volume/env).
- **`macdir`**: Path to the directory containing the macaroon file (or the single macaroon file path, depending on Aperture version; check upstream Aperture docs for “macaroon” / “macdir”).

OpenAgents’ **compiler in `apps/lightning-ops`** does **not** emit this block. It only emits **routes** (version, host/path match, upstream, auth type, pricing). So you have two options:

1. **Base config + overlay**: Maintain a base Aperture config that includes the `authenticator` block (and any other global settings). At deploy time, merge or override with the compiled `aperture.yaml` from lightning-ops (routes only), or pass routes via whatever mechanism your deploy uses.
2. **Single full config**: Build a full Aperture config (authenticator + services/routes) in your deploy pipeline: take the compiled YAML from lightning-ops for the route list and inject it into a template that also has the Voltage `authenticator` section.

Either way: **Voltage connection = Aperture’s `authenticator` section; L402 routes = lightning-ops compiled output.**

---

## 4. Deployment (GCP + Voltage) – Summary

From `L402_AGENT_PAYWALL_INFRA_PLAN.md` and `STAGING_GATEWAY_RECONCILE_RUNBOOK.md`:

1. **Secrets**
   - Put TLS cert and (scoped) macaroon in **GCP Secret Manager**.
   - Inject into the Aperture runtime (e.g. Cloud Run) as files or env, and point `tlspath` / `macdir` (or equivalent) at those mount paths.

2. **Aperture**
   - Run Aperture (e.g. on **Cloud Run**) with:
     - Config that includes the Voltage `authenticator` block and the **routes** (from lightning-ops compile).
     - DB (e.g. Cloud SQL Postgres or SQLite) for token/invoice state.
   - Front it with an HTTPS load balancer; that’s your paywall URL.

3. **lightning-ops**
   - Set Convex + ops secret env vars (see `apps/lightning-ops/README.md`).
   - For staging reconcile, set:
     - `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`
     - `OA_LIGHTNING_OPS_CHALLENGE_URL`
     - `OA_LIGHTNING_OPS_PROXY_URL`
   - Run:
     - `./scripts/staging-reconcile.sh`
     or
     - `npm run smoke:staging -- --json --mode convex`
   - Reconcile verifies 402 issuance and authenticated proxy; it does not configure Voltage. Voltage is configured only in the Aperture deployment (authenticator + secrets).

4. **Settlement ingestion**
   - The plan includes a settlement ingest pipeline from LND (Voltage) to Convex. That’s separate from “connecting Voltage to L402”: first get Aperture talking to Voltage for challenges; then wire invoice/settlement events from Voltage into Convex per the Phase 3A/ingest design.

---

## 5. Quick Checklist: Voltage → L402

- [ ] Voltage node created; you have gRPC host and port (e.g. `:10009`).
- [ ] TLS cert and (scoped) invoice macaroon obtained; stored in Secret Manager (or equivalent).
- [ ] Base Aperture config (or deploy template) includes `authenticator` with `lndhost`, `tlspath`, `macdir` pointing at Voltage and the mounted secrets.
- [ ] `apps/lightning-ops` compiles routes from Convex paywall state; deploy merges or injects that into Aperture’s config.
- [ ] Aperture deployed (e.g. Cloud Run) with secrets mounted and config that has both Voltage auth and OpenAgents routes.
- [ ] Staging reconcile and smoke tests (`OA_LIGHTNING_OPS_CHALLENGE_URL`, `OA_LIGHTNING_OPS_PROXY_URL`) pass.

---

## 6. Example: OpenAgents Voltage Node (openagents.m.voltageapp.io)

You have:

| Item | Value | Use for L402? |
|------|--------|----------------|
| Node public key | `03874920...` | Lightning identity (not needed for Aperture config). |
| Clearnet connect | `...@54.214.32.132:20466` | P2P channel connections; not gRPC. |
| TOR connect | `...@5kpyv...onion:9735` | P2P over Tor; not gRPC. |
| Node ID | `93614aec-7944-4a86-9fe2-f115ae26d40b` | Reference; not needed in Aperture. |
| **API endpoint** | **`openagents.m.voltageapp.io`** | **Yes – this is the LND API host.** |
| REST port | 8080 | Optional (e.g. settlement reader); Aperture uses gRPC. |
| **GRPC port** | **10009** | **Yes – Aperture talks to LND over gRPC.** |
| LND version | 0.20.0-beta | Compatible with current Aperture. |
| Voltage version | v0.7.13 | Reference. |
| Cloud | AWS | Node location; Aperture can run on GCP. |

So for Aperture you set:

- **`lndhost`**: `openagents.m.voltageapp.io:10009`

What you still need from Voltage (not in the snippet above):

1. **TLS certificate** – Download from the Voltage dashboard (or API) for this node. Voltage typically exposes a `tls.cert` (or equivalent) so the client can authenticate the gRPC connection. Store it in Secret Manager and mount it where Aperture runs; set **`tlspath`** to that path.
2. **Macaroon** – From Voltage you get an admin (or default) macaroon. For production:
   - Use it only to bake a **scoped invoice-only** macaroon (see L402 plan §5.2).
   - Store the invoice macaroon in Secret Manager, mount it for Aperture, set **`macdir`** (or the single-file path your Aperture version expects).
3. **Network** – Confirm in Voltage whether this node is **mainnet** or testnet/signet; set Aperture’s **`authenticator.network`** to match.

Once you have TLS + macaroon from the Voltage UI/API, you have everything needed for the Aperture ↔ Voltage connection.

---

## 7. How to Run Aperture

Recommended (matches repo plan): run Aperture as a **single service on GCP Cloud Run**, talking to Voltage (on AWS) over the public internet. Voltage’s API endpoint is TLS-protected; no need for Aperture and Voltage to be in the same cloud.

1. **Build/pull Aperture image**
   - Use upstream [lightninglabs/aperture](https://github.com/lightninglabs/aperture) (pin a git SHA). Build with Docker/Cloud Build and push to Artifact Registry (or use an official image if Lightning Labs publish one).
2. **Config**
   - Base config with `authenticator` pointing at `openagents.m.voltageapp.io:10009`, `tlspath` and `macdir` set to paths inside the container.
   - Merge in or mount the **routes** YAML produced by `apps/lightning-ops` (compile from Convex paywall state). Our compiler emits a custom `version: 1` / `routes:` format; confirm Aperture’s current schema supports it or add a small transform in the deploy pipeline.
3. **Secrets**
   - GCP Secret Manager: store `tls.cert` and invoice macaroon. Use Cloud Run “secret as volume” (or env) so the container sees e.g. `/secrets/tls.cert` and `/secrets/invoice.macaroon`; set `tlspath` and `macdir` to those.
4. **Database**
   - Plan recommends Cloud SQL Postgres. Run migrations; point Aperture at the DB via env or config. Alternatively SQLite on a mounted volume for a minimal staging test (less ideal for production).
5. **Ingress**
   - Cloud Run with HTTPS (or put a load balancer in front). The resulting URL is your paywall domain; set `OA_LIGHTNING_OPS_CHALLENGE_URL` and `OA_LIGHTNING_OPS_PROXY_URL` to a paywalled path on that domain for staging reconcile.
6. **Reconcile**
   - From CI or a runner: set `OA_LIGHTNING_OPS_*` env vars, run `./scripts/staging-reconcile.sh` to verify 402 and proxy.

Alternative: run Aperture on **AWS** (e.g. ECS/Fargate or a single EC2) in the same region as Voltage to reduce latency; config and secrets pattern stay the same, use AWS Secrets Manager instead of GCP. The repo’s automation and runbooks are GCP-oriented, so Cloud Run is the path of least friction unless you already run everything on AWS.

---

## 8. References in This Repo

- **High-level plan (Voltage + GCP):** `docs/lightning/L402_AGENT_PAYWALL_INFRA_PLAN.md` (§4–5, §6.2, §15).
- **Staging reconcile:** `docs/lightning/STAGING_GATEWAY_RECONCILE_RUNBOOK.md`; `apps/lightning-ops/scripts/staging-reconcile.sh`.
- **What’s implemented:** `docs/lightning/20260212-0753-status.md`.
- **Compiler (routes only):** `apps/lightning-ops/src/compiler/apertureCompiler.ts` – emits `version: 1`, `routes:` with match/upstream/auth/pricing; no authenticator block.
- **Upstream Aperture sample config:** https://github.com/lightninglabs/aperture/blob/master/sample-conf.yaml (for full `authenticator` and options).
