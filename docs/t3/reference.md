# T3 Code Architecture & Provider Reference (`18850a5eeb`)

- **Audited:** 2026-11-22
- **Reference repo:** `~/work/projects/repos/t3code` (`pingdotgg/t3code`)
- **Pinned commit:** [`18850a5eeb388148fc2b8fa3499bf8e624a1154c`](https://github.com/pingdotgg/t3code/commit/18850a5eeb388148fc2b8fa3499bf8e624a1154c) (`2026-11-22 19:34:29 -0800`, tag `v0.1.12-nightly.20261123.5124`)
- **Package versions:** `@t3tools/desktop` & `t3` server `0.1.11`; mobile workspace `0.0.0` (TestFlight `3.0.0`, Android `2.3.0`)
- **Method:** Direct source inspection only. No upstream scripts, installers, binaries, or tests were executed; no credential files were opened.
- **Role:** Compact live replacement for the stale `0.1.9` (`f313d0a6cb`) details in [`docs/research/t3code/01`–`08`](../research/t3code/README.md) and [`docs/audits/2026-11-20-t3code-comprehensive-gap-analysis.md`](../audits/2026-11-20-t3code-comprehensive-gap-analysis.md). For **what OpenAgents should and should not borrow**, see [`2026-11-22-t3code-borrowing-theory.md`](2026-11-22-t3code-borrowing-theory.md). For **Antigravity 1.5+ quirks**, see [`antigravity-quirks.md`](antigravity-quirks.md).

---

## 1. System Topology & Scale at `18850a5eeb`

T3 Code is a TypeScript / Effect-TS / Electron / React 19 / Expo client–server control plane for external coding-agent CLIs. Every client surface (macOS/Linux/Windows Electron desktop, browser web, Expo iOS/Android) consumes one environment server (`apps/server`) through `@t3tools/client-runtime` over authenticated HTTP + Effect RPC JSON-over-WebSocket.

| Subsystem | Scale / Reality at `18850a5eeb` | Key Source Paths |
| --- | --- | --- |
| **Persistence** | SQLite with **110** top-level migrations + **11** `OrchestrationV2` migrations (projections, bounded ~24h event replay log, durable command receipts, `EffectOutbox`, full-text `messageSearchIndex`, `110_Workflows.ts`) | `apps/server/src/persistence/Migrations/`, `apps/server/src/orchestration-v2/` |
| **Provider Drivers** | **11 built-in drivers** (8 with usage readers) + generic ACP Registry | `apps/server/src/provider/builtInDrivers.ts`, `packages/provider-*` |
| **MCP Server** | Built-in `/mcp` HTTP server with **17 toolkits**, per-session scoped bearer tokens, outside-agent OAuth PKCE, MCP Apps iframes, and **MCP Events webhooks + polling/streaming** (`#40774`, `#41141`) | `apps/server/src/mcp/McpHttpServer.ts`, `apps/server/src/mcp/toolkits/`, `McpWebhookTransport.ts` |
| **Source Control & Issues** | **8 Git forges** (GitHub, Cursor Origin, GitLab, GitCafe, TanStack Source [flag-gated], Forgejo, Bitbucket, Azure DevOps) + forge issue trackers + **multi-account / multi-team Linear** (`#39179`, `#40023`) | `apps/server/src/sourceControl/sourceControlDefinitions.ts`, `apps/server/src/issue/IssueTrackerRegistry.ts` |
| **Durable Workflows** | Labs-gated (`workflowsEnabled`) journaled workflow engine on `effect/workflow` where agent steps are live threads returning schema-checked outputs (`t3_submit_output`) across local or linked environments (`#38882`, `#38884`) | `apps/server/src/workflows/`, `docs/internals/workflows.md` |
| **Settings & UI** | **28** settings routes, 61 theme tokens (5 dual-appearance OKLCH palettes + T3 Dark/Light + VS Code/OpenVSX + 100 KB custom CSS), VS Code `when`-clause keybindings, 10-locale `gt-i18n`, multi-repo projects (`#35925`), environment-label sidebar rows (`#40736`) | `apps/web/src/routes/_chat.settings*.tsx`, `packages/shared/src/themePalettes.ts` |
| **Native Helpers** | Targeted native helpers: macOS `apple-speech` (Swift), `browser-secret`, `libghostty-vt` (WASM terminal), `resource-monitor` (Rust), Linux Wayland/KDE/Hyprland/GNOME SnapShot capture | `native/`, `apps/desktop/src/snapShot/` |
| **Eval Suites** | **6** automated eval suites: `mcp-tool-use`, `pr-content`, `test-signal`, `thread-titles`, `voice-cleanup`, `workflow-authoring` | `evals/` |

---

## 2. What Changed Since `0.1.9` (`f313d0a6cb` → `18850a5eeb`, +2,121 Commits)

### Wave A (`f313d0a6cb` → `b568fcbaa6`, Nov 13–20, +1,800 commits)
1. **Multi-environment peer linking & remote delegation (`apps/server/src/peer/`):** One T3 environment signs into another's `/mcp` via OAuth (`t3code-peer-link`), forwards calls with narrowing-only `T3-Mode-Limit`, stamps immutable `linkOrigin` fencing on non-full-access remote threads, relays read-only threads (`PeerThreadRelay`), supports transitive linking through an intermediary environment (`#40036`), renews links on use (`T3-Link-Expires-At`), and renders `EnvironmentsMap`.
2. **Loopback preview tunneling & port forwarding (`apps/server/src/preview/PreviewTunnel.ts`):** `/api/preview/tunnel` tunnels TCP over WebSocket to a remote machine's `localhost`; desktop wires a fixed SOCKS5 proxy and local port forwarding (`#39393`); mobile opens remote `localhost` tabs; gated by `allowsPreviewTunnel` managed policy.
3. **DPoP-renewing pairing & Connect device approval (`apps/server/src/auth/`, `cloud/`):** Pairing-link sessions renew in place for 30 days via DPoP without dropping live sockets; T3 Connect adds opt-in `requireConnectDeviceApproval` per DPoP thumbprint; Cloudflare Durable Object `HookInboxObject` buffers automation webhooks up to 24h while offline.
4. **Antigravity 1.5+ (`v0.1.11`, 45 commits):** Minimum `>=1.5.0` gate, Auto review mode, Edit-from-here host-side rollback (`#38227`), background-command turn holds + `usage_update`, detached subagents + `SUBAGENT_STATE_KILLED`, `steersWithPrompt`, `SSH_CONNECTION` profile isolation, `--login` OAuth URL capture, `/compact`, lost thinking-signature recovery, `call_mcp_tool` unwrap. *(Full ledger in [`antigravity-quirks.md`](antigravity-quirks.md).)*
5. **Issue trackers, PR/CI MCP tools & native voice:** Multi-forge PR + CI-log + stack-merge MCP tools, Issues UI across 5+ trackers, native macOS `AppleSpeech.swift` dictation + `transcribe-cpp` + Silero VAD, typed turn results (`#38874`), native stopped-thread resume, `secret-provider-onepassword` multi-value secret cards, HMAC guest share links, MCP OAuth sign-in from warnings, VS Code/OpenVSX theme import, 10-locale i18n, and `localEnvironmentEnabled: false` thin-client desktop mode.

### Wave B (`b568fcbaa6` → `18850a5eeb`, Nov 20–22, +321 commits)
1. **Durable workflows (`#38882`, `#38884`):** Journaled `effect/workflow` engine in `apps/server/src/workflows/` with `110_Workflows.ts`, `workflow` MCP toolkit, `skills/t3-workflows/SKILL.md`, and web/mobile UI. Step failures pause rather than abort; agent steps are ordinary interactive threads that finish via schema-validated `t3_submit_output`; runs stay local while delegating individual agent steps to linked environments.
2. **External-agent MCP Events (`#40774`, `#41141`):** Outside MCP clients can poll, stream, or subscribe via signed webhooks (`McpWebhookTransport.ts`) to thread/delegation milestones.
3. **Multi-repo projects (`#35925`):** One T3 project can track multiple repositories (`104_ProjectionProjectsTrackedFolders.ts`, `threadRepositories.ts`, `docs/user/multi-repo-projects.md`).
4. **Sidebar evolution (`#40736`, `#33057`, `#26661`):** Sidebar thread rows switched from showing git branch to showing the **environment** label (moving branch + worktree path into the hover card); the sidebar folder button now picks or adds a project; settling a thread no longer jolts list scroll.
5. **More breadth:** 8th source-control forge (`tanstack-source`, behind centralized `T3CODE_FEATURE_FLAGS`, `#40207`, `#40594`), multiple Linear accounts per environment (`#39179`), agent problem-reporting feedback (`feedback` MCP toolkit, `108_AgentFeedback.ts`, `#35871`), and **22 post-`v0.1.11` Antigravity fixes** (tracked individually in [`antigravity-quirks.md`](antigravity-quirks.md)).

---

## 3. Core Architectural Invariants Worth Preserving (Layer 2)

These design laws from T3's `docs/internals/` and `orchestration-v2/` remain directly valuable when translated into `#authority` Coax and versioned `proto/`:

1. **Distinct identities:** Environment ≠ provider instance ≠ application thread ≠ provider native thread ≠ OS process ≠ run/attempt ≠ live pending callback. Never collapse them into one ID.
2. **Intent before external side effects:** Accepting a command durably commits `(commandId, receipt, projection, outbox effect)` before spawning provider work. Receipt ≠ completed external tool; crash recovery reconciles unknown effects rather than blindly replaying.
3. **Transport vs. auth vs. freshness:** A live socket or `200 OK` does not prove authorization or complete transcript history. Separate connection state, credential renewal/revocation, and replay-cursor freshness (with authoritative snapshot fallback when the bounded ~24h event log has pruned past a cursor).
4. **Request lifetime modeling:** Live process callbacks (`canUseTool`, `session/request_permission`, `item/tool/requestUserInput`) die with the child process and cannot be resurrected from disk. Async message-response questions (`responseCapability: { type: "message" }`) survive restarts because answering posts a new user message.
5. **Safe workspace rewind (`CheckpointRestoreSafety`):** Store per-turn workspace checkpoints in hidden Git refs (not branch history), refuse to restore a checkout shared with another active thread, and preflight conversation rollback before touching files.
6. **Credential-audience isolation:** Keep cloud/relay auth, pairing bootstrap, scoped environment sessions (DPoP/bearer), WebSocket tickets, thread-scoped MCP tokens, external MCP OAuth ceilings (`mcp-client`), provider logins, and guest share tokens strictly separate.
7. **Real-transcript replay testing:** Qualify orchestration, recovery, and approval races by replaying scrubbed real provider transcripts through the real orchestrator (`OrchestratorReplayRecovery.integration.test.ts`), not fake happy-path stubs.

---

## 4. Provider Matrix & Pinned Versions (`18850a5eeb`)

| Driver | Protocol / Transport | Lockfile SDK / Manifest Compatibility | Key Ownership & Behavioral Notes |
| --- | --- | --- | --- |
| **Codex** (`codex`) | `codex app-server` JSONL over stdio (`effect-codex-app-server`) | CLI `>=0.156.0` (rec `>=0.159.0`; `<0.149.0` broken) | Shared runtime across threads; `turn/steer`, `thread/fork`, `thread/revert`, async questions, shadow-home auth overlay + managed ChatGPT token broker; `#40773` full-access prompt fix; `#38740` runtime pruning; `CodexAdapterV2.ts` (9.4k LOC). |
| **Claude** (`claudeAgent`) | `@anthropic-ai/claude-agent-sdk` `query()` → Claude Code child | SDK **`0.3.321`**; CLI `>=2.1.280` (rec `>=2.1.301`; `2.1.111–<2.1.280` graceful) | 1 thread per query; `CLAUDE_CONFIG_DIR` (not `HOME`) isolation; MCP registered post-init via `setMcpServers` to keep headers out of argv; idle compaction (`IdleCompaction.ts`, `#39766`); shared macOS process-table poll (`#37585`); resume phantom-wake fix (`#41120`); `ClaudeAdapterV2.ts` (15.3k LOC). |
| **Antigravity** (`antigravity`) | ACP (`agy acp --gemini-dir ... --app-data-dir antigravity-acp`) | CLI **`>=1.5.0 <2`** (rec **`>=1.5.12 <2`**; `<1.5.0` broken for T3 `>=0.1.10`) | Auto review mode, Edit-from-here host rollback (`#38227`), held turns for background commands + reactive wakes (`#39989`, `#40858`), detached subagents + `SUBAGENT_STATE_KILLED`, `steersWithPrompt`, `SSH_CONNECTION` profile isolation, `--login` OAuth. See [`antigravity-quirks.md`](antigravity-quirks.md). |
| **Cursor** (`cursor`) | `@cursor/sdk` local Agent API (`Agent.create`/`resume`/`send`) | SDK **`1.4.2`** (patched); CLI `>=2026.05.09` | Upgraded from `1.2.0` → `1.4.2` patched; isolated worker runner, edit-from-here rewind, preserved ended turns on retry exhaustion (`#41176`). |
| **OpenCode** (`opencode`) | Managed/external HTTP server + SSE event stream | `>=2.0.18` (1.14.19–<2.0.0 graceful; `<1.14.19` broken) | 1.x uses per-thread managed server (directory-scoped MCP); 2.x shares 1 server per instance with per-thread `t3-code-<thread>` MCP permission fencing. |
| **Grok** (`grok`) | ACP (`grok agent stdio`) | `>=1.0.13` (`<1.0.13` broken) | `XAI_API_KEY` or cached-token auth; no `auto-accept-edits` mode; status probe avoids opening a session. |
| **fx** (`fx`) | ACP (`fx acp`) | `>=0.0.13` (`<0.0.13` unsupported) | Per-thread backend selection (`#29388`), Full access support (`#26969`), safe CLI updates (`#29399`). |
| **Kiro** (`kiro`) | ACP (`kiro-cli acp --agent-engine=v3 --auth-method=cli`) | `>=2.27.0` | CLI auth; dedicated permission/cancellation support layer. |
| **Pi** (`pi`) | `pi --mode rpc` JSON stream | `>=0.84.3` (rec `>=1.0.0`) | Injected namespaced extension supplies MCP bridge + blocking `tool_call` hook (not an OS sandbox). |
| **Muse** (`muse`) | `@muse-code/sdk` → `muse serve` MSP | SDK **`1.4.6`** (patched); CLI `>=1.4.2` | Schema envelope v1; read-only helper launches disable shell/write. |
| **ACP Registry** (`acpRegistry`) | Generic ACP (checksum-verified binaries, npx, uvx) | Agent-dependent | Discovery/config/auth/options/commands; text-generation helper unsupported. |

Usage-capable drivers (`BUILT_IN_USAGE_DRIVERS`): `Claude`, `Codex`, `Grok`, `Pi`, `OpenCode`, `Muse`, `Antigravity`, `Cursor` (8 of 11).

---

## 5. Up-to-Date Source Map (`~/work/projects/repos/t3code` @ `18850a5eeb`)

| Question | Primary Entry Points |
| --- | --- |
| Registered providers & compatibility | `apps/server/src/provider/builtInDrivers.ts`, `model-manifest.json`, `ProviderInstanceRegistry.ts`, `packages/provider-core/src/server/{driver,ProviderAdapter,ProviderHost}.ts` |
| Client runtime & route failover | `packages/client-runtime/src/connection/{driver,supervisor,registry}.ts`, `rpc/{session,client,protocol}.ts`, `apps/mobile/src/state/{thread-outbox-storage,threadOutboxDelivery}.ts` |
| Orchestration V2 & recovery | `apps/server/src/orchestration-v2/{Orchestrator,EventSink,EffectWorker,CommandReceiptStore,ThreadMessageIntake,ProviderSessionManager,RunExecutionService,RunFinalizationService,ProviderRuntimeRecoveryService,RestartContinuation,ProjectionMaintenance,IdleCompaction,ContextHandoffService,CheckpointRestoreSafety}.ts` |
| Durable workflows | `apps/server/src/workflows/{WorkflowEngine,WorkflowRuntime,WorkflowStore,WorkflowAgents,WorkflowForge}.ts`, `docs/internals/workflows.md` |
| Peer links & preview tunnels | `apps/server/src/peer/{PeerLinks,PeerThreadRelay,RemoteDelegation,PeerForwarding,PeerClientPairing}.ts`, `apps/server/src/preview/PreviewTunnel.ts` |
| Auth, DPoP & MCP server/webhooks | `apps/server/src/auth/{http,RpcAuthorization,SessionStore,McpOAuth,dpop}.ts`, `apps/server/src/mcp/{McpHttpServer,McpSessionRegistry,McpToolAccess,McpWebhookTransport,linkOrigin}.ts`, `toolkits/*` |
| Antigravity 1.5+ integration | `apps/server/src/orchestration-v2/Adapters/AntigravityAdapterV2.ts`, `apps/server/src/provider/Drivers/AntigravityDriver.ts`, `apps/server/src/provider/acp/{AntigravityAcpSupport,AntigravityProtocol,AntigravityChildWork,AntigravityClientFiles,AntigravityTurnOutcome}.ts` |
| Claude & Codex adapters | `apps/server/src/orchestration-v2/Adapters/{ClaudeAdapterV2,CodexAdapterV2}.ts`, `provider/Drivers/{ClaudeDriver,ClaudeHome,CodexDriver,CodexHomeLayout}.ts`, `packages/effect-codex-app-server/` |
| Checkpoints, forges & issues | `apps/server/src/checkpointing/CheckpointStore.ts`, `apps/server/src/sourceControl/{builtInDrivers,sourceControlDefinitions}.ts`, `apps/server/src/issue/{IssueService,IssueTrackerRegistry}.ts` |
| Desktop sidebar & theming | `apps/web/src/components/{Sidebar,Sidebar.logic,Sidebar.motion,Sidebar.shiftGuard}.ts(x)`, `components/sidebar/*`, `packages/shared/src/themePalettes.ts` |
| Voice & native helpers | `native/{apple-speech,browser-secret,libghostty-vt,resource-monitor}/`, `apps/server/src/speech/`, `apps/desktop/src/snapShot/` |
| Internal architecture docs & evals | `docs/internals/*.md`, `docs/orchestration-v2/*.md`, `evals/*`, `apps/server/src/orchestration-v2/testkit/OrchestratorReplayRecovery.integration.test.ts` |
