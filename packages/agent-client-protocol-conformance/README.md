# Agent Client Protocol conformance

This private package is OpenAgents' hermetic conformance authority for the
pinned Agent Client Protocol stable wire-v1 surface. It is separate from both
the generated schema authority and product/provider policy.

It provides:

- an exact registry for all 23 stable manifest members and their support state;
- a data-driven peer launched through the production bounded stdio transport;
- concurrent multi-session, bidirectional, fragmentation, malformed-frame,
  lifecycle, capability, and resource-pressure oracles;
- complete stable content/update/stop/tool variant inventories, plus private
  retention of unknown future variants;
- deterministic, redacted native transcripts and checked coverage,
  compatibility declarations, plus an actually executed compatibility/fault
  report keyed by the current Git revision and platform;
- broker-reference MCP fixtures proving invalid/expired refusal and immediate
  secret redaction; and
- explicitly opt-in diagnostic probes for `grok agent stdio` and Cursor
  `agent acp`.

The Grok and Cursor fixtures are independently versioned beneath
`fixtures/peers`. Their provenance labels say exactly what they prove. The
current checked fixtures are source-derived synthetic evidence, not captured
binary transcripts and not release compatibility claims. Issue #8897 owns
pinned real-binary admission. Stable, unstable, and vendor extension fixtures
remain in separate namespaces.

Run the hermetic gates:

```bash
pnpm --dir packages/agent-client-protocol-conformance run typecheck
pnpm --dir packages/agent-client-protocol-conformance run test
pnpm --dir packages/agent-client-protocol-conformance run check:artifacts
pnpm --dir packages/agent-client-protocol-conformance run report
```

Live probes are inert unless explicitly armed and never run in ordinary CI:

```bash
GROK_ACP_LIVE=1 pnpm --dir packages/agent-client-protocol-conformance run live:grok
CURSOR_ACP_LIVE=1 pnpm --dir packages/agent-client-protocol-conformance run live:cursor
```

Each probe emits one machine-readable diagnostic result with command, binary
version, schema identity, and initialize outcome. It does not authenticate,
print secrets, or establish a release claim.
