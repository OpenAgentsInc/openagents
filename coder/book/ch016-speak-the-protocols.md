# 16. Speak the protocols

> **Status: partly implemented.** OpenAgents implements `#authority` MCP policy,
> session expiry, OAuth XAA/Step-Up guards, name normalization, and ToolSearch
> scoring (`openagents/core/mcp.coax`), headless NDJSON/stdout-guard/StructuredIO
> control protocols (`openagents/core/sdk_protocol.coax`), plus live stdio/HTTP
> MCP (`coder/tui/mcp.coax`, `coder/tui/mcp_http.coax`) and ACP
> (`coder/tui/acp.coax`, `openagents/workers/agyworker.coax`). Full OAuth browser dance
> over every remote MCP server and end-to-end ACP multi-client conformance remain
> open.

A coding agent never lives alone. Editors speak ACP (Agent Client Protocol) to
drive an agent; tools and external services expose MCP (Model Context Protocol)
over stdio or streamable HTTP; and desktop or daemon hosts speak newline-delimited
JSON (`mcworker-wire.coax` and headless SDK control frames) to worker processes.

OpenAgents implements protocol invariants in two `#authority` modules compiled
into [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax), alongside
live CLI/TUI protocol transports in [`coder/tui/acp.coax`](../../coder/tui/acp.coax),
[`coder/tui/mcp.coax`](../../coder/tui/mcp.coax), and
[`coder/tui/mcp_http.coax`](../../coder/tui/mcp_http.coax):

- **MCP policy, transport resilience & ToolSearch (`openagents/core/mcp.coax`):**
  - 7-scope policy (`flawed_mcp_scope_allowed`) enforces strict enterprise
    managed-only gates while permitting SDK-supplied servers when explicitly
    authorized.
  - Pre-trust `headersHelper` guard (`flawed_mcp_headers_helper_allowed`)
    refuses executing project-scoped credential helper commands before workspace
    trust is accepted.
  - Streamable HTTP session expiry (`flawed_mcp_is_session_expired`) triggers
    only on HTTP 404 paired with JSON-RPC `-32001`, avoiding spurious resets on
    generic 404s.
  - OAuth 2.1 / XAA (`flawed_mcp_oauth_action`) enforces the XAA no-downgrade
    rule (`FLAWED_OAUTH_XAA_REFUSE_DOWNGRADE`) and Step-Up re-consent
    (`FLAWED_OAUTH_STEPUP_RECONSENT`).
  - `flawed_mcp_normalize_name` normalizes `claude.ai` and MCP server names
    deterministically, while `flawed_tool_search_score_term` scores deferred
    MCP tools without polluting the prompt cache.
- **Headless NDJSON, stdout guard & StructuredIO (`openagents/core/sdk_protocol.coax`):**
  - `flawed_ndjson_escape_terminators` escapes UTF-8 `U+2028`/`U+2029` line
    separators in one pass so NDJSON streams never split mid-record.
  - `flawed_stdout_guard_classify_line` diverts stray non-JSON text to stderr
    instead of corrupting machine-readable stdout.
  - `FlawedResolvedIdRing` deduplicates redelivered `control_response` frames,
    and `flawed_sdk_control_dispatch` enforces reply-before-work for
    `mcp_set_servers` to prevent host/CLI deadlocks.
- **Live CLI & Desktop wire transports:**
  - [`coder/tui/acp.coax`](../../coder/tui/acp.coax) exposes Coder as an ACP
    agent (`initialize`, `session/new`, `session/prompt`, `session/cancel`).
  - [`coder/tui/mcp.coax`](../../coder/tui/mcp.coax) and
    [`coder/tui/mcp_http.coax`](../../coder/tui/mcp_http.coax) serve and call
    MCP tools and resources over stdio and HTTP.
  - [`coax/lib/agent/engine/mcworker-wire.coax`](../../coax/lib/agent/engine/mcworker-wire.coax)
    carries typed NDJSON frames between OpenAgents Desktop and `oa-worker`,
    `oa-subagent`, `agyworker`, `claudeworker`, and `inky` (see
    [Chapter 41](ch041-worker-protocol-and-sub-agents.md)).

## Profile

This chapter's profile is `openagents-protocols-v1 (MCP, SDK NDJSON, ACP), bounded CPU`:

- MCP strict enterprise scopes block unauthorized user servers and pre-trust
  `headersHelper` execution.
- HTTP 404 + JSON-RPC `-32001` classifies session expiry; XAA refuses
  downgrades; Step-Up requires re-consent.
- NDJSON escapes `U+2028`/`U+2029`, diverts stray stdout text to stderr,
  deduplicates control responses, and dispatches `mcp_set_servers`
  reply-before-work.

## Worked example

The runnable example [protocols.coax](../examples/protocols.coax) exercises
`openagents/core/mcp.coax` and `openagents/core/sdk_protocol.coax` directly.

<!-- source: coder/examples/protocols.coax example -->
```jai
main :: () {
  // 1. MCP scope policy, pre-trust headersHelper guard, and 404 + -32001 session expiry
  sdk_ok := flawed_mcp_scope_allowed(FLAWED_MSCOPE_SDK, true, true, false);
  user_blocked := flawed_mcp_scope_allowed(FLAWED_MSCOPE_USER, true, false, false);
  hdr_pre_trust := flawed_mcp_headers_helper_allowed(FLAWED_MSCOPE_PROJECT, true, false);
  exp_real := flawed_mcp_is_session_expired(404, -32001);
  exp_other := flawed_mcp_is_session_expired(404, 0);
  print("mcp policy: sdk_allowed=% user_in_strict=% pre_trust_helper=% expired_32001=% expired_404_only=%\n",
    sdk_ok, user_blocked, hdr_pre_trust, exp_real, exp_other);

  // 2. OAuth XAA no-downgrade + Step-Up re-consent + claude.ai name normalization
  xaa := flawed_mcp_oauth_action(true, false, false, 0, true);
  stepup := flawed_mcp_oauth_action(false, false, true, 999, true);
  nbuf: [32] u8;
  nn := flawed_mcp_normalize_name(" claude.ai  Slack! ", true, nbuf[..]);
  score := flawed_tool_search_score_term(true, true, false, false, true, true);
  print("mcp auth & search: xaa_refuse=% stepup_reconsent=% norm=\"%\" score=%\n",
    xaa == FLAWED_OAUTH_XAA_REFUSE_DOWNGRADE, stepup == FLAWED_OAUTH_STEPUP_RECONSENT,
    cast(string) nbuf[0..<nn], score);

  // 3. NDJSON U+2028 line-terminator escaper & stdout guard (diverts stray text to stderr)
  raw: [5] u8;
  raw[0] = 'a'; raw[1] = 0xE2; raw[2] = 0x80; raw[3] = 0xA8; raw[4] = 'b';
  esc: [16] u8;
  en := flawed_ndjson_escape_terminators(cast(string) raw[..], esc[..]);
  g_json := flawed_stdout_guard_classify_line("{\"type\":\"result\"}");
  g_stray := flawed_stdout_guard_classify_line("stray debug log");
  print("ndjson & stdout: escaped=\"%\" json_pass=% stray_divert=%\n",
    cast(string) esc[0..<en], g_json == FLAWED_GUARD_PASS, g_stray == FLAWED_GUARD_DIVERT_STDERR);

  // 4. StructuredIO dedup ring + deadlock-safe mcp_set_servers control dispatch
  ring: [1] FlawedResolvedIdRing;
  _ = flawed_resolved_ring_record(ring[..], 42);
  dup_action := flawed_sdk_classify_control_response(false, flawed_resolved_ring_has(ring[..], 42), true);
  disp_mcp := flawed_sdk_control_dispatch(FLAWED_CREQ_MCP_SET_SERVERS);
  disp_init := flawed_sdk_control_dispatch(FLAWED_CREQ_INITIALIZE);
  print("control io: dup_dropped=% mcp_reply_before_work=% init_inline=%\n",
    dup_action == FLAWED_CTRLRESP_DROP_DUP,
    disp_mcp == FLAWED_CDISP_REPLY_BEFORE_WORK,
    disp_init == FLAWED_CDISP_INLINE);
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/protocols.coax
```

```text
mcp policy: sdk_allowed=true user_in_strict=false pre_trust_helper=false expired_32001=true expired_404_only=false
mcp auth & search: xaa_refuse=true stepup_reconsent=true norm="claude_ai_Slack" score=18
ndjson & stdout: escaped="a\u2028b" json_pass=true stray_divert=true
control io: dup_dropped=true mcp_reply_before_work=true init_inline=true
```

The checked fixture is [protocols.out](../examples/protocols.out).

## What remains

- Live remote MCP OAuth 2.1 interactive login flow and cross-editor ACP
  conformance suite.

## Done means

- [x] MCP scope/auth/normalization/ToolSearch and NDJSON/StructuredIO control
  invariants pass `#authority` checks and are adopted by `openagents/runtime/oa-worker.coax`
  and `coder/tui/coder.coax`.
- [x] `coder/examples/protocols.coax` matches `coder/examples/protocols.out` in
  debug and release.
- [ ] Remote MCP OAuth interactive flows and full ACP editor conformance pass
  live end-to-end.
