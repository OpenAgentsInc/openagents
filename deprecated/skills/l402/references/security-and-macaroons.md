# Security And Macaroons

Use this reference to enforce least-privilege and key isolation in Lightning agent workflows.

## Source Of Truth

- `~/code/lightning-agent-tools/docs/security.md`
- `~/code/lightning-agent-tools/docs/two-agent-setup.md`
- `~/code/lightning-agent-tools/skills/lightning-security-module/SKILL.md`
- `~/code/lightning-agent-tools/skills/macaroon-bakery/SKILL.md`

## Recommended Security Tier

Preferred production architecture is watch-only node plus remote signer:
- Agent machine runs watch-only `lnd` without private keys.
- Signer machine holds all key material and signs over authenticated gRPC.

This prevents key extraction from the agent runtime host.

## Macaroon Role Model

Use least-privilege roles from macaroon bakery:
- `pay-only`: buyer agent paying L402 invoices.
- `invoice-only`: seller agent creating and tracking invoices.
- `read-only`: observability and monitoring.
- `channel-admin`: node operations.
- `signer-only`: remote signer path only.

Never use `admin.macaroon` in autonomous production loops.

## Core Commands

```bash
# Buyer role
~/code/lightning-agent-tools/skills/macaroon-bakery/scripts/bake.sh --role pay-only

# Seller role
~/code/lightning-agent-tools/skills/macaroon-bakery/scripts/bake.sh --role invoice-only

# Remote signer role (container signer)
~/code/lightning-agent-tools/skills/macaroon-bakery/scripts/bake.sh --role signer-only --container litd-signer

# Inspect macaroon permissions
~/code/lightning-agent-tools/skills/macaroon-bakery/scripts/bake.sh --inspect ~/.lnd/data/chain/bitcoin/mainnet/admin.macaroon
```

## Remote Signer Setup Pattern

Signer host:
1. Install signer component.
2. Create signer wallet.
3. Export credentials bundle.

Agent host:
1. Import signer credentials bundle.
2. Create watch-only wallet.
3. Start watch-only `lnd` wired to signer host.

For native mode, provide `--signer-host <ip>:10012` when creating and starting watch-only wallet flows.

## Production Checklist

1. Remote signer enabled for production funds.
2. Role-scoped macaroons in every agent runtime.
3. Signer-only macaroon used on signer path.
4. Signer port exposure restricted to authorized hosts.
5. Macaroon and seed file permissions locked down.
6. Spending caps enforced at client request layer (`--max-cost`).
