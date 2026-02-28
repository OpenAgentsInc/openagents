---
name: l402
description: L402 agent commerce workflows with lnd, lnget, scoped macaroons, aperture, and MCP.
metadata:
  oa:
    project: l402
    identifier: l402
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
      - process:spawn
---

# L402

## Overview

Build and operate Lightning-native agent commerce flows using L402. Use this skill when tasks involve setting up Lightning payment infrastructure (`lnd`), enforcing key isolation with a remote signer, baking scoped macaroons, paying for L402-gated APIs with `lnget`, selling paid endpoints behind `aperture`, querying node state with Lightning MCP over LNC, or orchestrating end-to-end buyer and seller workflows.

This skill is based on `~/code/lightning-agent-tools` and should be treated as Bitcoin and Lightning only.

## Environment

- Requires `bash`, `curl`, and `jq`.
- Requires access to `~/code/lightning-agent-tools` (or `LIGHTNING_AGENT_TOOLS_DIR` override).
- Docker is the default runtime for node and signer flows.
- Go 1.24+ is needed for source builds; `npx` can be used for zero-install MCP.

Use this skill for concrete implementation and operations, not generic payment theory.

## Workflow

1. Choose the role path first:
- Buyer agent: `lnd` + pay-only macaroon + `lnget` for paid API access.
- Seller agent: `lnd` + invoice-only macaroon + `aperture` paywall in front of backend.
- Observer agent: Lightning MCP server over LNC with read-only tools.
- Full loop: buyer and seller integration with explicit budgets and token checks.

2. Run preflight:
- `scripts/check-l402-prereqs.sh buyer`
- `scripts/check-l402-prereqs.sh seller`
- `scripts/check-l402-prereqs.sh observer`
- `scripts/check-l402-prereqs.sh full`

3. Bootstrap stack from [lightning-agent-tools-playbook](references/lightning-agent-tools-playbook.md):
- Install and start node components from `lightning-agent-tools/skills/*`.
- Use `lnget` for L402 buyer traffic.
- Use `aperture` for paid endpoint hosting.

4. Enforce security model from [security-and-macaroons](references/security-and-macaroons.md):
- Default to watch-only + remote signer for production.
- Bake and use least-privilege macaroons (`pay-only`, `invoice-only`, `signer-only`, `read-only`).
- Keep admin macaroons off agent runtime paths.

5. For read-only observability or assistant node introspection, use [mcp-observability](references/mcp-observability.md):
- Configure Lightning MCP server and connect by LNC pairing phrase.
- Use MCP tools for status, channels, invoices, payments, peers, and fee estimates.

6. Validate outcome before production traffic:
- Buyer: run `lnget --no-pay` and `lnget --max-cost` checks.
- Seller: verify 402 challenge and successful paid retry.
- Security: verify scoped macaroons in active config.

## Quick Commands

```bash
# Node + lnget setup (buyer path)
~/code/lightning-agent-tools/skills/lnd/scripts/install.sh
~/code/lightning-agent-tools/skills/lnd/scripts/create-wallet.sh --mode standalone
~/code/lightning-agent-tools/skills/lnd/scripts/start-lnd.sh
~/code/lightning-agent-tools/skills/lnget/scripts/install.sh
lnget config init
lnget --max-cost 500 https://api.example.com/paid-data.json

# Scoped buyer credentials (recommended)
~/code/lightning-agent-tools/skills/macaroon-bakery/scripts/bake.sh --role pay-only

# Seller path (aperture)
~/code/lightning-agent-tools/skills/aperture/scripts/install.sh
~/code/lightning-agent-tools/skills/aperture/scripts/setup.sh --insecure --port 8081
~/code/lightning-agent-tools/skills/aperture/scripts/start.sh

# MCP read-only path
~/code/lightning-agent-tools/skills/lightning-mcp-server/scripts/install.sh
~/code/lightning-agent-tools/skills/lightning-mcp-server/scripts/configure.sh --production
```

## Reference Files

- [lightning-agent-tools-playbook](references/lightning-agent-tools-playbook.md): practical buyer/seller workflow and end-to-end L402 loop.
- [security-and-macaroons](references/security-and-macaroons.md): remote signer tiers, role-scoped macaroons, and production hardening.
- [mcp-observability](references/mcp-observability.md): Lightning MCP setup over LNC and read-only operations surface.
