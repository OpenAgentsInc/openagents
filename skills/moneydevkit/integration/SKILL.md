---
name: integration
description: Integrate Lightning payment workflows using Money Dev Kit. Use when tasks involve setting up @moneydevkit/agent-wallet for autonomous agents, wiring @moneydevkit/nextjs or @moneydevkit/replit checkout flows, bootstrapping credentials via @moneydevkit/create, validating MDK environment variables, or applying hybrid architecture constraints (hosted API plus self-custodial node).
compatibility: Requires bash, curl, Node.js 20+, and internet access to npm and Money Dev Kit services.
metadata:
  oa:
    project: moneydevkit
    nostr:
      identifier: moneydevkit-integration
      version: "0.1.0"
      expiry_unix: "1798761600"
      capabilities_csv: "http:outbound filesystem:read"
---

# Money Dev Kit Integration

Use this skill for implementation tasks, not high-level Lightning theory.

## Workflow

1. Choose the right integration path first:
- `agent-wallet` path for autonomous agents and CLI automation (no API account required).
- `nextjs` or `replit` checkout path for hosted checkout UI and product catalog workflows (requires credentials).

2. Run preflight checks:
- `scripts/check-mdk-prereqs.sh agent-wallet` for wallet automation path.
- `scripts/check-mdk-prereqs.sh checkout` for API/checkout path.

3. Execute the selected path:
- Agent wallet flow from [agent-wallet-operations](references/agent-wallet-operations.md).
- Checkout flow from [checkout-integration](references/checkout-integration.md).

4. Apply architecture and custody constraints:
- Use [architecture-and-self-hosting](references/architecture-and-self-hosting.md) before finalizing deployment.
- Explicitly handle mnemonic custody, API key handling, and self-hosted vs hosted service decisions.

5. Verify outcome:
- For wallet path: can `receive`, `send`, and inspect `payments` with JSON responses.
- For checkout path: can create checkout, render hosted checkout page, expose `/api/mdk`, and verify paid status.

## Quick Commands

```bash
# Agent wallet path (signet recommended for testing)
npx @moneydevkit/agent-wallet@latest init --network signet
npx @moneydevkit/agent-wallet@latest status
npx @moneydevkit/agent-wallet@latest balance

# Checkout path credential bootstrap
npx @moneydevkit/create@latest
```

## Reference Files

- [agent-wallet-operations](references/agent-wallet-operations.md): no-account self-custodial CLI workflow.
- [checkout-integration](references/checkout-integration.md): Next.js/Replit wiring, env vars, and checkout loop.
- [architecture-and-self-hosting](references/architecture-and-self-hosting.md): hybrid model, trust boundaries, and self-host knobs.
