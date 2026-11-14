# Security, failure handling and evidence limits

This chapter records design boundaries and operational consequences found in
source. It is not a penetration test or a list of demonstrated vulnerabilities.

## Credentials have different audiences

| Credential/authority | Accepted by | Does not establish |
| --- | --- | --- |
| Clerk cloud login/relay authorization | T3 Connect broker | A login to an arbitrary environment or provider |
| One-time pairing/bootstrap | Environment token exchange | Authority wider than its original grant; proof-bound bootstrap still needs its private key |
| Environment cookie/bearer/DPoP session | Environment HTTP/RPC with granted scopes | Automatic access to every RPC or another environment |
| WebSocket ticket | Its permitted socket/media route | A reusable general environment bearer token |
| T3-launched thread MCP token | T3 MCP endpoint and its invocation policy | An ordinary app/RPC login |
| External MCP-client OAuth session | MCP endpoint, with approved ceiling/read access | Access to other environment HTTP/WebSocket surfaces |
| Claude/Codex/other provider credentials | That provider's native runtime/auth flow | T3 pairing or account-chat authorization |
| Guest share token | Guest thread routes | Environment session, RPC or MCP access |

Sources: [environment authentication][auth-notes], [Connect trust model][connect],
[MCP session registry][mcp-registry]. Conflating any two rows can accidentally
turn discovery or a share link into execution authority.

## Environment authorization is enforced twice

The socket upgrade authenticates the session. Each application RPC then declares
its required scope and runs through `RpcScopeAuthorization` before its handler.
The declarations distinguish orchestration read/operate, terminal, filesystem,
access-management and other capabilities. Client-side feature visibility is
helpful presentation, not enforcement. [RPC authorization][rpc-auth],
[socket server][ws].

Pairing grants can be narrowed, not widened by token exchange. Creating a
pairing link requires access management plus the scopes being delegated.
The normal broad grant includes powerful execution: terminal operation is a
shell as the server user, and orchestration operation can start a full-access
agent. T3's auth notes explicitly recognize that withholding only a management
scope cannot contain a client that already has arbitrary execution.
[Grant model][auth-notes], [HTTP issuance][auth-http].

Cookie, bearer and DPoP adapt the same scoped session model. An invalid DPoP
proof fails rather than falling back to bearer. WebSocket tickets keep
long-lived tokens out of socket URLs; proof-bound RPC tickets require a fresh
DPoP proof at upgrade. Media tickets have different bearer/reuse requirements.
Revocation/expiry still controls live socket lifetime. Auth responses retain
legacy `scopes` while newer `permissions` describe the exact grant, so old
clients can decode responses without widening stored authority.
[Auth design][auth-notes], [session store][sessions].

## MCP is a third protocol boundary

T3 both controls agents and exposes tools those agents call back into. The
MCP registry issues random tokens, keeps hashes and thread/provider-instance
scope, sets liveness limits, and supports explicit thread/session revocation.
The config carries endpoint and authorization header separately from a normal
application session. A token's liveness can be touched by trusted runtime work;
an MCP call itself cannot revive an expired token.
[Registry][mcp-registry], [provider MCP config][mcp-config].

MCP tool declarations classify allowed callers/read versus mutation. The
invocation layer checks live thread ownership, effective runtime/plan limits,
target access and optional device/browser capability before handlers execute.
Background subagents may retain access while their owning thread has valid
pending work; archived/ended/wrong-instance callers must not act. A broad
description in the model's prompt does not substitute for this server check.
[Tool access declarations][tool-access], [thread access][thread-access].

Agents not launched by T3 can authenticate to `/mcp` through a public OAuth
authorization-code/PKCE flow. The user grants read-only or a runtime ceiling.
Those sessions use subject `mcp-client`; non-MCP HTTP/WebSocket paths reject
them so the client cannot escape its ceiling by using the broader RPC API.
HTTPS redirects are supported for hosted clients, which makes identifying the
recipient host on the consent page important. [MCP OAuth][mcp-oauth],
[auth design][auth-notes].

Peer environments use this external-MCP path too. The initiating environment
stores the destination's session, forwards calls with `T3-Mode-Limit` and
namespaces idempotency keys. Limited peer-origin work is stamped and fenced
from unrelated mutations/links/setup scripts; reads are not equivalently
fenced. Full-access peer work is not isolated from the receiving OS user.
Unlinking locally forgets the token; destination revocation is separate.
[Peer trust and routing][remote].

## Filesystem and tool authority

The environment is the filesystem boundary. Projects organize work; they are
not OS sandboxes. `filesystem:read` can include absolute paths the server user
can read outside a project. Relative-path/write rules and native provider
sandbox rules still apply. A phone paired to a privileged environment can
therefore gain much more than permission to view one repository.
[Filesystem boundary][auth-notes], [workspace rules][workspace].

Uploads are validated/claimed for their thread and live outside the project.
Adapters choose native attachment input formats and permitted paths. Copying
an attachment into a workspace to bypass a provider restriction would change
the authority boundary. Stored file-bearing events also affect downgrade
compatibility: an older decoder can fail on persisted history, not only a new
live message. [Attachment claims][attachments], [provider notes][providers].

Signed asset URLs are bearer grants for a canonical file/device/inode identity.
They can remain usable until expiry even after the minting session is revoked;
they are not equivalent to session-bound filesystem reads. Remote browser/media
features bring their own credentials and should not inherit chat scope by
accident. [Asset boundary][auth-notes], [asset access][assets].

## Account/profile isolation and authentication transitions

The provider instance owns mutable account, catalog and runtime state. The
adapter registry coordinates shared credential bindings while sign-in changes,
and managed auth controllers gate admission/stop dependent work when needed.
The boundary is most visible in Codex shadow homes, managed refresh/profile
revision checks and Antigravity profiles. [Driver contract][driver],
[adapter registry][adapter-registry], [provider notes][providers].

Antigravity forces file credential storage for isolated profiles because a
shared OS Keychain would undermine that separation. It strips ambient Google
credentials, links only intended global skill directories, and keeps shared
immutable executable installation separate from account profiles. Its sign-out
orders admission closure/process stopping before clearing credentials.
These are source-documented isolation measures, not a general claim that all
same-user code is mutually sandboxed. [Provider notes][providers].

Status and model catalogs are also account data. A stale model list cannot
prove current account access; an authoritative empty list should clear it.
Health/status probes should avoid opening sessions that can execute hooks,
start MCP servers or launch login. Short-lived text-generation helpers face
the same pre-prompt issue; T3's guidance calls for rejecting problematic
configured profiles before launch rather than relying on tool denial later.
[Provider notes][providers], [extension guide][adding].

## Failures that a correct client must distinguish

| Failure | Correct interpretation in the audited design |
| --- | --- |
| Socket disconnected | Retain cache; supervisor repairs transport |
| Auth blocked/revoked/expired | Repair credentials or wait for authorized wakeup; transport spam will not fix it |
| Socket ready, subscription failed | Data synchronization error can exist independently of transport |
| Command acknowledged, agent startup failed | Durable intent was accepted; surface the failed execution outcome |
| Native process exited during approval | Stored UI record cannot resurrect a live callback |
| Async question remains after turn end | It can still be answered by a new message |
| Provider turn completed, child task running | Background lifetime and continuation still matter |
| Server restarted after a tool side effect | Do not assume replay is safe or exactly once |
| Cursor predates pruned events | Obtain an authoritative snapshot before resuming |
| Revert unsupported by native conversation | Refuse before changing files |

Sources: [connection design][connection], [overview][overview],
[provider protocol notes][providers], [recovery service][recovery]. These are
behavioral distinctions to preserve in a port, not new failure tests run here.

## Logging and maintenance

Native event logs include lifecycle/results/failures and filter high-volume
deltas/duplicate raw frames before copying or redaction. Payloads have a 64 KiB
encoded diagnostic budget and bounded structural summarization. Codex redacts
sensitive key names; Claude avoids leaking MCP headers through argv/registration
errors and avoids turning arbitrary startup stderr into user-visible failures.
Those are useful controls, but no key-name redactor can establish that free-text
prompts, tool output or logs contain no sensitive information.
[Provider diagnostics][providers], [Codex redaction][codex-redact],
[Claude runner][claude-runner].

Provider updates identify the installer that owns the executable, including
resolved paths/shims. They are not “run a global npm update for any agent.”
The maintenance runner checks the owner lock key again before mutation and
reports success only after reading a current installed version. Unproven,
pinned or immutable installs stay manual where appropriate. Long-lived native
processes and immutable installation leases have to survive updates safely;
an installed new version is not proof an old live process changed.
[Maintenance guidance][providers], [runner][maintenance].

These provider/server updates are separate from mobile OTA delivery. Updating
an app bundle cannot add a missing server capability, and updating the server
cannot make a client decode a new required stored shape it does not understand.
[Remote version boundary][remote], [attachment compatibility][providers].

## Test evidence and what remains unproven

The tree contains transport/schema tests, adapter unit tests, real-provider
recorded transcripts, replay tests through the orchestrator/projections,
recovery regressions and opt-in live tests. The provider-extension guide
requires representative scenarios: simple/multi-turn, queue, interrupt,
steering, native resume and approvals. Real recordings test more than an
adapter-only fake happy path. [Test strategy][testing], [extension guide][adding],
[replay recovery test][replay].

For this audit, these artifacts were inspected as source evidence only. No
passing test result, latency measurement, platform qualification or successful
provider login is claimed. Before relying on the design in a product, validate
at least real-device background/resume, server crash after dispatch, native
process loss during approval, stale callback rejection, declined tool effects,
subagent attribution, snapshot fallback and the exact installed binary versions.
Those are recommended qualification cases, not additional work performed here.

[auth-notes]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/environment-auth.md
[connect]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/t3-connect.md
[rpc-auth]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/auth/RpcAuthorization.ts
[ws]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/ws.ts#L3540
[auth-http]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/auth/http.ts
[sessions]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/auth/SessionStore.ts
[mcp-registry]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/mcp/McpSessionRegistry.ts
[mcp-config]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-core/src/server/mcpSession.ts
[tool-access]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/mcp/McpToolAccess.ts
[thread-access]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/mcp/threadAccess.ts
[mcp-oauth]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/auth/McpOAuth.ts
[remote]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/remote.md
[workspace]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/workspace/WorkspaceFileSystem.ts
[attachments]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/AttachmentClaims.ts
[assets]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/assets/AssetAccess.ts
[driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-core/src/server/driver.ts
[adapter-registry]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProviderAdapterRegistry.ts
[providers]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/providers.md
[adding]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/adding-a-provider.md
[connection]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/connection-runtime.md
[overview]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/overview.md
[recovery]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProviderRuntimeRecoveryService.ts
[codex-redact]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L1670
[claude-runner]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L1325
[maintenance]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/providerMaintenanceRunner.ts
[testing]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/orchestration-v2/testing-strategy.md
[replay]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/testkit/OrchestratorReplayRecovery.integration.test.ts
