# MCP, SDK, And Agent Integration

Use this reference for practical Neutronpay integration in AI/agent and app contexts.

## Source Of Truth

- Local index:
  - `~/code/neutronpay/README.md`
- Repos referenced by that index:
  - `~/code/neutronpay/neutron-mcp`
  - `~/code/neutronpay/neutron-sdk`
  - `~/code/neutronpay/neutron-ai-agent`
  - `~/code/neutronpay/neutron-react-payment-component`

## Credentials

Get credentials from `https://portal.neutron.me`.

Required env vars for most paths:
- `NEUTRON_API_KEY`
- `NEUTRON_API_SECRET`

Additional for agent path:
- `ANTHROPIC_API_KEY`
- `WEBHOOK_SECRET`

## MCP Server Path (Preferred For AI Tools)

The MCP server package is `neutron-mcp` and exposes tool operations for balances, invoices, sends, transactions, and webhook-related workflows.

Core local references:
- `~/code/neutronpay/neutron-mcp/README.md`
- `~/code/neutronpay/neutron-mcp/src/index.ts`
- `~/code/neutronpay/neutron-mcp/src/neutron-client.ts`

## Cursor Config

Create or update `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "neutron": {
      "command": "npx",
      "args": ["-y", "neutron-mcp"],
      "env": {
        "NEUTRON_API_KEY": "${NEUTRON_API_KEY}",
        "NEUTRON_API_SECRET": "${NEUTRON_API_SECRET}"
      }
    }
  }
}
```

## Other MCP Hosts

- Claude Desktop:
  - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
  - Windows: `%APPDATA%\\Claude\\claude_desktop_config.json`
- Claude Code: `~/.claude.json` or project `.mcp.json`
- Windsurf/Cline: use their MCP settings with the same `command`/`args`/`env`.

## MCP Validation Checklist

1. Confirm host loads the `neutron` MCP server.
2. Run a low-risk read action first (wallet balances).
3. Create a small test invoice.
4. Confirm transaction status and resulting balance changes.

## SDK Path (Programmatic Backend)

Use `neutron-sdk` for direct API integration.

Core local references:
- `~/code/neutronpay/neutron-sdk/README.md`
- `~/code/neutronpay/neutron-sdk/src/index.ts`
- `~/code/neutronpay/neutron-sdk/src/resources/account.ts`
- `~/code/neutronpay/neutron-sdk/src/resources/lightning.ts`
- `~/code/neutronpay/neutron-sdk/src/resources/transactions.ts`
- `~/code/neutronpay/neutron-sdk/src/resources/webhooks.ts`
- `~/code/neutronpay/neutron-sdk/src/resources/rates.ts`
- `~/code/neutronpay/neutron-sdk/src/resources/fiat.ts`

Install:

```bash
npm install neutron-sdk
```

Usage pattern:

```ts
import { Neutron } from "neutron-sdk";

const neutron = new Neutron({
  apiKey: process.env.NEUTRON_API_KEY!,
  apiSecret: process.env.NEUTRON_API_SECRET!,
});

const wallets = await neutron.account.wallets();
const invoice = await neutron.lightning.createInvoice({ amountSats: 10000 });
```

## Agent Template Path

`neutron-ai-agent` provides a starter pay-per-task loop using Claude + Neutron + webhook callback handling.

Core local references:
- `~/code/neutronpay/neutron-ai-agent/README.md`
- `~/code/neutronpay/neutron-ai-agent/src/index.ts`
- `~/code/neutronpay/neutron-ai-agent/src/agent.ts`
- `~/code/neutronpay/neutron-ai-agent/src/tools.ts`
- `~/code/neutronpay/neutron-ai-agent/src/webhook.ts`

Run template:

```bash
cd ~/code/neutronpay/neutron-ai-agent
npm install
cp .env.example .env
# set NEUTRON_API_KEY, NEUTRON_API_SECRET, ANTHROPIC_API_KEY, WEBHOOK_SECRET
npm run dev
```

## React Checkout Path

Use `neutron-react-payment-component` in the frontend and keep invoice/status authority in backend endpoints powered by `neutron-sdk`.

Core local reference:
- `~/code/neutronpay/neutron-react-payment-component/README.md`

Expected backend endpoints:
- `POST /api/neutron/create-invoice`
- `GET /api/neutron/status`

## Operational Guardrails

- Keep keys out of source control and logs.
- Use separate credentials per environment.
- Use low-value test invoices first in new environments.
- Gate high-value sends behind explicit confirmation.
