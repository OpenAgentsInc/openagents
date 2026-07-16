# ACP-10 pinned peer release proof

Date: 2026-07-16
Issue: [#8897](https://github.com/OpenAgentsInc/openagents/issues/8897)
Live peer evidence revision: `63cc0d073417d04bfa3146f7b92da1f385f9f420`
Release-candidate integration revision: `df03cf2ef76dda8f203083e7c22a02cd519b1a05`
Protocol: **Agent Client Protocol**, not Agent Communication Protocol and not A2A

## Verdict

Neither installed peer is eligible for a general `supported` claim yet. Both
remain independently `experimental`.

The checked machine ledger is
[`release-matrix.json`](../../../packages/agent-client-protocol-conformance/compatibility/release-matrix.json).
Its validator enforces the release/schema/platform/profile/binary/initialize
identities, the exact 47-scenario catalog, evidence freshness, repository-local
evidence references, and independently derived `releaseEligible`; it does not
trust a hand-written promotion bit or matrix-controlled requiredness flag.
Code assigns each scenario to `live-peer`, `packaged-desktop-live`,
`hermetic-production`, or `not-applicable`. The first two require `live-pass`;
an executed production-transport fixture may satisfy only the explicitly
hermetic class. Grok passing never changes Cursor's gate, and Cursor passing
never changes Grok's gate.

| Peer | Exact live identity | Basic live result | Code-owned requirements unresolved | Claim |
|---|---|---:|---:|---|
| Grok CLI | `0.2.101`, executable SHA-256 `8431538d…4e2` | 20 live passes | 13 | experimental |
| Cursor Agent | `2026.06.24-00-45-58-9f61de7`, launcher SHA-256 `b7babf47…edf`, closure SHA-256 `69d078da…faa` | 22 live passes | 10 | experimental |

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

Both peers then passed live stream cancellation, cancellation while a reverse
interaction was outstanding, and two independent concurrent peer processes.
The Cursor reverse-cancel proof exposed and fixed a transport binding gap:
`cursor/create_plan` omits native `sessionId`, so the admitted handler now binds
the request to its resolved session before cancellation can target it. Both
peers also received broker-materialized MCP configuration scoped to the live
session. Grok reached initialize/list/call; Cursor reached initialize/list and
its post-run known-root scan found zero credential matches. Grok's bounded
post-run persistence scan remains incomplete, so its no-durable-secret row is
not promoted. Cursor model discovery returned 33 models (26 with configuration)
and `cursor/create_plan` passed live; the other Cursor extension requests were
not observed.

The production main-owned Desktop host then ran both pinned drivers and
serialized the closed `openagents.desktop.acp-support.v1` bundle. The bundle
contained two provider entries, two receipt references and one evidence
reference per provider, and zero matches for retained prompt, private path,
authorization material, or provider-secret canaries.

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

The largest shared gaps are credential lifecycle paths, every required live
permission outcome, Grok enabled reverse filesystem/terminal behavior, complete
tool/plan/config/usage variants, the remaining MCP persistence proof, full
secret/support export scans, packaged Desktop journeys, and non-Darwin-arm64
platform evidence.

Provider-specific gaps:

- Grok: intentional valid `xai.api_key`, auth cancel/expiry/logout, the
  non-underscore ask-question spelling, session list, full permission/reverse
  authority, rich plan/config/usage streaming, and bounded MCP persistence scan.
- Cursor: pending/cancel/expiry/re-auth login, permission outcomes, and live
  `cursor/ask_question`/`cursor/update_todos`. Model listing, create-plan,
  list/load, mode/config round trips, cancellation, and restart/load are proven.
- Desktop: the isolated production build completed a real Grok Full Auto turn,
  durable journal settlement, disposable-repository commit, disable, and clean
  shutdown. Grok failure/recovery journeys, the corresponding packaged Cursor
  journey remain incomplete. The production main-owned host ran both pinned
  drivers, and its sanitized closed support bundle is live-proven for both
  peers. The combined Desktop journey row therefore stays `fixture-pass`, not
  `live-pass`.

The ACP-10 validator now publishes the proof, derives the claim independently
for each peer, and fails closed. Its current verdict is a release denial for
general support—not an implied promotion. The issue remains open because the
reproducible full live runner, remaining Desktop/provider failure journeys,
credential-dependent auth states, and claimed-platform executions are not complete. Future evidence can
change one peer at a time from `experimental` only by satisfying every
code-owned evidence class on each claimed platform.

## Verification commands

```bash
pnpm --dir packages/agent-client-protocol-conformance run typecheck
pnpm --dir packages/agent-client-protocol-conformance run test
pnpm --dir packages/agent-client-protocol-conformance run check:artifacts
pnpm --dir packages/agent-client-protocol-conformance run report
pnpm --dir packages/agent-client-protocol-conformance run check:release
```

`check:release` rejects stale/future evidence, incomplete or invented scenario
catalogs, matrix-controlled requiredness changes, missing repository-local
evidence, identity/platform drift, host-private paths, secret-shaped material,
unsupported status values, and any `supported`/`releaseEligible` assertion
lacking its code-owned evidence class.
