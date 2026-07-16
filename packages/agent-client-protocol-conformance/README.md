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
- Grok replay-load and Cursor resume/mode/config fixtures that pin the
  replay-to-live lifecycle barrier without claiming live-binary support; and
- explicitly opt-in diagnostic probes for `grok agent stdio` and Cursor
  `agent acp`.

The Grok and Cursor fixtures are independently versioned beneath
`fixtures/peers`. Their provenance labels say exactly what they prove. The
current checked fixtures are source-derived synthetic evidence, not captured
binary transcripts and not release compatibility claims. Issue #8897 owns
pinned real-binary admission. Stable, unstable, and vendor extension fixtures
remain in separate namespaces.

The product lifecycle consuming these fixtures is specified by the
[Agent Client Protocol session runtime ADR](../../docs/adr/2026-07-16-agent-client-session-runtime.md).

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

The checked Cursor initialize result in
`compatibility/live/cursor-2026.06.24-darwin-arm64.json` is diagnostic only. It
pins the installed command, full reported build, launcher and installation-closure digests, wire
version, advertised auth method, and capability keys; every unexercised
scenario is explicitly `not-proven` and remains blocked on #8897.

Each probe emits one machine-readable diagnostic result with command, binary
version, schema identity, and initialize outcome. It does not authenticate,
print secrets, or establish a release claim.
