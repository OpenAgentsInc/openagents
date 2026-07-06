# MDK Treasury / Sidecar / Tips-Buffer → Cloud Run Cutover Runbook (CFG-15)

**Status:** EXECUTION ATTEMPTED 2026-07-06 after owner GO — **flip blocked**:
the deployed production Worker predates the endpoint seam and no code deploy
can ship (free-plan 3 MiB cap, error 10027). CF containers left RUNNING (live
payments intact); the live flip is now COUPLED to the CFG-9 monolith cutover.
See §8 for the execution-attempt record and §9 for the revised sequencing.
**Epic:** #8515 (Cloudflare → GCP migration). Cutover issue: CFG-15.
**Audit context:** `docs/cloud/2026-07-06-cloudflare-to-google-consolidation-audit.md`.

The Cloudflare Workers Paid plan was cancelled. The three MDK money-path
containers (`MdkSidecarContainer`, `MdkTreasuryContainer`,
`MdkTipsBufferContainer`) still run on Cloudflare Containers serving live
Spark/MDK payments and could be disabled by Cloudflare at any moment. This
runbook is the rehearsed, owner-gated procedure to move them to Google Cloud
Run (project `openagentsgemini`, region `us-central1`).

> **STANDING INVARIANT (fund-loss): never run two live daemons on one
> mnemonic.** Every step below is ordered so the old daemon is verifiably
> stopped before a daemon holding the same mnemonic starts anywhere else —
> including during redeploys and rollbacks. See
> `apps/openagents.com/INVARIANTS.md` → "MDK Money-Path Daemon Endpoint
> Override (CFG-15)".

---

## 1. Architecture map (what actually moves)

All three services are small Bun HTTP daemons under
`apps/openagents.com/services/` with identical Dockerfiles (`oven/bun:1.3.1`,
port 8080). Today the Worker reaches them via `getContainer(namespace,
INSTANCE).fetch(...)` (Durable Object → Cloudflare Container).

| Service | Wallet engine | Durable state location | CF instances |
|---|---|---|---|
| `mdk-treasury` | `@moneydevkit/lightning-js` `MdkNode` (mainnet) + `@breeztech/breez-sdk-spark` | MDK: **external VSS**, keyed by mnemonic + MDK access token. Spark: **Spark network**, keyed by mnemonic; local SQLite (`SPARK_TREASURY_STORAGE_DIR/storage.sql`) is a re-syncable cache | max 1 (single writer) |
| `mdk-tips-buffer` | `MdkNode` (own mnemonic) | MDK **external VSS** (same shape as treasury) | max 1 (single writer) |
| `mdk-sidecar` | `@moneydevkit/core/route` checkout handler | MDK hosted checkout state (external) | max 2 |

State that does NOT survive the move (accepted, by design):

- **In-daemon payment-outcome maps.** Both node daemons hold
  `paymentOutcomes` / `receivedPayments` in process memory; the code already
  documents that "callers must tolerate a pending answer after a container
  restart", and the Worker reconciles pending treasury transactions through
  the D1 ledger (`reconcilePendingTreasuryTransactions`).
- **The Durable Object outcome journal.** On Cloudflare, the
  `DurableMdkOutcomeContainer` wrapper journals terminal `/pay` outcomes into
  DO storage and serves them when the container restarts. The HTTP path has no
  DO, so this journal is bypassed. Mitigation: pending statuses resolve via
  the D1 ledger reconcile; treat any `pending` after cutover as reconcilable,
  not lost. **We could not inspect the live DO journal contents without
  touching prod** — assume some historical outcome rows exist only there; the
  D1 ledger remains the settlement source of truth.
- **The treasury container-generation key** (`ensureCurrentContainerGeneration`)
  is a CF-only restart mechanism; irrelevant on Cloud Run.

### Why Cloud Run and not GCE

- No persistent-disk requirement was found in the evidence: MDK wallet state
  lives in the external VSS (mnemonic + access token recreate the node
  anywhere) and Spark wallet state lives on the Spark network (the local
  SQLite file is a cache that rebuilds via `syncWallet`). Verified on staging:
  a fresh Cloud Run revision with an empty `/tmp` storage dir synced, minted
  invoices, and answered balance queries.
- Cloud Run matches the existing `apps/oa-updates` deploy pattern and the GCP
  credit posture; GCE would add unattended-VM patching burden for no
  durability gain.
- Residual honesty note: MDK's own guidance (see "MDK Agent-Wallet Send
  Readiness" invariant) is that mnemonic-only restore is NOT accepted as
  send-ready evidence. The cutover moves the ORIGINAL live daemon home, so
  post-cutover verification must re-prove send readiness (balance parity +
  a small real payout) before payouts are re-enabled.

### The Worker client seam (already landed, inert)

`workers/api/src/mdk-service-endpoints.ts` + three config vars:

- `MDK_TREASURY_SERVICE_URL` → `fetchMdkTreasuryPath` goes HTTPS with the
  existing `x-treasury-service-token` header.
- `MDK_TIPS_BUFFER_SERVICE_URL` → `fetchMdkTipsBufferPath` goes HTTPS with
  `x-tips-buffer-service-token`.
- `MDK_SIDECAR_SERVICE_URL` (+ `MDK_SIDECAR_SERVICE_TOKEN`) →
  `fetchMdkSidecarRequest` forwards the whole `/api/mdk` checkout request with
  `x-mdk-sidecar-service-token`; the daemon enforces that token whenever it is
  configured.

Unset vars = the existing Durable Object path; nothing changes until the flip.
The CFG-9 Cloud Run monolith consumes the same seam (it has no DO bindings, so
HTTP mode is its only route) — set the same three vars in its environment.

URLs must be HTTPS (loopback HTTP allowed for local smoke only); malformed or
non-HTTPS values are ignored and the DO path is used. Tests:
`workers/api/src/mdk-service-endpoints.test.ts`.

### Cloud Run quirk discovered in staging

The Google Frontend **reserves `/healthz` on `run.app` domains** and answers
404 before the container sees the request. All three daemons now serve
`/health` as an alias; use `/health` for Cloud Run checks. The CF container
`pingEndpoint` still uses `/healthz` over localhost (not intercepted).

---

## 2. Deploy tooling (already landed)

Per-service scripts, following the `apps/oa-updates` pattern:

- `apps/openagents.com/services/mdk-treasury/scripts/deploy-cloudrun.sh`
- `apps/openagents.com/services/mdk-tips-buffer/scripts/deploy-cloudrun.sh`
- `apps/openagents.com/services/mdk-sidecar/scripts/deploy-cloudrun.sh`

Behavior:

- Default to STAGING service names (`oa-mdk-*-staging`) and `staging-`
  Secret Manager prefixes. Deploying a non-staging name **refuses** unless
  `ALLOW_PRODUCTION_MONEY_PATH_DEPLOY=yes` (the owner gate).
- Mount only Secret Manager secrets that exist, so partial staging stacks
  deploy with honest `/health` flags.
- Treasury and tips-buffer pin `--max-instances 1` (single writer —
  load-bearing, never raise), `--no-cpu-throttling` (the node drains payment
  events between requests), gen2, 1 GiB.
- Sidecar mirrors CF's `max_instances: 2` and requires its service-token
  secret (a network-reachable `/api/mdk` without a token gate is a policy
  violation).

Secret Manager names (prefix `staging-` for staging, none for production):

| Secret | Env var | Service |
|---|---|---|
| `mdk-treasury-mnemonic` | `MDK_TREASURY_MNEMONIC` | treasury |
| `mdk-treasury-access-token` | `MDK_TREASURY_ACCESS_TOKEN` | treasury |
| `mdk-treasury-service-token` | `MDK_TREASURY_SERVICE_TOKEN` | treasury |
| `spark-treasury-api-key` | `SPARK_TREASURY_API_KEY` | treasury (optional; daemon has the owner-authorized default) |
| `mdk-tips-buffer-mnemonic` | `MDK_TIPS_BUFFER_MNEMONIC` | tips-buffer |
| `mdk-tips-buffer-access-token` | `MDK_TIPS_BUFFER_ACCESS_TOKEN` | tips-buffer |
| `mdk-tips-buffer-service-token` | `MDK_TIPS_BUFFER_SERVICE_TOKEN` | tips-buffer |
| `mdk-sidecar-service-token` | `MDK_SIDECAR_SERVICE_TOKEN` | sidecar |
| `mdk-sidecar-access-token` | `MDK_ACCESS_TOKEN` | sidecar |
| `mdk-sidecar-mnemonic` | `MDK_MNEMONIC` | sidecar |
| `mdk-sidecar-webhook-secret` | `MDK_WEBHOOK_SECRET` | sidecar |
| `mdk-sidecar-withdrawal-destination` | `WITHDRAWAL_DESTINATION` | sidecar |

Grant the Cloud Run runtime SA
(`157437760789-compute@developer.gserviceaccount.com`)
`roles/secretmanager.secretAccessor` on each secret before deploying.

---

## 3. Staging evidence (2026-07-06)

Deployed with a **fresh throwaway mnemonic** (generated with `@scure/bip39`,
never printed, never the production mnemonic) and fresh random service tokens:

- `oa-mdk-treasury-staging` — daemon boots; `/health` flags honest
  (`mnemonicConfigured: true`, `accessTokenConfigured: false`,
  `sparkMnemonicConfigured: true`); Spark SDK built against the live Spark
  network on the throwaway wallet:
  - `GET /spark/balance` → `{"balanceSat":0,"maxSendableSat":0,"rail":"spark"}`
  - `POST /spark/funding-invoice {amountSat:1000}` → real bolt11 +
    `paymentHash` (decoded from the invoice; matches)
  - `GET /spark/received/<hash>` → `{"received":false,"settled":false}`
  - `GET /spark/funding-destination` → spark address + registered
    `…@breez.tips` lightning address
  - MDK-node endpoints answer `503 treasury_unconfigured` (no staging MDK
    access token exists — MDK API keys are owner-issued; this is the honest
    staging boundary).
- `oa-mdk-tips-buffer-staging`, `oa-mdk-sidecar-staging` — image builds and
  daemon boot verified via `/health` (tips-buffer with its own throwaway
  mnemonic; sidecar with service token only).

**NOT verified in staging (and why):** an actual receive→settle needs real
sats on mainnet (throwaway wallet is unfunded; Breez regtest needs a regtest
Spark environment we do not run) and MDK-node send/receive needs an MDK access
token. Both are re-verified live in step 5 of the cutover.

---

## 4. Cutover preconditions (ALL must hold)

1. **Owner sign-off recorded on the CFG-15 issue.** This runbook is not
   authorization.
2. The Worker deployed at current `main` includes the endpoint seam (this
   change) and tests are green.
3. Production Secret Manager secrets exist (table above, no prefix) EXCEPT the
   mnemonics, which are loaded during the freeze window (step order below).
   Never load the production treasury mnemonic while the CF container class
   still exists in the deployed Worker.
4. Staging stack verified within the last 7 days (redo §3 if stale).
5. Owner has the local backup env available (`.secrets/openagents-mdk-treasury.env`
   in the workspace root) for the treasury mnemonic + access token + service
   token; equivalents for tips-buffer and sidecar from the production Worker
   secret store. Values move via `gcloud secrets versions add --data-file`,
   never through echo, logs, or tracked files.
6. A low-traffic window is chosen; MPP Lightning purchases and tips will 503
   during the gap (crypto/card rails are unaffected).

## 5. Cutover procedure (owner-gated; per service, treasury shown)

Do ONE service at a time, treasury LAST (it is the highest-value wallet;
rehearse the exact sequence on sidecar first, then tips-buffer).

**Phase A — stop the Cloudflare daemon FIRST.**

1. Announce the freeze; disable payout dispatch gates (Tassadar
   real-settlement gate stays OFF; X-claim dispatcher window closed).
2. Snapshot pre-cutover truth via the existing operator status API
   (`/api/operator/treasury/status`: MDK balance, Spark balance, pending
   transactions). Record in the CFG-15 issue (public-safe numbers only).
3. Deploy a Worker revision that **removes** the treasury entry from
   `containers` + its DO binding/migration in `wrangler.jsonc` and removes the
   `MDK_TREASURY_MNEMONIC` / `MDK_TREASURY_ACCESS_TOKEN` Worker secrets.
   Removing the container class destroys its instances; treasury routes now
   fail closed (503). **Downtime window opens.**
4. Verify no treasury container instance remains (Cloudflare dash → Workers →
   Containers; `wrangler containers list` if available). Wait an extra 10
   minutes as a belt-and-suspenders buffer past any in-flight request.

**Phase B — start the Cloud Run daemon on the production mnemonic.**

5. Load the production mnemonic + access token into Secret Manager
   (`mdk-treasury-mnemonic`, `mdk-treasury-access-token`) from the owner
   backup, and the existing service token into `mdk-treasury-service-token`.
6. Deploy:
   ```sh
   ALLOW_PRODUCTION_MONEY_PATH_DEPLOY=yes \
   SERVICE_NAME=oa-mdk-treasury SECRET_PREFIX= MIN_INSTANCES=1 \
   ./apps/openagents.com/services/mdk-treasury/scripts/deploy-cloudrun.sh
   ```
7. Verify directly against the service URL: `/health` flags all true;
   `/spark/balance` and `/balance` match the step-2 snapshot (balance parity
   is REQUIRED before re-pointing the Worker; investigate any delta before
   proceeding).

**Phase C — re-point the Worker.**

8. Set the Worker config `MDK_TREASURY_SERVICE_URL=<cloud run url>` (keep
   `MDK_TREASURY_SERVICE_TOKEN` as-is) and deploy. **Downtime window closes.**
9. Verification: operator treasury status API healthy; mint a small MPP
   Lightning funding invoice end-to-end; run the pending-transaction
   reconcile; then (owner) one small real payout to a known destination before
   re-enabling payout gates.

**Redeploys after cutover:** Cloud Run runs old and new revisions side by side
during traffic migration. For the treasury/tips services: deploy, flip traffic
100% to the new revision, then IMMEDIATELY `gcloud run revisions delete <old>`
so no stale instance with a live node can linger. The node starts lazily on
the first money-path request, so a not-yet-hit new revision holds no wallet.

**Tips-buffer:** same A→B→C with `MDK_TIPS_BUFFER_*` names,
`oa-mdk-tips-buffer`, and the tips instance/binding in `wrangler.jsonc`.

**Sidecar:** same order (`MDK_SIDECAR_SERVICE_URL` + `MDK_SIDECAR_SERVICE_TOKEN`
on the Worker; sidecar carries `MDK_ACCESS_TOKEN`/`MDK_MNEMONIC`/webhook
secret). Do it FIRST as the rehearsal: it is the checkout path (lowest custody
risk, hosted MDK state) and proves the whole seam live. Verify with a real
checkout session before moving on.

## 6. Rollback (per service)

Rollback re-arms the Cloudflare container — the same no-overlap rule applies
in reverse:

1. Unset the `MDK_*_SERVICE_URL` var and deploy the Worker (routes fail closed
   503 — do NOT restore the container config yet).
2. **Stop the Cloud Run daemon completely:** `gcloud run services delete
   oa-mdk-treasury --region us-central1` (deletion, not scale-to-zero; an
   idle instance can linger otherwise). Verify it is gone.
3. Restore the `containers` + DO entries in `wrangler.jsonc` and the Worker
   mnemonic/access-token secrets; deploy. The CF container re-materializes on
   first request against the same external VSS/Spark state.
4. Re-run the step-2 balance snapshot and parity check before re-enabling
   payout gates.

## 7. Invariant callouts (repeat, because they are the whole point)

- **Never two live daemons on one mnemonic** — not during cutover, not during
  redeploys, not during rollback. Stop-verify-wait, then start.
- **Never print or track secrets.** Mnemonics/tokens move only via
  `gcloud secrets versions add --data-file` from owner-held files.
- **`--max-instances 1`** for treasury and tips-buffer is load-bearing.
- **Balance parity before traffic.** Any unexplained delta halts the cutover.
- **Payout gates stay OFF** until post-cutover send readiness is re-proven
  (mnemonic-only restore is not send-ready evidence — MDK send-readiness
  invariant).
- **The flip vars are owner-gated.** `MDK_TREASURY_SERVICE_URL` /
  `MDK_TIPS_BUFFER_SERVICE_URL` / `MDK_SIDECAR_SERVICE_URL` in production
  change only inside this procedure.

---

## 8. Execution attempt record (2026-07-06, after owner GO)

Owner sign-off was recorded on the CFG-15 issue and the live cutover was
attempted the same day. Outcome: **the flip is impossible against the current
Cloudflare Worker deployment**, and the safe result is CF containers left
RUNNING with the live flip re-sequenced onto the CFG-9 monolith cutover.

### Why the flip is blocked (verified, not inferred)

1. **The deployed Worker does not contain the endpoint seam.** The last
   successful production code upload was `2026-07-06T12:26:53Z` (source:
   `wrangler deployments list` for `openagents-autopilot`). The seam commit
   `89898be350` was authored at `2026-07-06T19:18:34Z` — seven hours later.
   Confirmed directly: the deployed script bundle (fetched via the Workers
   API) contains **zero** references to `MDK_TREASURY_SERVICE_URL` /
   `MDK_TIPS_BUFFER_SERVICE_URL` / `MDK_SIDECAR_SERVICE_URL` or the
   `x-mdk-sidecar-service-token` header (it does contain the pre-existing
   `x-treasury-service-token` DO-path header). Deployed code that never reads
   the flip vars cannot be re-pointed by any config change.
2. **No code deploy can ship the seam.** The Workers Paid plan on the account
   is cancelled; deploys fail with API error `10027` ("Worker exceeded the
   size limit of 3 MiB") — the bundle is ~3.79 MiB gzipped. Verified on both
   staging and production by the CFG-1 lane (#8516) after the 12:26Z upload.

### What WAS proven (mechanics for the eventual flip)

- **Secret-only updates work on the free plan with no code upload.**
  `wrangler secret put` succeeded against both `openagents-staging` and
  `openagents-autopilot` (probe secret `CFG15_SECRET_INJECTION_PROBE`, set
  and then deleted; each appears as a `Source: Secret Change` deployment).
  Once a Worker build containing the seam is deployable again, the flip vars
  can be injected as secrets without a code upload.
- **The CF containers can be stopped WITHOUT a Worker deploy.**
  `wrangler containers list` shows the three live MDK container applications
  (`openagents-autopilot-mdktreasurycontainer` — 1 instance,
  `openagents-autopilot-mdktipsbuffercontainer` — 1 instance,
  `openagents-autopilot-mdksidecarcontainer` — 2 instances), and
  `wrangler containers delete <ID>` is available. This replaces Phase A
  step 3 (which assumed a `wrangler.jsonc` deploy) while the deploy freeze
  holds. **Caution: while the freeze holds, a containers delete is
  one-way** — re-creating a container class requires a Worker deploy, so
  rollback path §6 step 3 is unavailable. Delete the container apps only at
  the moment the replacement path is verified ready to take traffic.

### Decisions taken (single-writer invariant preserved)

- **CF containers left RUNNING.** Stopping them without a way to re-point the
  Worker would take live Spark/MDK payments down with no replacement path.
- **Production mnemonics NOT loaded into Secret Manager**, and **no
  production-named Cloud Run services created** (per precondition 3: never
  load the production treasury mnemonic while the CF container class exists
  in the deployed Worker). Standby remains the staging stack
  (`oa-mdk-*-staging`, throwaway mnemonics), re-verified healthy on
  2026-07-06 (`/health` OK on all three).
- **Baseline snapshot recorded** (public-safe): live CF treasury via
  `GET /api/operator/treasury/status` — MDK `balanceSat: 8622`,
  `maxSendableSat: 8536`, reward dispatch `enabled: false` (payout gates
  OFF), `pendingPaymentCount: 0`, all configured flags true. The deployed
  Worker exposes no operator route for the Spark-rail balance number; the
  Spark balance is re-snapshotted from the daemon directly during the actual
  flip window.
- **Considered and rejected:** a small route-scoped proxy Worker (under the
  free cap) intercepting money paths. It could only intercept the public
  `/api/mdk` checkout route; the treasury and tips-buffer calls are internal
  Worker→DO calls that never cross a routable URL, and it would add a new
  custody-adjacent surface without owner review.

## 9. Revised sequencing: the live flip is coupled to CFG-9/CFG-10

The CFG-9 Cloud Run monolith (#8524) consumes the same seam from plain env
vars — no Worker deploy needed. The MDK cutover therefore executes as part of
the monolith cutover, preserving stop-CF-first:

1. **Prepare (any time before the flip):** monolith staging acceptance green
   (#8524); staging MDK stack re-verified within 7 days (§3).
2. **Freeze + snapshot:** payout gates confirmed OFF; snapshot MDK + Spark
   balances and pending transactions via `/api/operator/treasury/status`
   (post public-safe numbers to the CFG-15 issue).
3. **Stop CF FIRST — via the containers API, not a deploy:**
   `wrangler containers delete` each of the three MDK container application
   IDs; verify `wrangler containers list` shows them gone (LIVE INSTANCES 0 /
   absent); wait 10 minutes. The still-deployed old Worker now fails closed
   (503) on money paths. **This is a point of limited return while the deploy
   freeze holds** (no Worker deploy = no container re-create), so do it only
   when step 1 is green and the owner has acknowledged the flip window.
4. **Start GCP on production material:** load production mnemonics/tokens
   into Secret Manager (`gcloud secrets versions add --data-file`, from
   `.secrets/openagents-mdk-treasury.env`, `.secrets/openagents-mdk-tips-buffer.env`,
   and the sidecar backup env); deploy `oa-mdk-sidecar`, `oa-mdk-tips-buffer`,
   `oa-mdk-treasury` with `ALLOW_PRODUCTION_MONEY_PATH_DEPLOY=yes`;
   **balance parity** against the step-2 snapshot before any traffic.
5. **Flip traffic:** the monolith deploys/starts with
   `MDK_TREASURY_SERVICE_URL` / `MDK_TIPS_BUFFER_SERVICE_URL` /
   `MDK_SIDECAR_SERVICE_URL` (+ `MDK_SIDECAR_SERVICE_TOKEN`,
   `MDK_TREASURY_SERVICE_TOKEN`, `MDK_TIPS_BUFFER_SERVICE_TOKEN`) set; CFG-10
   DNS moves `openagents.com` to the monolith. Residual DNS-propagation
   traffic hitting the old Worker 503s on money paths only (bounded,
   accepted); it can never double-drive a wallet because the CF daemons no
   longer exist.
6. **Verify:** §5 Phase C step 9 unchanged (status API healthy, small MPP
   funding invoice end-to-end, pending reconcile, one small real payout
   before payout gates re-arm).

**Rollback inside the window** (before DNS flip): delete the production Cloud
Run services (§6 step 2). Note the CF side cannot be re-armed while the deploy
freeze holds — rollback means money paths stay 503 until the monolith or a
Worker deploy is available. This asymmetry is why step 3 waits for monolith
staging acceptance.

**If Cloudflare disables the containers before CFG-9** (live risk — the paid
plan is cancelled): payments are down regardless of what we do, and the
single-writer constraint is then trivially satisfied on the CF side. Verify
the container apps are actually gone (`wrangler containers list`), then
execute steps 4-6 as soon as the monolith can take traffic. Do not start
production-mnemonic daemons early with no caller; that adds custody exposure
for zero traffic.

**If the owner re-enables Workers Paid instead** (fast unfreeze per #8516):
deploy current `main` (which contains the seam) via `deploy:safe`, then run
this runbook §5 exactly as originally written — the secret-injection path
proven above can carry the flip vars without further code deploys.
