# OpenAgents MCP Contract

`@openagentsinc/mcp-contract` is the shared, transport-neutral contract for
OpenAgents MCP server and client work.

Phase 0 intentionally defines types and validation helpers only. It does not
start a stdio server, loopback listener, remote bridge, or external MCP client.
Later Pylon, Autopilot Desktop, Worker, web, Verse, payment, and coding-agent
surfaces should import these contracts instead of defining local protocol
shapes.

Current Phase 0 scope:

- shared contract package metadata;
- schema decode helpers;
- authority taxonomy and grant filtering;
- transport config and lifecycle schemas;
- tool, resource, and prompt descriptors;
- naming and resource URI rules;
- receipt, tagged error, progress, and elicitation schemas;
- public-safe output and redaction rules;
- import markers for Pylon, Autopilot Desktop, Worker/API, and web surfaces;
- package status metadata for docs and compatibility checks.

## Authority And Grants

The contract models MCP authority as explicit classes such as `public_read`,
`operator_read`, `private_account_read`, `workspace_read`,
`coding_session_control`, `approval_resolution`, `payment_read`,
`payment_receive`, and the high-risk classes `workspace_write`,
`payment_spend`, `deployment`, and `admin`.

Servers and clients should filter descriptors by granted authority before
returning list results. Ungranted capabilities must be absent from
`tools/list`, `resources/list`, and prompt projections rather than visible as
disabled entries. High-risk classes are absent by default and require explicit
grants before they can appear.

## Transport And Lifecycle

The contract models planned client/server transport kinds without starting
them: `stdio`, `loopback_http`, `streamable_http`, `sse`, `websocket`,
`ide_local`, `in_process`, and `bridge_proxy`.

MCP server config records also carry source metadata for local private config,
shared project config, user config, managed config, dynamic discovery, plugins,
IDE-provided config, and Desktop-discovered servers. Lifecycle status is
explicitly tagged, including `pending_approval`, `needs_auth`, `rejected`,
`revoked`, and `blocked_by_policy`, so clients can explain state without
retrying noisy failures.

Credential values are not part of transport config. Private config may carry
local credential refs, but public/debug projections omit those refs and expose
only the transport kind plus non-secret metadata.

## Descriptors And Names

Tool, resource, and prompt descriptors carry authority requirements, risk class,
schema refs, receipt/progress behavior, staleness metadata, and output handling
policy. Descriptor metadata is sufficient for grant filtering without importing
runtime app code.

OpenAgents MCP tool and prompt names use lowercase dotted identifiers such as
`pylon.health`, `verse.scene.state`, and `coding.session.spawn`. Resource URIs
use the `mcp://openagents/<namespace>/<path>` form. Phase 0 namespaces are
`pylon`, `autopilot`, `verse`, `worker`, `forum`, `payments`, and
`coding-session`.

## Receipts, Errors, Progress, And Elicitation

Errors are tagged (`missing_grant`, `needs_auth`, `blocked_by_policy`,
`unsafe_output_omitted`, and related tags) so callers can branch on stable
data rather than English message text.

Receipt schemas cover no-op, read, mutation, approval, payment receive/payment
spend, deployment, and admin outcomes. Receipts carry refs and summaries, not
raw prompts, tokens, mnemonics, local paths, or provider payloads.

Long-running tool calls can emit transport-neutral progress events. Client-side
elicitation uses tagged request/response records for approval prompts, auth
prompts, missing config, amount caps, and human confirmation.

## Output Safety

Output projections carry a safety class: `public`, `operator`,
`private_account`, `local_only`, `workspace_private`, `secret_bearing`, or
`omitted`. The contract includes truncation metadata and persistence policy
fields so callers can distinguish public summaries from operator-only summaries
and private refs.

The redaction helpers detect common unsafe material before projection:
mnemonics, access tokens, bearer tokens, private prompts, local absolute paths,
wallet secrets, and credential material. Secret-bearing or unsafe output is
omitted from serialized projections instead of being embedded into receipts,
progress events, diagnostics, issue comments, or public resources.

## Surface Imports

The Phase 0 package is imported by:

- `apps/pylon/src/mcp-contract-import.ts`;
- `apps/autopilot-desktop/src/mcp-contract-import.ts`;
- `apps/openagents.com/workers/api/src/mcp-contract-import.ts`;
- `apps/openagents.com/apps/web/src/mcp-contract-import.ts`.

Each marker records the shared schema version, surface authority, output safety
class, and reserved future transport kind while keeping
`runtimeTransportExposed: false`. These markers prove package resolution across
the current runtime surfaces without starting an MCP server, client connector,
loopback listener, or bridge.
