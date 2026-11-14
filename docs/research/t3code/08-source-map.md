# Source map, versions and audit method

## Reproducibility

- Reference checkout: `~/work/projects/repos/t3code`.
- Remote: `https://github.com/pingdotgg/t3code.git`.
- Pinned commit: `f313d0a6cb843551fb60e57a454a71e71a3a8c04`.
- Commit timestamp: `2026-11-13T20:10:55+05:30` (as recorded by Git).
- Commit subject: `fix(server): stop device daemon alongside slow hub shutdown (#33566)`.
- Reference checkout was clean when inspected and was not modified by this work.
- All T3 GitHub source links in this audit use that full commit ID.
- OpenAgents comparison base: `6fe29cc703`; references to local modules describe
  the inventory/parity documents at that base, not a newly tested integration.

Inspection used file inventories, targeted text searches, bounded source reads,
package/lockfile inspection and comparison with the repository's architecture
notes. Deepest coverage is the app/server boundary, V2 lifetime/durability,
Claude adapter/query runner and Codex protocol/adapter/auth paths. Other
providers are traced through their registered driver, launch/SDK/runtime
boundary and documented special behavior; this is not a line-by-line audit of
every provider's tool formatter, every UI component or every dependency.

No reference-repository script, installer, test, server or agent was executed.
No provider/login/keyring/session secret was inspected. The current OpenAI
app-server protocol page was read through the official documentation connector;
it is an explanatory reference outside the pinned T3 tree. Repository link and
source-anchor validation is distinct from running product tests.

## Dependency and native-version evidence

| Item | Evidence at the pinned revision | Interpretation |
| --- | --- | --- |
| Server / desktop | Package version `0.1.9` | Source version, not proof of published app installation |
| Mobile | Private workspace package `0.0.0`; Expo/React Native dependencies | Workspace metadata, not the App Store build number |
| Claude Agent SDK | Lockfile `0.3.321` | Actual resolved npm SDK, separate from configured Claude CLI |
| Cursor SDK | Lockfile `1.2.0`, patched | T3 wrapper/patch behavior is part of the integration |
| Muse SDK | Lockfile `1.4.6`, patched | Same qualification applies |
| Codex protocol package | Workspace `effect-codex-app-server` with generated schemas and local transport | Not an npm lock on the user's Codex executable |
| Codex compatibility advice for this T3 range | Recommended `>=0.159.0`; supported `>=0.156.0`; `0.149–<0.156` unsupported; older broken | Manifest policy, not proof that every future binary will work |
| Claude CLI advice for this T3 range | Recommended `>=2.1.301`; supported `>=2.1.280`; `2.1.111–<2.1.280` graceful; older unsupported | Distinct from SDK version and live capability probes |

Sources: [server package][server-package], [desktop package][desktop-package],
[mobile package][mobile-package], [lockfile][lock], [Codex package][codex-package],
[compatibility manifest][manifest]. Version numbers here are historical source
facts, not recommendations to upgrade an installation to an untested release.

## Entry points by question

Paths below are relative to the T3 repository, not this OpenAgents checkout.
The chapter citations link directly to the important implementations; these
entry points let a follow-up investigation widen from that evidence.

| Question | Main entry points |
| --- | --- |
| Which agents are actually registered? | `apps/server/src/provider/builtInDrivers.ts` |
| What is reusable provider state? | `packages/provider-core/src/server/driver.ts`, `ProviderAdapter.ts`, `ProviderHost.ts`; `apps/server/src/provider/ProviderInstanceRegistry.ts` |
| What does mobile share with web/desktop? | `apps/mobile/src/connection/runtime.ts`; `packages/client-runtime/src/connection/{driver,supervisor,registry}.ts` |
| Which wire protocol do apps use? | `packages/client-runtime/src/rpc/{session,client,protocol}.ts`; `packages/contracts/src/{rpc,orchestrationV2}.ts`; `apps/server/src/ws.ts` |
| How does mobile retain a send? | `apps/mobile/src/state/{thread-outbox-storage,thread-outbox-model,threadOutboxDelivery}.ts`; `packages/client-runtime/src/operations/commands.ts` |
| How does desktop start/authenticate a backend? | `apps/desktop/src/backend/{DesktopBackendManager,DesktopLocalEnvironmentAuth,DesktopBackendConfiguration}.ts` |
| How does remote routing work? | `docs/internals/remote.md`; `packages/ssh/src/tunnel.ts`; `apps/web/src/hostedPairing.ts` |
| What is Connect trusted to do? | `docs/internals/t3-connect.md`; `apps/server/src/cloud/CloudLink.ts`; `infra/relay/src/environments/EnvironmentConnector.ts` |
| What does a socket session authorize? | `apps/server/src/auth/{http,RpcAuthorization,SessionStore,McpOAuth}.ts`; `docs/internals/environment-auth.md` |
| When is a send durable? | `apps/server/src/orchestration-v2/{Orchestrator,EventSink,EffectWorker,CommandReceiptStore,ThreadMessageIntake}.ts` |
| Who owns a running process/thread? | `apps/server/src/orchestration-v2/{ProviderSessionManager,RunExecutionService,ProviderContinuationService}.ts` |
| What happens after a crash/reconnect? | `ProviderRuntimeRecoveryService.ts`, `RestartContinuation.ts`, `ProjectionMaintenance.ts` in `orchestration-v2/`; `ws.ts` thread subscription |
| What is a provider switch? | `ContextHandoffService.ts`, `ContextHandoffDelivery.ts`; `docs/internals/context-handoffs.md` |
| How do checkpoints/revert work? | `apps/server/src/checkpointing/CheckpointStore.ts`; provider rollback/fork implementations; `RunFinalizationService.ts` |
| How does Claude launch and stream? | `apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts`; `provider/Drivers/{ClaudeDriver,ClaudeHome}.ts`; `claudeHistoryWorker.ts` |
| How does Codex launch and speak? | `apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts`; `provider/codexLaunchArgs.ts`; `packages/effect-codex-app-server/src/{client,protocol,_internal/stdio}.ts` |
| Where is managed Codex auth? | `apps/server/src/provider/{CodexManagedRuntime,CodexManagedTokenBroker,CodexChatGptAuth,CodexChatGptHandoff,CodexInstallation}.ts`; `Drivers/CodexHomeLayout.ts` |
| What do agents call back into? | `apps/server/src/mcp/{McpSessionRegistry,McpToolAccess,threadAccess,AcpMcpStdioBridge}.ts`; `packages/provider-core/src/server/{mcpSession,McpProviderSessions}.ts` |
| How do generic ACP agents connect? | `packages/provider-acp/src/server/{AcpSessionRuntime,adapter}.ts`; `packages/effect-acp/src/{client,protocol}.ts` |
| How do the other drivers differ? | `packages/provider-{cursor,grok,fx,kiro,opencode,pi,muse,acp-registry}/src/server/`; `apps/server/src/provider/acp/` and `Adapters/AntigravityAdapterV2.ts` |
| How are binaries/models maintained? | `apps/server/src/provider/{providerMaintenanceRunner,model-manifest.json}`; `docs/internals/{providers,model-manifest,adding-a-provider}.md` |
| What tests represent the real path? | `docs/orchestration-v2/testing-strategy.md`; `apps/server/src/orchestration-v2/testkit/OrchestratorReplayRecovery.integration.test.ts`; adapter `.test.ts`/`.testkit.ts` and native transcript fixtures |

## Important source-reading traps

1. `apps/server/src/provider/Drivers` is not the full provider inventory; newer
   integrations moved into `packages/provider-*`.
2. The Claude query runner is inside the large `ClaudeAdapterV2.ts` file. Its
   test filename does not imply a matching standalone production module.
3. `effect-codex-app-server`'s generated methods show the available schema
   surface, not every method T3 actually invokes.
4. A provider's capability declaration can intentionally be narrower than a
   specialized helper in the same adapter. It is not a passing live test.
5. A managed runtime can restrict a generic driver's features; managed Codex
   disables native multi-agent even though the general adapter handles it.
6. An app RPC socket, provider stdio protocol, MCP callback, relay bootstrap,
   provider model-network connection and terminal PTY are distinct channels.
7. Native provider history, T3 projections, bounded events and mobile caches
   have different authority/retention; “chat history” alone is ambiguous.
8. Architecture notes explain intent and tricky historical cases. Where this
   audit gives exact flags, SDK methods or capability values, it also cites the
   implementation rather than treating prose as runtime evidence.

## Follow-up validation boundaries

To reproduce behavior rather than source structure, a separately authorized
qualification should record T3 commit, client platform/build, native provider
version, setup/auth family, route, runtime mode, workspace and capability
snapshot. Use scrubbed provider transcripts and isolated fixture workspaces.
Exercise the failure cases in [the reliability chapter](06-security-and-reliability.md).
Do not infer a deployment is secure or compatible from this source audit alone.

[server-package]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/package.json
[desktop-package]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/desktop/package.json
[mobile-package]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/package.json
[lock]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/pnpm-lock.yaml
[codex-package]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/effect-codex-app-server/package.json
[manifest]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/model-manifest.json
