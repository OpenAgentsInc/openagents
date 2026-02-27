# Architecture And Self-Hosting

Money Dev Kit uses a hybrid architecture:

- Hosted API for coordination, checkout state, and infrastructure services.
- Self-hosted Lightning node behavior in your app/runtime.

## Trust And Custody Boundaries

- Lightning funds are self-custodial when using local node/wallet paths.
- `MDK_MNEMONIC` is wallet-critical secret material.
- `MDK_ACCESS_TOKEN` authorizes hosted API actions and must be treated as a secret.

## Account Requirements

- `agent-wallet` path: no MDK API account required.
- Hosted checkout/product path: requires MDK account and API credentials.

## Self-Host Knobs

Environment overrides can move from default hosted endpoints to custom infrastructure:

```env
MDK_API_BASE_URL=...
MDK_VSS_URL=...
MDK_ESPLORA_URL=...
MDK_NETWORK=mainnet|signet
```

## Operational Guidance

- Keep mnemonic and API secrets out of logs and git-tracked files.
- Validate env preconditions before running payment operations.
- Use signet for initial automation tests, then switch to mainnet deliberately.
- If self-hosting VSS or related components, keep TLS/auth/rate limiting explicit.
