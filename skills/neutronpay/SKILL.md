---
name: neutronpay
description: Neutronpay MCP and SDK workflows for Lightning, stablecoin, and fiat payments.
metadata:
  oa:
    project: neutronpay
    identifier: neutronpay
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
      - process:spawn
---

# Neutronpay

## Overview

Integrate Neutronpay for agent-driven payments. Use this skill when tasks involve wiring the Neutron MCP server into AI tools (Cursor/Claude/Windsurf), building Neutron SDK backends, running Neutron AI-agent templates, or adding Lightning checkout flows in React apps.

## Environment

- Requires `bash`, `curl`, Node.js, and `npx`.
- Requires Neutron credentials from `portal.neutron.me`.
- Requires internet access to Neutron services and npm.

Use this skill for implementation and operations, not generic payments theory.

## Workflow

1. Pick the integration path first:
- MCP-first AI tool integration (`neutron-mcp`) for assistants using tool calls.
- SDK backend integration (`neutron-sdk`) for invoices/payments/status APIs.
- Agent runtime template (`neutron-ai-agent`) for pay-per-task automation.
- Frontend checkout (`neutron-react-payment-component`) backed by SDK endpoints.

2. Run preflight checks:
- `scripts/check-neutron-prereqs.sh mcp` for MCP configuration work.
- `scripts/check-neutron-prereqs.sh sdk` for backend SDK work.
- `scripts/check-neutron-prereqs.sh agent` for `neutron-ai-agent` flows.

3. Configure MCP from [mcp-sdk-agent-integration](references/mcp-sdk-agent-integration.md):
- Add MCP server config to tool-specific settings.
- Inject `NEUTRON_API_KEY` and `NEUTRON_API_SECRET` via environment.
- Restart/reload MCP and verify tool calls.

4. Implement Neutron API path:
- For MCP usage: validate balance lookup and invoice creation end-to-end.
- For SDK usage: wire `lightning.createInvoice`, transaction status checks, and webhook handling.
- For agent usage: wire webhook secret validation and payment-to-task completion flow.

5. Apply safety and policy controls:
- Never commit live API keys/secrets.
- Use per-environment credentials and explicit spend/risk limits.
- Require human confirmation for high-value sends.

## Quick Commands

```bash
# MCP preflight
scripts/check-neutron-prereqs.sh mcp

# Run Neutron MCP server
npx -y neutron-mcp

# SDK install in a repo
npm install neutron-sdk
```

## Reference Files

- [mcp-sdk-agent-integration](references/mcp-sdk-agent-integration.md): MCP setup, SDK entrypoints, agent template, and checkout wiring.
