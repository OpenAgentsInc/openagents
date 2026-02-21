# lightning-wallet-executor

Rust HTTP service for agent-owned Spark wallet execution.

## Endpoints

- `GET /healthz`
- `GET /status`
- `POST /pay-bolt11`
- `POST /wallets/create`
- `POST /wallets/status`
- `POST /wallets/create-invoice`
- `POST /wallets/pay-bolt11`
- `POST /wallets/send-spark`

## Runtime Modes

- `mock` (default): deterministic local/testing behavior
- `spark`: real Breez Spark SDK runtime

## Required env for `spark` mode

- `OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark`
- `OA_LIGHTNING_SPARK_API_KEY=<key>`
- `OA_LIGHTNING_WALLET_ALLOWED_HOSTS=sats4ai.com,l402.openagents.com`
- `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=<bearer-token>`
- optional: `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN_VERSION=<positive-int>` (default `1`)
- mnemonic source:
  - env: `OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=env` and `OA_LIGHTNING_WALLET_MNEMONIC=<seed phrase>`
  - gcp: `OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=gcp` and `OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION=projects/.../secrets/.../versions/latest`

## Commands

```bash
cargo test --manifest-path apps/lightning-wallet-executor/Cargo.toml
cargo run --manifest-path apps/lightning-wallet-executor/Cargo.toml -- serve
cargo run --manifest-path apps/lightning-wallet-executor/Cargo.toml -- smoke
```

## Archived TypeScript Lane

Legacy TypeScript/Effect implementation is preserved in:

- `apps/lightning-wallet-executor/archived-ts/`

It is historical only and not used by active runtime paths.
