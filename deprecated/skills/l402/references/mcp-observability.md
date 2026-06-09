# MCP Observability

Use this reference when the agent needs Lightning node observability without spend authority.

## Source Of Truth

- `~/code/lightning-agent-tools/docs/mcp-server.md`
- `~/code/lightning-agent-tools/skills/lightning-mcp-server/SKILL.md`
- `~/code/lightning-agent-tools/lightning-mcp-server/*`

## When To Use This Path

Use Lightning MCP over LNC when:
- you need read-only node introspection;
- you cannot expose direct gRPC ports to agent hosts;
- you want ephemeral session auth instead of persistent local credentials.

## Setup Paths

### Zero-install MCP registration

```bash
claude mcp add --transport stdio lnc -- npx -y @lightninglabs/lightning-mcp-server
```

### Source-driven setup

```bash
~/code/lightning-agent-tools/skills/lightning-mcp-server/scripts/install.sh
~/code/lightning-agent-tools/skills/lightning-mcp-server/scripts/configure.sh --production
~/code/lightning-agent-tools/skills/lightning-mcp-server/scripts/setup-claude-config.sh --scope project
```

## Connection Model

1. Register server and restart MCP host.
2. Connect with a 10-word LNC pairing phrase from Lightning Terminal.
3. Run read-only tools for state inspection.

The exposed surface includes read-only operations for:
- node status and balances;
- channels and pending channels;
- invoices and payments;
- peers and graph data;
- on-chain UTXOs, transactions, and fee estimates.

## Security Notes

- Pairing phrase is in-memory only for session setup.
- Sessions use encrypted mailbox transport.
- No payment send/open channel actions are exposed through this MCP surface.
- Use direct gRPC path only when state-changing actions are needed.
