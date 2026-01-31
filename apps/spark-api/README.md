# Spark API

Agent Payments backend: balance, invoice, pay. Deployed at **openagents.com/api/spark/\*** and called by the main API Worker when `SPARK_API_URL` points here.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/agents/:id/balance` | Balance for agent (stub: 0 until Breez SDK + KV adapter) |
| POST | `/payments/invoice` | Create invoice (body: `agent_id`, `amount_sats`, `description?`) |
| POST | `/payments/pay` | Pay invoice (body: `agent_id`, `invoice`) |

Responses are JSON. The main API wraps them in `{ ok, data, error }`.

## Local dev

```bash
cd apps/spark-api
npm install
npm run dev
```

Runs at `http://localhost:8788` (or next free port). In the API app set `SPARK_API_URL=http://localhost:8788` (e.g. in `apps/api/.dev.vars`) so balance/invoice/pay proxy here.

## Deploy

```bash
npm run deploy
```

Route: `openagents.com/api/spark/*`. Set the main API’s `SPARK_API_URL` to `https://openagents.com/api/spark` (e.g. in Dashboard or `apps/api/wrangler.toml` vars) so the full flow runs on Cloudflare.

## Full Spark integration

This Worker currently returns **stub** balance/invoice/pay so the request flow works end-to-end. Real balance and Lightning operations require:

- Breez SDK (e.g. `@breeztech/breez-sdk-spark`) with a **KV-backed storage adapter** (the SDK expects IndexedDB/filesystem; Workers have KV/D1).
- Per-agent wallet state stored in KV (or D1) and loaded per request.

See `apps/api/docs/agent-wallets.md` and `crates/spark/docs/CONFIGURATION.md` for storage and config.
