# lightning-wallet-executor

Effect-first HTTP service for agent-owned Spark wallet execution.

## Endpoints

- `GET /healthz`
- `GET /status`
- `POST /pay-bolt11`

Request payload for `POST /pay-bolt11`:

```json
{
  "requestId": "optional-id",
  "payment": {
    "invoice": "lnbc...",
    "maxAmountMsats": 100000,
    "host": "sats4ai.com"
  }
}
```

## Runtime Modes

- `mock` (default): deterministic local/testing behavior
- `spark`: real Breez Spark SDK runtime

## Required env for `spark` mode

- `OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark`
- `OA_LIGHTNING_SPARK_API_KEY=<key>`
- `OA_LIGHTNING_WALLET_ALLOWED_HOSTS=sats4ai.com,l402.openagents.com`
- optional hardening: `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=<bearer-token>`
- mnemonic source:
  - env: `OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=env` and `OA_LIGHTNING_WALLET_MNEMONIC=<seed phrase>`
  - gcp: `OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=gcp` and `OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION=projects/.../secrets/.../versions/latest`

## Commands

```bash
npm run typecheck
npm test
npm run smoke
npm run dev
```
