# Lightning Agent Tools Playbook

Use this reference for practical L402 buyer and seller execution.

## Source Of Truth

- `~/code/lightning-agent-tools/README.md`
- `~/code/lightning-agent-tools/docs/l402-and-lnget.md`
- `~/code/lightning-agent-tools/docs/commerce.md`
- `~/code/lightning-agent-tools/docs/quickref.md`

## Scope

Lightning Agent Tools provides a seven-skill stack:
- `lnd`
- `lightning-security-module`
- `macaroon-bakery`
- `lnget`
- `aperture`
- `lightning-mcp-server`
- `commerce`

Treat this as an L402 commerce toolkit where agents can buy (`lnget`) and sell (`aperture`) resources over Lightning.

## Buyer Path (Pay For L402 APIs)

1. Install and start node runtime:

```bash
~/code/lightning-agent-tools/skills/lnd/scripts/install.sh
~/code/lightning-agent-tools/skills/lnd/scripts/create-wallet.sh --mode standalone
~/code/lightning-agent-tools/skills/lnd/scripts/start-lnd.sh
```

2. Install and configure `lnget`:

```bash
~/code/lightning-agent-tools/skills/lnget/scripts/install.sh
lnget config init
lnget ln status
```

3. Fetch with hard spend caps:

```bash
lnget --no-pay --json https://api.example.com/data | jq '.invoice_amount_sat'
lnget --max-cost 500 https://api.example.com/data
```

4. Inspect token cache after purchases:

```bash
lnget tokens list
lnget tokens show api.example.com
```

## Seller Path (Host Paid Endpoint)

1. Start a backend service (example):

```bash
mkdir -p /tmp/l402-data
echo '{"ok":true}' > /tmp/l402-data/data.json
cd /tmp/l402-data && python3 -m http.server 8080
```

2. Install and configure aperture:

```bash
~/code/lightning-agent-tools/skills/aperture/scripts/install.sh
~/code/lightning-agent-tools/skills/aperture/scripts/setup.sh --insecure --port 8081
~/code/lightning-agent-tools/skills/aperture/scripts/start.sh
```

3. Confirm 402 challenge:

```bash
lnget -k --no-pay https://localhost:8081/api/data.json
```

## End-To-End Commerce Loop

The buyer flow should verify all three states:
1. 402 challenge returned on unpaid request.
2. Invoice payment succeeds under `--max-cost`.
3. Retried request returns data with paid authorization.

Suggested operational checks:

```bash
# Buyer-side spending visibility
lnget tokens list --json | jq '[.[] | .amount_paid_sat] | add'

# Seller-side invoice settlement visibility
~/code/lightning-agent-tools/skills/lnd/scripts/lncli.sh listinvoices
```

## Operational Guardrails

- Always set `lnget --max-cost` in autonomous runs.
- Use preview mode (`--no-pay`) in planning and dry-runs.
- Keep test and prod configs separate.
- Prefer watch-only + remote signer for production funds.
