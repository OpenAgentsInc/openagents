# ACP-10 pinned peer release proof

Date: 2026-07-16  
Issue: [#8897](https://github.com/OpenAgentsInc/openagents/issues/8897)  
Revision under test: `63cc0d073417d04bfa3146f7b92da1f385f9f420`  
Protocol: **Agent Client Protocol**, not Agent Communication Protocol and not A2A

## Verdict

Neither installed peer is eligible for a general `supported` claim yet. Both
remain independently `experimental`.

The checked machine ledger is
[`release-matrix.json`](../../../packages/agent-client-protocol-conformance/compatibility/release-matrix.json).
Its validator derives `releaseEligible`; it does not trust a hand-written
promotion bit. Every required scenario must be `live-pass`. A hermetic or
fixture pass is retained as useful implementation evidence but still blocks a
provider release claim. Grok passing never changes Cursor's gate, and Cursor
passing never changes Grok's gate.

| Peer | Exact live identity | Basic live result | Required scenarios not yet live-passed | Claim |
|---|---|---:|---:|---|
| Grok CLI | `0.2.101`, executable SHA-256 `8431538d…4e2` | 14 live passes | 27 | experimental |
| Cursor Agent | `2026.06.24-00-45-58-9f61de7`, launcher SHA-256 `b7babf47…edf`, closure SHA-256 `69d078da…faa` | 15 live passes | 25 | experimental |

Only Darwin arm64 / macOS 26.4 / Node 24.13.1 was tested. Darwin x64, Linux
arm64, and Linux x64 are explicitly `not-tested`; profile declaration is not a
platform compatibility receipt. Both installations were detected and pinned,
but installer provenance was not independently proven.

## Live evidence executed

Both runs used a newly created disposable Git repository. The retained records
contain update kinds, stop state, binary identity, and counts only—no prompt or
response text, session identifier, credential, hostname, username, or absolute
workspace path.

```text
GROK_ACP_LIVE=1 node --import tsx packages/grok-harness/scripts/live-acp-smoke.ts
result: pass; stop=end_turn; bytes=4; events=thread_ready,message_start,message_delta,message_done

CURSOR_ACP_LIVE_RUNTIME=1 CURSOR_ACP_LIVE_WORKSPACE=<disposable-absolute-path> \
  pnpm --dir packages/cursor-agent-runtime run live-smoke
result: pass; stop=end_turn; text length=15
updates: agent_message_chunk, agent_thought_chunk, available_commands_update, session_info_update
modes observed: agent, plan, ask; config identifiers observed: mode, model
```

Deeper live runs then completed two sequential turns, an orderly peer-process
restart, `session/load`, and a post-load turn for each provider. Cursor also
completed `session/list` (an empty result was observed for the disposable
workspace), an agent-plan-agent `session/set_mode` round trip, and the same
round trip through `session/set_config_option` for the mode option. These runs
retained only booleans, counts, update discriminators, and option identifiers.
Each pinned peer was then SIGKILLed during a live session. The transport
reported `crash`; a new process loaded the session and completed a post-repair
prompt. A bounded repetition run completed five crash cycles and five clean
cycles per peer with no exit-listener delta and zero pending request counters
after each clean shutdown. This is real crash/restart evidence, but it does not
cover every queue/update/stderr pressure combination in the matrix.

Cursor also completed a real disposable-repository file tool call: three tool
updates were observed and the requested disposable file existed. No file or
tool content was retained. The combined tool/plan/config/usage release row
remains unpassed because plan and usage variants were not induced.

The Cursor run authenticated through the advertised `cursor_login` method
against an already signed-in local session. It does **not** prove pending login,
cancellation, expiry, or clean re-authentication. The Grok run used its existing
cached-token path. It does **not** prove intentional `xai.api_key`, auth expiry,
logout, or re-authentication.

## Hermetic evidence

The production bounded stdio transport executes the complete 23-member stable
manifest and the fault matrix. Current deterministic coverage includes:

- fragmented, oversized, malformed, unknown, duplicate, and late frames;
- reverse refusal/timeout, false-capability enforcement, auth omission/failure;
- startup through drain process exits, slow consumers, queue overload;
- cancellation races, replay/live interleaving, restart generation fencing;
- MCP broker reference materialization plus invalid/expired refusal and durable
  secret canaries; and
- executable identity, fixed argv/environment, profile admission, version, and
  installation-closure controls.

Those are real executions through production transport code, but they use the
scripted peer. The matrix therefore records them as `fixture-pass`, never as
named-peer live compatibility.

## Release blockers retained as data

The largest shared gaps are live incompatible-version rejection; auth failure
and recovery; every required permission outcome; enabled reverse filesystem and
terminal behavior where claimed; broker-issued MCP on both real peers and a
post-run persistence scan; stream/reverse cancellation; real process crash,
restart, repair, and repeated start/stop pressure; sequential turns and multiple
real sessions under concurrent pressure; and complete sanitized support bundles.

Provider-specific gaps:

- Grok: intentional `xai.api_key`, both ask-question method spellings, and rich
  tool/plan/config/usage streaming. Orderly and SIGKILL restart/load are now
  live-proven.
- Cursor: pending/cancel/expiry/re-auth login, model listing, and all Cursor
  reverse extensions. List, mode/config round trips, and orderly restart/load
  and SIGKILL restart/load are live-proven.
- Both: packaged Desktop clean-machine happy/failure/recovery journeys. This is
  blocked on ACP-8 integration and cannot be replaced by direct runtime probes.

The release matrix is therefore useful now as the repeatable closure checklist,
but is deliberately not a release promotion artifact.

## Verification commands

```bash
pnpm --dir packages/agent-client-protocol-conformance run typecheck
pnpm --dir packages/agent-client-protocol-conformance run test
pnpm --dir packages/agent-client-protocol-conformance run check:artifacts
pnpm --dir packages/agent-client-protocol-conformance run report
pnpm --dir packages/agent-client-protocol-conformance run check:release
```

`check:release` rejects stale evidence, host-private paths, secret-shaped
material, mismatched Grok/Cursor scenario catalogs, unsupported status values,
and any `supported`/`releaseEligible` assertion lacking complete required live
evidence.
