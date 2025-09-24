# Model Context Protocol (MCP) Overview

Codex can act as an MCP client and server. This allows tools to be supplied by
external processes and converted into OpenAI tools for model calls.

Files:
- `codex-rs/mcp-client` — client integration (load servers from config).
- `codex-rs/mcp-server` — server implementation and message processing.
- `codex-rs/mcp-types` — shared types.

## Client integration

- `Config.mcp_servers` configures servers; tools are fetched and converted via
  `mcp_tool_to_openai_tool` (see `core-openai-tools.md`).
- Input schemas from servers are sanitized into the internal `JsonSchema`.

## Server side

- Codex can expose prompts (`Prompt`), tools, and other capabilities to
  compatible MCP clients.
- Message processing translates between protocol envelopes and Codex events.

