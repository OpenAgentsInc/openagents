# Agent Client Runtime Bridge

This private package is the protocol-to-domain boundary for the Agent Client Protocol used by Grok CLI and Cursor. It does not implement Linux Foundation Agent Communication Protocol or A2A.

The bridge retains each validated native payload in a bounded private evidence store before projecting renderer-safe `KhalaRuntimeEvent` records. Provider session IDs are scoped attachments; canonical thread, turn, message, and tool refs include the connection generation and are never copied from provider IDs.

Reverse requests are fail-closed. Filesystem, terminal, permission, and MCP capabilities are advertised only when a handler, scoped grant, tested broker, and current health are all present. Every effect is session/generation scoped and returns a refs-only receipt.

`./reverse-handlers` binds validated stdio reverse requests and response schemas to native JSON-RPC request IDs. `./node-brokers` supplies hardened workspace and owned-process implementations with containment, symlink, byte/output, environment, cancellation, and lifecycle enforcement. MCP launch material is reference-based, expiring, and callback-scoped specifically to `session/new`.

See [`docs/adr/2026-07-16-agent-client-runtime-bridge.md`](../../docs/adr/2026-07-16-agent-client-runtime-bridge.md) for the mapping and authority contract.
