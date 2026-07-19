# ACP-10 pinned peer release proof

Date: 2026-07-16
Issue: [#8897](https://github.com/OpenAgentsInc/openagents/issues/8897)
Live peer and shipped-admission integration revision: `6a26feb82244247b1d591c26aa5ff614b00b8b50`
Protocol: **Agent Client Protocol**, not Agent Communication Protocol and not A2A

## Verdict

Grok and Cursor satisfy every code-owned required scenario on the tested Darwin
arm64 platform. The checked release evidence is now compiled fail-closed into
the shipped main-owned Desktop admission and runtime path, so only the exact
pinned identities project `supported`. Substituted binaries, stale/incomplete
evidence, Grok 0.2.102, and every untested platform remain experimental or not
tested.

The checked machine ledger is
[`release-matrix.json`](../../../packages/agent-client-protocol-conformance/compatibility/release-matrix.json).
Its validator enforces the release/schema/platform/profile/binary/initialize
identities, the exact 47-scenario catalog, evidence freshness, repository-local
evidence references, and independently derived `releaseEligible`. It does not
trust a hand-written promotion bit or matrix-controlled requiredness flag.
Code assigns each scenario to `live-peer`, `optional-live-peer`,
`packaged-desktop-live`, `hermetic-production`, or `not-applicable`. Required
live peer and packaged Desktop rows require `live-pass`.
an executed production-transport fixture may satisfy only the explicitly
hermetic class. Grok passing never changes Cursor's gate, and Cursor passing
never changes Grok's gate.

Peer admission likewise cannot promote a supported-range binary from an
arbitrary passing live receipt. It requires the named full-release suite,
the exact executable digest, the tested OS/architecture, freshness, and (for
Cursor) the installation-closure digest. A smoke test from another platform or
a partial live probe therefore remains experimental.

| Peer         | Exact live identity                                                                            | Basic live result | Code-owned requirements unresolved | Claim        |
| ------------ | ---------------------------------------------------------------------------------------------- | ----------------: | ---------------------------------: | ------------ |
| Grok CLI     | `0.2.101`, executable SHA-256 `8431538d…4e2`                                                   |    30 live passes |                                  0 | supported |
| Cursor Agent | `2026.06.24-00-45-58-9f61de7`, launcher SHA-256 `b7babf47…edf`, closure SHA-256 `69d078da…faa` |    29 live passes |                                  0 | supported |

Only Darwin arm64 / macOS 26.4 / Node 24.13.1 was tested. Darwin x64, Linux
arm64, and Linux x64 are explicitly `not-tested`. Profile declaration is not a
platform compatibility receipt. Both installations were detected and pinned,
but installer provenance was not independently proven.

## Live evidence executed

Both runs used a newly created disposable Git repository. The retained records
contain update kinds, stop state, binary identity, and counts only—no prompt or
response text, session identifier, credential, hostname, username, or absolute
workspace path.

The checked
[`release-run-2026-07-16-darwin-arm64.json`](../../../packages/agent-client-protocol-conformance/compatibility/live/release-run-2026-07-16-darwin-arm64.json)
is now reproducible with the opt-in production runner. On revision
`661c1b3c7f74ac396026ec6aa1a0c6ea8845dcd1`, it launched both pinned peers in
separate disposable repositories and completed initialize, advertised local
authentication, session creation, two sequential real model turns, and stream
cancellation. Cursor additionally completed advertised session listing and a
mode change. Grok did not advertise session listing, and the runner does not
claim shutdown leak proof because it does not retain process leak counters.
The artifact is a candidate input only. It cannot mutate or promote the release
matrix.

The checked packaged Desktop receipts for
[Cursor](../../../packages/agent-client-protocol-conformance/compatibility/live/desktop-cursor-release-run-2026-07-16-darwin-arm64.json)
and [Grok](../../../packages/agent-client-protocol-conformance/compatibility/live/desktop-grok-release-run-2026-07-16-darwin-arm64.json)
use the production macOS app and exact admitted lanes. Both prove workspace
mismatch refusal, a real turn interrupted by app exit, durable
`interrupted_by_restart` settlement, explicit re-enable of the original
thread, a completed post-restart continuation, durable disable, and clean
shutdown. Grok additionally proved that a failed retry does not bypass its
one-session-per-process admission rule: the app restarted cleanly before the
original thread completed. The runner preserves the ordinary authenticated
HOME and never changes login or keychain state.

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
reported `crash`. A new process loaded the session and completed a post-repair
prompt. A bounded repetition run completed five crash cycles and five clean
cycles per peer with no exit-listener delta and zero pending request counters
after each clean shutdown. This is real crash/restart evidence, but it does not
cover every queue/update/stderr pressure combination in the matrix.

Both peers completed a real disposable-repository file tool call: three tool
updates were observed and the requested disposable file existed. No file or
tool content was retained. Grok's pinned implementation places cumulative
usage in notification `_meta` and turn usage/completion metadata in the
`session/prompt` response rather than emitting a stable `usage_update`. The
production bridge now preserves both private metadata rails. A fresh exact
0.2.101 run observed metadata on all 63 updates and both completed prompts,
with 62 usage-bearing observations. The checked receipt retains counts only in
[`release-run-grok-metadata-2026-07-16-darwin-arm64.json`](../../../packages/agent-client-protocol-conformance/compatibility/live/release-run-grok-metadata-2026-07-16-darwin-arm64.json).
Together with the live tool proof, this passes Grok's capability-aware combined
row. Grok advertises no mode/config surface. Cursor's live tool, plan, model,
mode, and configuration paths also pass. Its exact adapter returns only
`stopReason`, so the matrix records that capability-aware absence instead of
inventing an unsupported usage variant.

Both peers then passed live stream cancellation, cancellation while a reverse
interaction was outstanding, and two independent concurrent peer processes.
The Cursor reverse-cancel proof exposed and fixed a transport binding gap:
`cursor/create_plan` omits native `sessionId`, so the admitted handler now binds
the request to its resolved session before cancellation can target it. Both
peers also received broker-materialized MCP configuration scoped to the live
session. Grok reached initialize/list/call. Cursor reached initialize/list and
its post-run known-root scan found zero credential matches. A later Grok-only
run passed a random canary solely through the broker-materialized stdio server
environment, verified server receipt by SHA-256 digest, shut the peer down, and
completed a bounded scan of the exact disposable session tree, MCP logs, and
configuration/state surfaces: 23 files scanned and zero canary matches. The
redacted checked receipt is
[`release-run-grok-mcp-2026-07-16-darwin-arm64.json`](../../../packages/agent-client-protocol-conformance/compatibility/live/release-run-grok-mcp-2026-07-16-darwin-arm64.json).
Cursor model discovery returned 33 models (26 with configuration)
and `cursor/create_plan` passed live. The other Cursor extension requests were
not observed. A reproducible exact-binary qualification on the current runner
confirmed the same boundary—33 models and one create-plan request, but zero
question or todo calls—even after mode- and tool-qualified prompts. Its checked
partial receipt is
[`release-run-cursor-extensions-2026-07-16-darwin-arm64.json`](../../../packages/agent-client-protocol-conformance/compatibility/live/release-run-cursor-extensions-2026-07-16-darwin-arm64.json).
the production handlers for the two conditionally unobserved methods pass
their bounded contract suite and remain installed when the peer emits them.
the matrix does not falsely claim those model-dependent calls were observed.
A later exact-binary pass installed
an empty allow/deny policy only in the disposable repository's
`.cursor/cli.json`. Separate pinned processes then emitted one
`session/request_permission` each: OpenAgents selected one offered approval and
one offered refusal, with refusal occurring before command execution. No global
Cursor configuration changed.

Grok's exact-binary reverse qualification enabled the stable client filesystem
and terminal capabilities against the disposable repository. The latest peer
run made four filesystem and sixteen terminal reverse calls through bounded
handlers, so `fs-terminal-enabled` passes. Per-session metadata forced only two
new disposable sessions out of inherited YOLO/auto mode without changing
global configuration. Grok then emitted five peer-offered permission approvals
and one refusal in a separate session. The refusal occurred before command
execution. Pinned Grok 0.2.101 emitted the underscore spelling live. The same
allowlisted production handler accepts the historical non-underscore spelling,
so compatibility is qualified without claiming the exact binary emitted both.
The checked receipt is
[`release-run-grok-reverse-2026-07-16-darwin-arm64.json`](../../../packages/agent-client-protocol-conformance/compatibility/live/release-run-grok-reverse-2026-07-16-darwin-arm64.json).

The production main-owned Desktop host then ran both pinned drivers and
serialized the closed `openagents.desktop.acp-support.v1` bundle. The bundle
contained two provider entries, two receipt references and one evidence
reference per provider, and zero matches for retained prompt, private path,
authorization material, or provider-secret canaries.

The Cursor run authenticated through the advertised `cursor_login` method
against an already signed-in local session. A separate process using the same
ordinary HOME cancelled exactly once in the client decision callback before
`authenticate` and returned typed `auth_required`. It did not open a browser,
log out, or change keychain state. This proves client-side cancellation, not
pending-device login, expiry, logout, or clean re-authentication. The Grok run
used its existing cached-token path. A separate exact-binary run passed the
required auth-failure/recovery branch with an intentionally invalid ephemeral
API key followed by a fresh cached-token process. It did not alter stored
credentials. It does **not** prove intentional valid `xai.api_key` or literal
credential expiry. Neither exact peer advertised ACP `auth.logout`.
the matrix records logout as unsupported rather than demanding an unavailable
method. Fresh-process primary authentication is covered independently.
`xai.api_key` is an optional alternative only: the supported headless/ACP path
uses the existing local `cached_token` session and does not require an API key.
Desktop does not treat an ambient `XAI_API_KEY` inherited from the shell as an
intentional provider setting, so it preserves the cached local-login path by
default. The runtime's API-key path remains available only to an explicit
owner configuration surface.

Cursor authentication failure is covered without destroying the real login:
the production runtime maps a rejected `authenticate` request to typed
`auth_lost`, disposes the failed process, and permits a fresh-process
`cursor_login` retry. This proves the failure/retry contract while leaving the
user's stored credential and Keychain untouched.

A later exact-binary Grok process used ordinary HOME with the cached login
untouched, explicitly requested the advertised `grok.com` method, and invoked
the typed owner decision once. Cancellation returned `auth_required` before
any `authenticate` request, so no browser opened and no credential changed.
The redacted receipt is
[`release-run-grok-auth-cancel-2026-07-16-darwin-arm64.json`](../../../packages/agent-client-protocol-conformance/compatibility/live/release-run-grok-auth-cancel-2026-07-16-darwin-arm64.json).

## Hermetic evidence

The production bounded stdio transport executes the complete 23-member stable
manifest and the fault matrix. Current deterministic coverage includes:

- fragmented, oversized, malformed, unknown, duplicate, and late frames.
- reverse refusal/timeout, false-capability enforcement, auth omission/failure.
- startup through drain process exits, slow consumers, queue overload.
- cancellation races, replay/live interleaving, restart generation fencing.
- MCP broker reference materialization plus invalid/expired refusal and durable
  secret canaries. And
- executable identity, fixed argv/environment, profile admission, version, and
  installation-closure controls.

Those are real executions through production transport code, but they use the
scripted peer. The matrix therefore records them as `fixture-pass`, never as
named-peer live compatibility.

## Release blockers retained as data

No code-owned required scenario remains unresolved on tested Darwin arm64.
Non-Darwin-arm64 platform evidence remains absent and therefore cannot support
a claim on those targets.
Permission timeout, stale-response fencing, and policy denial are production
host authority semantics and pass through the hermetic production transport.
approval and refusal now pass against both pinned live peers.

Provider-specific gaps:

- Grok: Optional `xai.api_key` qualification is tracked separately and cannot
  block cached-session support. The pinned
  build's absence of advertised session listing is now retained as its exact
  live capability-false outcome.
- Cursor: signed-in `cursor_login` and cancellation pass live. Typed
  pending/failure/retry behavior passes hermetically without signing the user
  out. `cursor/ask_question` and `cursor/update_todos` are conditional methods
  that the exact model did not emit under qualification prompts. Their
  production handlers pass bounded contract tests and no emitted-call claim is
  made.
- Desktop: the isolated production build completed a real Grok Full Auto turn,
  durable journal settlement, disposable-repository commit, disable, and clean
  shutdown. The packaged Cursor journey now additionally proves workspace
  refusal plus interruption/restart recovery on the same durable thread. The
  equivalent packaged Grok journey also passes while preserving its
  one-session-per-process rule. The production main-owned host ran both pinned
  drivers, and its sanitized closed support bundle is live-proven for both
  peers.

The ACP-10 validator publishes the proof, derives the claim independently for
each peer, and fails closed. The shipped host consumes only the compiler output
of that complete matrix and passes the same evidence into runtime feature
gates. Current-revision candidate runs are retained in
`release-run-grok-current-2026-07-17-darwin-arm64.json` and
`release-run-cursor-current-2026-07-17-darwin-arm64.json`. They re-prove exact
identity, auth, sessions, real turns, cancellation, permissions, and the
provider-specific observed capability subset without inventing conditional
extension emissions. Packaging was explicitly excluded from the 2026-07-17
work order, so the already-checked packaged journeys remain the packaging
evidence. Future evidence can promote another version or platform only by
satisfying every code-owned evidence class for that exact identity.

## Verification commands

```bash
pnpm --dir packages/agent-client-protocol-conformance run typecheck
pnpm --dir packages/agent-client-protocol-conformance run test
pnpm --dir packages/agent-client-protocol-conformance run check:artifacts
pnpm --dir packages/agent-client-protocol-conformance run report
pnpm --dir packages/agent-client-protocol-conformance run check:release
ACP_RELEASE_LIVE=1 ACP_RELEASE_PEER=both \
  pnpm --dir packages/agent-client-protocol-conformance run live:release
```

`check:release` rejects stale/future evidence, incomplete or invented scenario
catalogs, matrix-controlled requiredness changes, missing repository-local
evidence, identity/platform drift, host-private paths, secret-shaped material,
unsupported status values, and any `supported`/`releaseEligible` assertion
lacking its code-owned evidence class.
