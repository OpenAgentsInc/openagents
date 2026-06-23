# Khala Observability, SLA, And Operator Runbook

Date: 2026-06-23

Issue: OpenAgentsInc/openagents#6115

Purpose: give the Khala launch operator one public-safe control room for
deploying, rolling back, disabling spend paths, checking readiness, checking
receipts, and triaging incidents without printing secrets.

This runbook is operational guidance, not runtime authority. The deployed Worker
code, Cloudflare configuration, model catalog, receipt routes, and owner-armed
settlement gates remain the source of truth.

## Launch Posture

Khala can run in three postures:

| Posture | Gateway | Acceptance runner | Real settlement | Use |
| --- | --- | --- | --- | --- |
| Off | `INFERENCE_GATEWAY_ENABLED` not explicitly on | Off | Off | Incident stop, prelaunch, or disabled production |
| Dogfood | Gateway on with at least one servable model | `KHALA_ACCEPTANCE_DISPATCH_ENABLED` off unless runner is owned and ready | `OPENAGENTS_KHALA_LOOP_ARMED` off and `OPENAGENTS_REAL_SETTLEMENT_GATE` disabled | Narrow owner-gated testing |
| Broad launch | Gateway on and readiness green | Runner host deployed, callback token configured, dispatch intentionally on | Real settlement remains off unless the owner explicitly arms the payout experiment | Public paid endpoint |

Broad launch does not require live payout. A live payout experiment requires the
separate owner money gate described below.

## Secret Hygiene

- Source local secret files only in a shell where the values are not echoed.
- Do not paste bearer tokens, provider keys, Stripe values, Spark addresses,
  invoices, payment preimages, raw prompts, raw completions, or private checkout
  payloads into issues, docs, commits, chat, or terminal transcripts.
- Prefer smoke scripts that print check names and public receipt refs.
- When using `curl`, write bodies to `/tmp` and print only HTTP status plus a
  small public-safe `jq` projection.
- Redact `Authorization` headers before sharing any command.

Safe command shape:

```bash
BASE="${OPENAGENTS_BASE_URL:-https://openagents.com}"
curl -fsS -o /tmp/khala-readiness.json -w "readiness_http=%{http_code}\n" \
  "$BASE/v1/gateway/readiness"
jq '{status, totalModelCount, servableModelCount, hiddenModelCount, lanes, reasonRefs}' \
  /tmp/khala-readiness.json
```

Unsafe command shape:

```bash
echo "$OPENAGENTS_AGENT_TOKEN"
curl -v -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" ...
```

## Deploy

Use the shared deployment hub before any outward-facing deploy:

- `docs/DEPLOYMENT.md`
- `apps/openagents.com/docs/2026-06-15-openagents-web-deploy-runbook.md`
- `docs/launch/2026-06-23-khala-production-gateway-readiness-smoke.md`

Deploy only from a clean `origin/main` worktree. Build web assets before the
Worker deploy, and pass assets to Wrangler so the public UI is not stale.

Operator checklist:

```bash
cd /Users/christopherdavid/work/openagents
git status --short --branch
git fetch origin main

cd apps/openagents.com
bun run check:deploy
bun run build:web

cd workers/api
npx wrangler deploy --containers-rollout=none --assets ../../apps/web/dist
```

If the target is Worker-only and no web asset update is intended, follow the
surface runbook for the explicit worker-only deploy form. Do not infer a special
deploy command during an incident.

## Rollback

Rollback decision tree:

1. If the endpoint is serving errors, disable the gateway first.
2. If the gateway is healthy but verification dispatch is failing, disable
   acceptance dispatch and keep the gateway in dogfood or paid text-only mode.
3. If settlement or payout behavior is suspect, disable the Khala loop arm and
   the real-settlement gate before investigating receipts.
4. If the deployed build itself is wrong, roll back the Worker to the last known
   good deployment from Cloudflare, then run readiness-only smoke.

Rollback actions:

- Set `INFERENCE_GATEWAY_ENABLED` to a non-on value or remove it, then redeploy
  or update Worker configuration through the approved Cloudflare path.
- Set `KHALA_ACCEPTANCE_DISPATCH_ENABLED` to a non-on value or remove it.
- Set `OPENAGENTS_KHALA_LOOP_ARMED` to anything other than the exact token
  `armed`.
- Disable or remove `OPENAGENTS_REAL_SETTLEMENT_GATE`.
- Use the Cloudflare Workers deployment history to roll back code when config
  disablement is not enough.

After any rollback or disablement, run the readiness-only smoke and record:

- deployed Worker version or rollback target;
- gateway readiness status or disabled response;
- affected model id, if known;
- public receipt refs that demonstrate the incident, if any;
- whether real settlement was armed.

## Disable Gateway

The inference gateway is guarded by `INFERENCE_GATEWAY_ENABLED`; absent or
non-on values fail closed. When disabled, `/v1/gateway/readiness`, `/v1/models`,
`/v1/quote`, and `/v1/chat/completions` should reject as disabled rather than
advertising paid serving.

Verification:

```bash
BASE="${OPENAGENTS_BASE_URL:-https://openagents.com}"
curl -fsS -o /tmp/khala-readiness-disabled.json -w "readiness_http=%{http_code}\n" \
  "$BASE/v1/gateway/readiness" || true
```

Expected after disablement: HTTP `404` with a public-safe
`inference_gateway_disabled` body.

## Disable Acceptance Dispatch

The khala-code executed-verifier dispatch is guarded by
`KHALA_ACCEPTANCE_DISPATCH_ENABLED`. The callback route also requires
`ACCEPTANCE_VERDICT_CALLBACK_TOKEN`; without that token, verdict ingestion fails
closed.

Disable dispatch when:

- the runner host is down or producing inconsistent verdicts;
- queue lag is accumulating;
- callback authentication or receipt backfill looks suspicious;
- public receipts would otherwise imply executed verification that did not run.

Disablement keeps the delivered completion path honest: khala-code receipts stay
`unverified` until an authenticated runner verdict is backfilled.

## Disable Paid Settlement

Khala real payout requires multiple gates:

- `OPENAGENTS_KHALA_LOOP_ARMED` must be exactly `armed`;
- `OPENAGENTS_REAL_SETTLEMENT_GATE` must authorize the requested adapter, run,
  contributor, amount, per-payout cap, daily cap, and eligibility source;
- the settlement engine must see an executed accepted outcome;
- the contributor must have a registered payout destination.

Turn off spend or payout by disabling either owner gate:

- Set `OPENAGENTS_KHALA_LOOP_ARMED` to a value other than `armed`, or remove it.
- Disable or remove `OPENAGENTS_REAL_SETTLEMENT_GATE`.

Authority boundary:

- A Cloudflare operator with Worker environment permission can disable gateway,
  acceptance dispatch, and loop arming.
- Only the owner or owner-approved operator should arm or re-arm real settlement.
- Public UI, Pylon readiness, and product copy do not have payout authority.

Do not run live payout tests during an incident unless the owner explicitly
approves the exact run, amount cap, daily cap, contributor, payout target, and
evidence refs.

## Check Model Catalog

Readiness and catalog checks are public-safe and do not require a bearer token.

```bash
BASE="${OPENAGENTS_BASE_URL:-https://openagents.com}"
curl -fsS -o /tmp/khala-models.json -w "models_http=%{http_code}\n" \
  "$BASE/v1/models"
jq '{modelCount: (.data | length), models: [.data[].id]}' /tmp/khala-models.json
```

Launch expectation:

- HTTP `200`;
- the intended Khala model id is present;
- the model is also counted as servable by `/v1/gateway/readiness`.

If `/v1/models` lists a model that readiness does not mark servable, stop broad
launch and investigate the model-serving policy before accepting paid traffic.

## Check Provider Lane Health

Provider lane health for launch is the public-safe readiness route:

```bash
BASE="${OPENAGENTS_BASE_URL:-https://openagents.com}"
curl -fsS "$BASE/v1/gateway/readiness" \
  | jq '{status, servableModelCount, hiddenModelCount, lanes, reasonRefs}'
```

The route derives from the same public catalog and presence-only provider arming
that gate `/v1/models`, `/v1/quote`, and `/v1/chat/completions`.

Triage by status:

| Status | Meaning | Operator action |
| --- | --- | --- |
| `ready` | Every published catalog model is servable. | Continue launch smoke. |
| `degraded` | Some catalog models are hidden because a lane is unarmed. | Launch only if the intended model is servable; comment with hidden lanes. |
| `unavailable` | No published model is servable. | Do not launch paid traffic; check provider credentials and gateway flag. |

## Check Receipts

Use receipt refs or receipt URLs emitted by the smoke tooling. Receipts should be
dereferenceable without exposing tokens, prompts, private completions, provider
payloads, Spark addresses, invoices, or checkout secrets.

Safe receipt check:

```bash
RECEIPT_URL="<public receipt URL from smoke output>"
curl -fsS -o /tmp/khala-receipt.json -w "receipt_http=%{http_code}\n" "$RECEIPT_URL"
jq '{
  receiptRef,
  schemaVersion,
  requestedModel,
  servedModel,
  worker,
  verification,
  settled,
  realBitcoinMoved,
  settlementReceiptRef,
  detailRef
}' /tmp/khala-receipt.json
```

Broad launch expectation:

- completion receipt dereferences with HTTP `2xx`;
- `requestedModel` and `servedModel` are present;
- verification state is honest (`verified` only after executed acceptance);
- settlement fields are false or dry-run unless owner-armed real payout was
  explicitly approved.

## Smoke Commands

Readiness-only, no live spend:

```bash
cd /Users/christopherdavid/work/openagents/apps/openagents.com
bun run smoke:khala:gateway-readiness -- --readiness-only
```

Full gateway smoke, can meter credits:

```bash
cd /Users/christopherdavid/work/openagents/apps/openagents.com
# Source OPENAGENTS_AGENT_TOKEN without echoing it.
bun run smoke:khala:gateway-readiness -- --approve-live-spend
```

Billing/MPP proof has its own owner-gated smoke. Run it only when validating the
billing lane, not as the default incident smoke:

```bash
cd /Users/christopherdavid/work/openagents/apps/openagents.com
bun run smoke:khala:billing-mpp-proof
```

## SLA And Incident Classes

| Class | Example | Target first action | Required operator action |
| --- | --- | --- | --- |
| P0 spend or payout risk | unexpected real settlement, cap bypass suspicion, private receipt leakage | 15 minutes | Disable settlement gates, preserve public refs, notify owner |
| P0 gateway outage during broad launch | all Khala requests fail or readiness `unavailable` | 15 minutes | Disable gateway or roll back Worker, then readiness smoke |
| P1 degraded serving | intended model unavailable, one provider lane down, receipt route broken | 1 hour | Keep dogfood only, record readiness and receipt evidence |
| P1 verifier drift | khala-code says `verified` without executed verdict or runner backlog grows | 1 hour | Disable acceptance dispatch; receipts must remain honest |
| P2 docs/dashboard mismatch | public copy overstates model, billing, settlement, or benchmark state | 1 business day | Downgrade copy or link the missing evidence |

Incident notes should include only public-safe evidence:

- base URL;
- deployed Worker version or commit hash;
- readiness status and servable model count;
- model id;
- public receipt URL or ref;
- smoke check names;
- flags disabled or rolled back.

## Broad Launch Exit Criteria

Before leaving dogfood posture:

- `GET /v1/gateway/readiness` returns `ready` or a documented `degraded` state
  where the intended model is servable.
- `GET /v1/models` lists the intended Khala model.
- `smoke:khala:gateway-readiness -- --approve-live-spend` exits `0` against the
  intended production base URL and returns a dereferenceable receipt.
- If khala-code executed verification is advertised, the runner host is deployed,
  callback token is configured, dispatch is intentionally on, and receipt tests
  prove `verified` only follows executed acceptance.
- Real payout remains disabled unless the owner has approved the exact bounded
  run, caps, payout target, and evidence refs.
- Product promises and public copy point at current evidence, not aspirational
  future work.
