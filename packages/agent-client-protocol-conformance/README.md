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
  report keyed by the pinned Git revision and platform;
- broker-reference MCP fixtures proving invalid/expired refusal and immediate
  secret redaction; and
- Grok replay-load and Cursor resume/mode/config fixtures that pin the
  replay-to-live lifecycle barrier without claiming live-binary support; and
- explicitly opt-in diagnostic probes for `grok agent stdio` and Cursor
  `agent acp`.

The Grok and Cursor fixtures are independently versioned beneath
`fixtures/peers`. Their provenance labels say exactly what they prove. The
current checked fixtures are source-derived synthetic evidence, not captured
binary transcripts and not release compatibility claims. The checked ACP-10
release matrix owns pinned real-binary release admission. Stable, unstable, and
vendor extension fixtures remain in separate namespaces.

Live initialize summaries retain only safe identity/capability keys. This
includes nested auth-capability keys so unavailable stable methods such as
`auth.logout` can be recorded as exact unsupported behavior instead of an
invented release blocker.

The product lifecycle consuming these fixtures is specified by the
[Agent Client Protocol session runtime ADR](../../docs/adr/2026-07-16-agent-client-session-runtime.md).

Run the hermetic gates:

```bash
pnpm --dir packages/agent-client-protocol-conformance run typecheck
pnpm --dir packages/agent-client-protocol-conformance run test
pnpm --dir packages/agent-client-protocol-conformance run check:artifacts
pnpm --dir packages/agent-client-protocol-conformance run report
pnpm --dir packages/agent-client-protocol-conformance run check:release
```

`compatibility/release-matrix.json` is the release-defining named-peer ledger.
It uses a closed claim vocabulary (`supported`, `experimental`, `incompatible`,
`not-installed`, `auth-required`, `degraded`) and distinguishes `live-pass`
from fixture-only, blocked, untested, unsupported, and failed scenarios. The
validator enforces the exact release/schema/platform/profile/binary identities,
the complete 47-scenario catalog, freshness, and repository-local evidence
references before recomputing release eligibility. Requiredness and evidence
class are code-owned: live peer and packaged Desktop rows require live proof,
while only explicitly hermetic production-transport rows may be satisfied by
executed fixture proof. Matrix flags cannot self-exempt a provider, and one
provider can never mask the other. The current checked verdict is
`experimental` for both peers.

Live probes are inert unless explicitly armed and never run in ordinary CI:

```bash
GROK_ACP_LIVE=1 pnpm --dir packages/agent-client-protocol-conformance run live:grok
CURSOR_ACP_LIVE=1 pnpm --dir packages/agent-client-protocol-conformance run live:cursor
```

The deeper candidate runner launches the production Grok and Cursor runtimes in
separate disposable Git repositories, authenticates through the peers' normal
local login, performs sequential prompts, and attempts session listing, mode
changes, streaming cancellation, and bounded shutdown where advertised. Its
Grok lane also sends a random canary only through broker-materialized stdio MCP
environment, proves the disposable server received it by digest, shuts Grok
down, and scans the exact session/configuration persistence surfaces for the
unredacted value:

```bash
ACP_RELEASE_LIVE=1 ACP_RELEASE_PEER=both \
  pnpm --dir packages/agent-client-protocol-conformance run live:release
```

Set `ACP_RELEASE_OUTPUT` to an absolute `.json` path beneath
`compatibility/live/` to retain its closed-schema, redacted receipt. The runner
does not retain prompts, responses, session IDs, auth material, or absolute
paths. Its artifact is deliberately marked `candidate-live` and has no power to
change the release matrix or promote either peer; a human-reviewed matrix edit
and the fail-closed `check:release` gate remain mandatory. The Cursor lane also
proves client-side `cursor_login` cancellation using the ordinary HOME and
stopping before `authenticate`; it never creates an empty HOME or changes login
or keychain state. Its exact-binary extension qualification registers only the
four allowlisted Cursor methods and retains aggregate observation counts; an
unobserved extension remains blocked rather than inferred from a prompt.

After packaging Desktop, the separately armed peer journey proves real app
workspace refusal, interruption, restart settlement, re-enable, completion,
disable, and shutdown without changing HOME or credential/keychain state:

```bash
ACP_DESKTOP_RELEASE_LIVE=1 ACP_DESKTOP_RELEASE_PEER=<grok-or-cursor> \
  ACP_DESKTOP_RELEASE_OUTPUT=<absolute compatibility/live JSON path> \
  pnpm --dir apps/openagents-desktop run smoke:acp-release
```

The checked live records beneath `compatibility/live/` pin the installed
commands, full reported builds, executable/installation-closure digests, wire
version, advertised auth methods, and capability keys. The release matrix adds
the deeper redacted peer runs described in the human ledger; every unexercised
required scenario remains an explicit blocker to a `supported` claim.

Each probe emits one machine-readable diagnostic result with command, binary
version, schema identity, and initialize outcome. It does not authenticate,
print secrets, or establish a release claim.
