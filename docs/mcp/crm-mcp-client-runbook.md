# CRM MCP Client Runbook

How to connect an MCP client to the OpenAgents CRM MCP server and the manual
compatibility smoke (epic #5991, sub-issue #5999). The in-process protocol smoke
(`workers/api/src/crm-mcp-smoke.test.ts`) covers the wire path + policy in CI;
this runbook is the external-client (Inspector / Claude Code / Codex) checklist.

## Endpoint + auth

- **Transport:** Streamable HTTP, JSON-RPC at `POST https://openagents.com/api/mcp`.
- **Discovery:** `GET https://openagents.com/.well-known/openagents-mcp.json` (public, refs-only).
- **Auth:** `Authorization: Bearer <token>` where `<token>` is either the admin
  API token (full CRM authority on the `X-OpenAgents-Tenant` header / default
  tenant) or a **scoped MCP grant** token.
- **Tenant:** bound to the credential. Admin callers may set `X-OpenAgents-Tenant`;
  scoped tokens are pinned to their grant's tenant. Client-supplied `args.tenant`
  is ignored.

### Mint a scoped grant (admin)

```
curl -sX POST https://openagents.com/api/operator/crm/mcp-grants \
  -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "content-type: application/json" \
  -d '{"tenant":"tenant.openagents","authorities":["operator_read"],"label":"read bot"}'
# -> { "grant": {...}, "token": "oa_mcp_..." }   (token shown ONCE)
```

Authorities: `operator_read` (reads + propose + dry-run batch), `workspace_write`
(template upsert + CSV import), `approval_resolution` (approve/reject commands).
Revoke: `DELETE /api/operator/crm/mcp-grants/<grantRef>`.

## Client config

**MCP Inspector / generic Streamable HTTP:**
```json
{ "type": "streamable-http", "url": "https://openagents.com/api/mcp",
  "headers": { "Authorization": "Bearer oa_mcp_..." } }
```

**Claude Code** (`.mcp.json` / `claude mcp add`):
```json
{ "mcpServers": { "openagents-crm": {
  "type": "http", "url": "https://openagents.com/api/mcp",
  "headers": { "Authorization": "Bearer oa_mcp_..." } } } }
```

**Codex** (`~/.codex/config.toml`):
```toml
[mcp_servers.openagents-crm]
url = "https://openagents.com/api/mcp"
http_headers = { Authorization = "Bearer oa_mcp_..." }
```

## Manual compatibility smoke

1. **initialize** — client connects; server returns protocolVersion + capabilities.
2. **tools/list** — only granted tools appear. An `operator_read` grant shows the
   read tools + `crm.send.command.propose` + `crm.batch.send`, but NOT
   `crm.send.command.approve` / `crm.import.run` / `crm.template.upsert`.
3. **tools/call `crm.contacts.list`** — returns shaped contacts for the bound tenant.
4. **tools/call `crm.contact.render`** `{contactId, template}` — returns a
   personalized preview + send eligibility; sends nothing.
5. **tools/call `crm.send.command.propose`** — records a `pending_approval`
   command; **nothing is sent**. An operator approves separately (Desktop CRM
   pane / `crm.send.command.approve` with an `approval_resolution` grant).
6. **Two clients, different grants** — a second client with a read-only grant
   sees fewer tools than an admin/approval client; grants produce different
   `tools/list` results.
7. **Suppressed address** — `crm.contact.render` for a suppressed/unsubscribed
   contact reports `eligibility.allowed=false`; a send is blocked.
8. **Revoke** — `DELETE` the grant; the client's next call returns 401.

## What it must NOT do

- No tool can bypass the CRM gates (suppression/unsubscribe, dry-run, approval).
- A read/propose client cannot approve a send or run an un-gated/live blast
  (`crm.batch.send` is dry-run only over MCP).
- No client reaches another tenant's data; no raw tokens/secrets in output.
