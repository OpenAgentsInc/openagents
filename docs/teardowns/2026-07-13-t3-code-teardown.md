# T3 Code Teardown — 2026-07-13

Read-only architecture and product audit of the open-source T3 Code repository
(`pingdotgg/t3code`), pinned to commit
[`c1ec1915fc16f3dc1ec5d47d9a97f6210a574526`](https://github.com/pingdotgg/t3code/tree/c1ec1915fc16f3dc1ec5d47d9a97f6210a574526)
("[codex] Add Android mobile support (#3579)", committed 2026-07-12), cloned to
`projects/repos/t3code` and read as reference material only.

T3 Code matters to OpenAgents differently than the other teardown subjects.
Codex and Claude Code are engines; OpenCode is an engine plus workbench;
Executor is a capability substrate; Cursor is an IDE-turned-agent-platform.
T3 Code is the first audited product occupying almost exactly the OpenAgents P0
lane: **a local server that supervises multiple foreign coding-agent harnesses
(Codex, Claude Code, Cursor, Grok, OpenCode) in parallel worktrees, projected
to web, desktop, and mobile clients, with remote access and phone
notifications** — built by a team with large distribution (Theo/t3.gg) and, by
its own commit record, substantially built by the agents it hosts.

Evidence labels:

- **[source]** — observed directly in the pinned source tree
- **[schema]** — encoded in a typed Effect Schema, wire contract, or storage
  descriptor
- **[docs]** — stated by the repository's own checked-in documentation
- **[test]** — encoded in a checked-in test, CI workflow, or release check
- **[history]** — supported by the pinned Git history (full history fetched)
- **[public]** — corroborated by a named public source, fetched 2026-07-13
- **[inferred]** — concluded from several observations
- **[limitation]** — a boundary on what this audit can establish

No T3 Code source or user state was modified. No credentials, accounts, or
hosted relay behavior were exercised. Source proves intended implementation at
this commit; it does not prove every path is enabled in every release channel.

## TL;DR

T3 Code is a **provider-neutral control plane over other vendors' coding
agents, not an agent engine**. It has no model loop of its own. A Node/Bun
server (npm package literally named `t3`, default port 3773) wraps five
harnesses through four transport kinds — Codex over `codex app-server`
JSON-RPC/stdio, Claude Code in-process through the Claude Agent SDK, Cursor
and Grok over ACP (Zed's Agent Client Protocol), OpenCode over its V2 HTTP
SDK — and normalizes all of them into one **event-sourced CQRS orchestration
core** persisted in SQLite. React web, Electron desktop, and Expo mobile
clients consume one hand-written Effect RPC WebSocket contract with ordered
typed pushes. [source]

```text
web (React/Vite)   desktop (Electron)   mobile (Expo iOS/Android)
        \                 |                  /
         \     Effect RPC over WebSocket    /
          \   ordered typed push channels  /
           +--------------+---------------+
                          |
                 T3 server ("t3", :3773)
        ProviderRuntimeIngestion -> OrchestrationEngine
        decider -> SQLite event store -> projections
        CheckpointReactor / ProviderCommandReactor
        terminals, git, worktrees, preview browser, MCP
                          |
     +---------+----------+-----------+-----------+
     |         |          |           |           |
  codex     Claude     Cursor       Grok      OpenCode
app-server  Agent SDK   (ACP)       (ACP)     SDK v2 HTTP
JSON-RPC   in-process
```

The five most important findings:

1. **It is a third independent production adoption of Effect 4 beta as a
   whole-app substrate**, after OpenCode V2 (`beta.83`) and Executor
   (`beta.59`): T3 pins `effect 4.0.0-beta.78`, uses the unstable RPC, HTTP,
   SQL, process, CLI, and AI modules, typechecks with native `tsgo`, enforces
   Effect hygiene with a custom oxlint plugin, and **patches the `effect`
   package itself** rather than waiting for upstream. [source]
2. **The orchestration core is textbook event sourcing.** Client and internal
   commands pass invariant validation, a decider emits typed
   `OrchestrationEvent`s carrying `aggregateId`, `commandId`,
   `causationEventId`, and `correlationId`, an event store persists them, and
   projection tables serve reads. This is the durable-admission/one-owner
   architecture the OpenCode V2 teardown recommended, implemented across five
   foreign harnesses. [schema]
3. **Remote access is modeled as environments and endpoints, not a cloud.**
   One running T3 server is an `ExecutionEnvironment`; clients hold
   `KnownEnvironment` records reachable through multiple `AccessEndpoint`s
   (direct ws/wss, Cloudflare-tunnel relay, Tailscale-discovered addresses,
   desktop-managed SSH forward). Access and launch are deliberately separate
   concerns. Execution never leaves the user's machine; the hosted "T3
   Connect" layer is identity (Clerk), tunnels (`cloudflared`), and APNs push
   only. [docs] [source]
4. **The authority posture is inverted relative to OpenAgents.** Access to the
   environment is guarded by serious machinery — OAuth-style capability
   scopes, RFC 8693-shaped token exchange, pairing links, DPoP proofs — while
   execution containment defaults to `approvalPolicy: never` +
   `sandboxMode: danger-full-access` ("Full access" is the default runtime
   mode) and T3 ships no sandbox of its own, delegating containment entirely
   to the wrapped harnesses. [schema] [docs]
5. **The repository is a working agent-operated software factory.** 1,929
   commits between 2026-02-07 and 2026-07-12, merged PR numbers past #3899,
   277 commits carrying agent-tool prefixes such as `[codex]` (including the
   HEAD commit), Cursor bot committers, a checked-in `.plans/` corpus,
   vendored framework source in `.repos/` that agents are instructed to read
   before writing Effect code, vouch-gated community PRs, and nightly releases
   every three hours. [history] [source]

For OpenAgents, T3 Code is simultaneously the closest product-shape
competitor, a strong independent confirmation of the Effect/event-sourced/
one-server-many-clients architecture already chosen, and a clean example of
what OpenAgents must refuse: default-YOLO execution, no receipts, no
containment authority, and prose-free trust in wrapped harnesses.

## 1. Identification and scope

| Field | Value | Evidence |
| --- | --- | --- |
| Repository | `pingdotgg/t3code` | [source] |
| Commit | `c1ec1915fc16f3dc1ec5d47d9a97f6210a574526` | [source] |
| Commit time | 2026-07-12 12:03 +0200 | [source] |
| License | MIT | [source] `LICENSE` |
| First commit | 2026-02-07 ("Monorepo electron init") | [history] |
| Total commits | 1,929 | [history] |
| Merged PR numbers | ≥ #3899 | [history] |
| Tracked TypeScript | ~531,000 lines across ~1,939 `.ts`/`.tsx` files (tests and generated code included) | [source] |
| Runtime | Node `^24.13.1` (Bun-capable server), pnpm 11 | [source] |
| Framework | Effect `4.0.0-beta.78` (patched), Effect Schema, `@effect/atom-react` | [source] |
| npm package | `t3` `0.0.28` (`npx t3@latest`) | [source] `apps/server/package.json` |
| Desktop | Electron 41.5.0, electron-builder, electron-updater | [source] |
| Mobile | Expo SDK 56, iOS + Android, EAS builds | [source] |
| Toolchain | Vite Plus (`vp`), oxlint/oxfmt, `@effect/tsgo` + TypeScript-native preview | [source] |
| Public positioning | "a minimal web GUI for coding agents (currently Codex, Claude, Cursor, and OpenCode, more coming soon)" | [source] `README.md`; [public] |
| Traction | ~9.6k GitHub stars within months of launch; free/BYOK; installs via npx, brew cask, winget, AUR | [public] |

Authorship concentration is unusual for a repo this large: one maintainer
(1,434 of 1,929 commits) plus the founder (81), release bots, Cursor
bot/agent committers (~29), and a long tail of single-commit contributors.
Contributions are explicitly gated: `CONTRIBUTING.md` warns that large
unsolicited PRs will be closed, and CI labels PRs from a checked-in vouch
list (`.github/VOUCHED.td`). [history] [source] [test]

`CLAUDE.md` is a symlink to `AGENTS.md`, which mandates `vp check` and
`vp run typecheck` before task completion, names package roles, and requires
agents to read the vendored `.repos/effect-smol/LLMS.md` before writing Effect
code. The repo self-describes as "VERY EARLY WIP." [source]

Two docs-versus-code skews are worth recording as evidence-handling caveats:
`docs/architecture/providers.md` still claims "Codex is the only implemented
provider" while five drivers exist in source, and the README omits the
implemented Grok driver. Fast agent-built repos drift their own docs.
[source] [limitation]

## 2. Monorepo anatomy

pnpm workspace lanes: `apps/*`, `infra/*`, `packages/*`, `oxlint-plugin-t3code`,
`scripts`, plus `experiments/` and vendored `.repos/`. [source]

| Path | Role | Scale |
| --- | --- | --- |
| `apps/server` | The product core: npm `t3` CLI, WebSocket/HTTP server, provider drivers/adapters, orchestration engine, SQLite persistence, terminals, git/worktrees/checkpoints, preview browser, MCP server, cloud relay client | 457 files / ~158.7k lines |
| `apps/web` | React 19 + Vite + TanStack Router workbench: Lexical composer, xterm terminal, `@pierre/diffs` review, command palette, keybindings, preview browser UI | 554 / ~118.4k |
| `apps/mobile` | Expo 56 remote-control client with five Swift/Kotlin native modules (composer, markdown, review diff, terminal, controls), iOS Live Activities, Android (new at HEAD) | 404 / ~63.2k |
| `apps/desktop` | Electron shell: backend pool, WSL backend, SSH bridge, Tailscale endpoint provider, updater, preview automation via `playwright-core` | 115 / ~30.6k |
| `apps/marketing` | Two-page Astro site | 5 / 167 |
| `packages/contracts` | Schema-only shared contracts (enforced by policy: "no runtime logic") | 44 / ~14.2k |
| `packages/client-runtime` | Shared web/desktop/mobile connection runtime: `EnvironmentSupervisor`, `ConnectionBroker`, `RpcSessionFactory`, environment state | 129 / ~23.4k |
| `packages/effect-codex-app-server` | Generated Effect client for OpenAI's `codex app-server` JSON-RPC protocol | 17 / ~42.3k (mostly generated) |
| `packages/effect-acp` | Effect implementation of the Agent Client Protocol (agent + client + terminal), partially generated | 19 / ~15.0k |
| `packages/shared` | Runtime utilities incl. `DrainableWorker`; explicit subpath exports, no barrels | 80 / ~12.1k |
| `packages/ssh`, `packages/tailscale` | Remote access-method support | ~4.0k combined |
| `infra/relay` | Cloudflare Worker relay ("T3 Connect"): Clerk auth, DPoP, tunnels, PlanetScale Postgres via drizzle 1.0-rc, APNs Live Activity delivery, deployed with Alchemy | 60 / ~19.2k |
| `oxlint-plugin-t3code` | Custom lint rules: `namespace-node-imports`, `no-global-process-runtime`, `no-inline-schema-compile`, `no-manual-effect-runtime-in-tests` | 11 / ~1.0k |
| `patches/` | 13 pnpm patches, including to `effect` itself and `@effect/vitest` | — |

All [source]. Two structural details deserve emphasis:

- **The internal client↔server contract is hand-written Effect Schema**
  (`packages/contracts/src/rpc.ts`, 753 lines of `Rpc.make(...)` definitions
  plus a `WS_METHODS` map). Only the outward protocol clients (Codex
  app-server, ACP) are generated, via `@effect/openapi-generator`. [source]
- **`.repos/` vendors `effect-smol` and `alchemy-effect` as read-only
  reference subtrees for coding agents**, with instructions to prefer vendored
  source over "generated guesses or web search results," managed by a sync
  script — the same pattern as this workspace's `projects/` lane, checked
  into the product repo for its resident agents. [source]

## 3. The provider layer: five harnesses, four transports

There is no first-party model loop. The engine seam is a **driver/adapter
pattern** under `apps/server/src/provider/`: a driver is a plain value whose
`create()` returns a `ProviderInstance` bundling a live provider snapshot, a
session/turn/approval adapter, and a text-generation capability. [source]

| Provider | Transport | Mechanism |
| --- | --- | --- |
| Codex | subprocess | spawns `codex app-server`, JSON-RPC over stdio through the generated `effect-codex-app-server` client |
| Claude Code | in-process | `@anthropic-ai/claude-agent-sdk ^0.3.170`; driver kind `claudeAgent`; knows both npm and native `claude update` maintenance paths |
| Cursor | subprocess | ACP via `packages/effect-acp` plus `CursorAcpExtension` |
| Grok | subprocess | ACP via `effect-acp` plus `XAiAcpExtension`/`GrokAcpSupport` |
| OpenCode | HTTP | `@opencode-ai/sdk/v2` client (`OpencodeClient`, `PermissionRequest`, `QuestionRequest`) |

All [source]. Three details matter architecturally:

- **Multiple instances of one provider are first-class.** A user can run
  `codex_personal` and `codex_work` as fully independent app-server processes
  with isolated `CODEX_HOME` environments — account isolation by homedir, the
  same pattern as OpenAgents Pylon's isolated per-account Codex homes. [source]
- **Provider capability is probed, not assumed.** The Codex client reads
  account and skills data; the Claude driver probes Anthropic account and
  slash-command metadata; drivers own auto-update resolution for their
  harness. [source] [test]
- **Even utility text generation is delegated to the harnesses**: commit
  messages, PR titles/descriptions, and branch names are produced by shelling
  to `codex exec`, Claude, or Grok rather than by a separate model
  integration. [source]

This validates, from a shipping competitor, the exact seam OpenAgents built
for #8712-class harness-agnostic work: one neutral runtime-event vocabulary
(`ProviderRuntimeEvent`, versioned, with item-oriented start/delta/complete
events and request open/resolve) into which each adapter normalizes its
harness's native stream. [schema]

## 4. The orchestration core: event-sourced CQRS over SQLite

The most architecturally serious part of T3 Code is invisible in screenshots.
`packages/contracts/src/orchestration.ts` plus `apps/server/src/orchestration/`
implement a full command/event/projection pipeline [schema] [source]:

- **Commands**: `ClientOrchestrationCommand` ∪ `InternalOrchestrationCommand`,
  validated by dedicated command invariants before deciding.
- **Decider**: pure decision logic emitting typed events.
- **Events**: `OrchestrationEventType` covers project create/meta/delete and
  thread create/delete/archive/meta, runtime-mode and interaction-mode set,
  message-sent, turn-start-requested, turn-interrupt-requested,
  approval-response-requested, user-input-response-requested,
  checkpoint-revert-requested, reverted, session-stop-requested, session-set,
  proposed-plan-upserted, turn-diff-completed, and activity-appended. Every
  event carries `aggregateId`, `commandId`, `causationEventId`, and
  `correlationId`.
- **Store + projections**: migration `001_OrchestrationEvents` creates the
  event store; command receipts (`002`), checkpoint diff blobs (`003`),
  provider session runtime (`004`), and a family of projection tables
  (`ProjectionThreads`, `ProjectionTurns`, `ProjectionThreadMessages`,
  `ProjectionThreadActivities`, `ProjectionCheckpoints`,
  `ProjectionPendingApprovals`, `ProjectionThreadProposedPlans`,
  `ProjectionThreadSessions`, `ProjectionState`) serve reads. 32 numbered
  migrations exist at this commit, with data-transforming migrations carrying
  their own tests.
- **Reactors**: side effects (dispatching provider calls, capturing
  checkpoints, deleting threads) run in queue-backed workers built on a shared
  `DrainableWorker` primitive; a typed `RuntimeReceiptBus` publishes
  completion signals ("checkpoint capture, diff finalization, or a turn
  becoming fully quiescent") so tests and orchestration code wait on receipts
  instead of polling internal state. [docs] [source]

Token accounting is a first-class typed snapshot
(`ThreadTokenUsageSnapshot`: used/max/input/cached-input/output/reasoning
tokens, tool uses, duration) delivered through `thread.token-usage.updated`
events. [schema]

The SQLite layer runs WAL with foreign keys and selects its client at runtime:
`@effect/sql-sqlite-bun` under Bun, a home-grown `node:sqlite` wrapper under
Node — the server ships as an npm package but is Bun-capable. Provider
sessions carry an opaque `resumeCursor`; event replay is a client-callable
RPC; NDJSON provider event logs exist for diagnostics. [source]

Read against the teardown set: this is OpenCode V2's central lesson
(durable facts, projections, one owner, receipts as signals) independently
reinvented — but **without** V2's durable-admission inbox, steer/queue
delivery semantics, or replay-to-live synchronization marker, and with
volatile ordered pushes as the only live stream. Reconnect repair leans on
the replay RPC plus projections. [inferred]

## 5. Protocol: hand-written Effect RPC over one WebSocket

- **Transport**: "T3's primary transport is long-lived WebSocket RPC"
  [source: `apps/server/src/server.ts`]. Requests are `{id, method, params}`;
  pushes are typed envelopes with `channel` and a per-connection monotonic
  `sequence`, sent through one ordered `ServerPushBus`. Channels:
  `server.welcome`, `server.configUpdated`, `terminal.event`,
  `orchestration.domainEvent`. [docs] [source]
- **Boundary validation**: payloads are schema-validated at the transport
  boundary; decode failures produce structured `WsDecodeDiagnostic` with code,
  reason, and path. [docs]
- **Method surface**: `WS_METHODS` spans projects, providers
  (start/sendTurn/interrupt/respondToRequest/stop), filesystem browse, assets,
  vcs (worktree create/remove, ref create/switch, pull, status), stacked git
  actions and PR preparation, review diff preview, terminals, preview browser
  and preview automation, cloud relay, and source-control forges. [schema]
- **Client runtime**: web/desktop/mobile share `packages/client-runtime` — a
  per-environment scoped Effect context with `EnvironmentSupervisor` as the
  sole retry owner (exponential backoff capped at 16 s), a `ConnectionBroker`
  choosing among primary, bearer, relay, and SSH targets, and outbound
  requests queued while disconnected. Mobile loads thread snapshots over HTTP
  before live sync. [docs] [source] [history]
- **Startup gate**: the server withholds `server.welcome` until
  `ServerReadiness` barriers complete, so clients never hydrate against a
  half-started runtime. [docs]

Contrast with the references: Codex and OpenCode generate clients from the
protocol source; T3 hand-writes its wire contract in one schema package and
relies on shared code plus boundary decoding. That is workable while one team
owns every client, but there are no compatibility fixtures or
version-negotiation surfaces visible at this commit — the protocol version is
effectively "whatever this release shipped." [source] [limitation]

## 6. Remote architecture: environments, endpoints, and a deliberately thin cloud

`docs/architecture/remote.md` is the best product-thinking document in the
repository. Its model [docs]:

- **`ExecutionEnvironment`** — one running T3 server instance, identified by a
  stable `environmentId`. It owns provider auth state, projects/threads,
  terminals, git, filesystem, and settings. Desktop, mobile, and web all
  reason about the same primitive.
- **`KnownEnvironment`** — a client-local saved entry (LAN URL, public wss,
  SSH host, tunneled relay). Not server-authored; hosted-web entries are
  browser-local.
- **`AccessEndpoint`** — one concrete way to reach an environment. One
  environment may have many: `wss://t3.example.com`, `ws://10.0.0.25:3773`, a
  relay URL, a desktop-managed SSH tunnel resolving to a forwarded local URL.
- **`AdvertisedEndpoint`** — server/desktop-authored endpoint hints with
  reachability class (loopback/LAN/private/public/tunnel) and hosted-HTTPS
  compatibility flags; treated as hints, never proof of reachability.
  Endpoint providers plug in here — **Tailscale is the first provider**,
  contributing Tailnet IP and MagicDNS candidates without becoming part of
  the core model.
- **Access versus launch are separate questions.** Access: direct WebSocket,
  tunneled WebSocket, desktop-managed SSH port-forward. Launch: pre-existing
  server, desktop-managed remote launch over SSH (explicitly borrowing Zed's
  probing/session-directory/reconnect discipline while rejecting Zed's custom
  proxy protocol), or publishing a local server through a tunnel.
- **Hosted pairing** is a bootstrap URL for the static web app
  (`https://app.t3.codes/pair?host=...#token=...`) with the token in the URL
  hash so it never reaches the hosted origin; the hosted app never proxies
  traffic.

Sessions are environment-local by design: "a local clone and a remote clone
are different projects… threads still bind to one project in one
environment," with `RepositoryIdentity` as best-effort UI grouping only.
[docs]

The hosted layer ("T3 Connect", `infra/relay`) is a Cloudflare Worker deployed
with Alchemy, PlanetScale Postgres, one Clerk application shared by web,
desktop (`@clerk/electron` + passkeys), and mobile (`@clerk/expo`); the relay
accepts only Clerk JWTs minted from a dedicated `t3-relay` template with a
shared audience. Tunnels use a pinned managed `cloudflared` binary that the
CLI installs on demand. The headless path (`t3 connect login/link`,
`t3 serve`) uses a Clerk public OAuth client with PKCE on a loopback callback.
[docs] [source] [test]

The consequence: **T3's "cloud" cannot execute anything.** It authenticates,
relays, and pushes notifications. All execution and state stay in the
environment on the user's machine. [docs] [inferred]

## 7. Environment auth: capability scopes, token exchange, and DPoP

Access to an environment is governed by an OAuth-shaped system that is
markedly more rigorous than anything guarding execution [docs] [source]:

- capability scopes: `orchestration:read`, `orchestration:operate`,
  `terminal:operate`, `review:write`, `access:read/write`, `relay:read/write`;
- RFC 8693-shaped token exchange
  (`grant_type=urn:ietf:params:oauth:grant-type:token-exchange`) from an
  environment-bootstrap subject token;
- pairing links and browser session cookies; and
- **DPoP proofs** binding tokens to client keys (`apps/server/src/auth/dpop.ts`,
  relay `DpopProofs.ts`, migration `032_AuthPairingProofKeyThumbprint`).

This directly answers the weakness the OpenCode teardowns flagged (one shared
Basic password as local-server identity): T3 demonstrates that a local agent
server can have per-client, scope-limited, proof-of-possession credentials
without a heavyweight identity stack. [inferred]

## 8. Execution authority: the default is YOLO

The inversion is the single most important product finding [schema] [docs]:

```ts
export const ProviderApprovalPolicy = Schema.Literals(["untrusted","on-failure","on-request","never"]);
export const ProviderSandboxMode  = Schema.Literals(["read-only","workspace-write","danger-full-access"]);
export const RuntimeMode = Schema.Literals(["approval-required","auto-accept-edits","full-access"]);
export const DEFAULT_RUNTIME_MODE: RuntimeMode = "full-access";
```

`docs/architecture/runtime-modes.md` states it plainly: **Full access (the
default)** starts sessions with `approvalPolicy: never` and
`sandboxMode: danger-full-access`; **Supervised** opts into
`on-request`/`workspace-write` with in-app prompts. T3 implements no sandbox
of its own — no Seatbelt, Landlock, container, or egress layer exists in the
tree; containment is entirely whatever the wrapped harness enforces, and the
default asks the harness to enforce nothing. [schema] [docs] [inferred]

Approvals that do occur are properly modeled: request kinds
(`command`/`file-read`/`file-change`) flow through the orchestration event
model, pending approvals are persisted in a projection, and responses are
typed RPCs — so the *plumbing* for a supervised posture is real; the
*default* is not. [schema]

There is no authority manifest, no execution receipt, no
effective-containment record, and no per-run usage/receipt artifact beyond
token-usage snapshots. The `RuntimeReceiptBus` name refers to internal async
completion signals, not user-facing evidence. [source] [inferred]

## 9. Git: worktrees, hidden-ref checkpoints, and forge flows

- **Worktrees are the parallelism mechanic.** Threads can run in isolated
  worktrees (`worktreePath` on thread contracts; server-owned `worktreesDir`;
  typed `vcs.createWorktree`/`removeWorktree` RPCs; PR preparation supports
  `local` and `worktree` modes). Public commentary consistently identifies
  worktree-parallel agents as the flagship feature. [schema] [source] [public]
- **Checkpoints are hidden Git refs.** `CheckpointStore` captures workspace
  state at turn boundaries using an isolated temporary Git index and hidden
  refs — no commits on the user's branches — with diff blobs persisted and a
  typed revert path (`thread.checkpoint-revert-requested` → `thread.reverted`)
  through the event model. This is a leaner cousin of Claude Code's file
  checkpoints, scoped to turn boundaries and reactor-captured. [source] [docs]
- **Review**: server-side `ReviewService` + `review.getDiffPreview`, rendered
  by `@pierre/diffs` on web and a native diff module on mobile. [source]
- **Forges**: four source-control providers — GitHub (`gh`), GitLab (`glab`),
  Bitbucket (REST), Azure DevOps (CLI) — plus stacked git actions and
  one-click PR-from-thread flows whose text is generated by the providers
  themselves. [source] [docs]
- **VCS abstraction**: a `VcsDriver` registry with Git as the only driver —
  the seam exists for non-Git VCS. [source]

## 10. Desktop app

Electron 41.5.0 with a hardened baseline: app windows are created with
`contextIsolation: true, nodeIntegration: false, sandbox: true`. The one
deliberate exception is the preview-picker webview
(`contextIsolation=false, sandbox=true, nodeIntegration=false`), documented
with an unusually careful comment about Electron's truthy-string parsing
footgun, plus a `will-attach-webview` handler that force-sets safe values.
[source: `apps/desktop/src/window/DesktopWindow.ts`,
`preview/WebviewPreferences.ts`]

Structure highlights [source]:

- **Backend pool**: the desktop spawns and pools bundled T3 server backends
  (`DesktopBackendPool`/`DesktopBackendManager`), exposes them
  (`DesktopServerExposure`), and contributes Tailscale endpoints.
- **WSL**: parallel Windows and WSL backends with a mode picker.
- **SSH bridge**: the main process owns SSH probing, remote launch, askpass,
  and port-forwarding; the renderer then connects to an ordinary forwarded
  WebSocket URL — no SSH-specific renderer RPC.
- **Preview automation**: `playwright-core` drives the embedded preview
  browser for agent-run automation and recording.
- **Updater**: `electron-updater` with stable and nightly channels, a tested
  update state machine, and a local mock update server (an Effect HttpRouter
  static server with path-traversal guards) for exercising the update path.
  [source] [test]
- **Packaging**: an Effect CLI program stages a filtered workspace, resolves
  the pnpm catalog, and drives electron-builder for mac DMG (arm64/x64),
  Linux AppImage, and Windows NSIS (`com.t3tools.t3code`). Signing is
  auto-detected — Apple identity for macOS, Azure Trusted Signing for
  Windows — and **unsigned artifacts still release**. [source] [docs]

## 11. Mobile app

`apps/mobile` is a remote-control client, not a local execution surface. Expo
SDK 56, iOS and (at HEAD) Android, EAS preview/production CI lanes. [source]
[history]

- **Connection**: the shared client-runtime connection machinery with a
  schema-validated connection catalog in secure storage; direct ws/wss,
  relay-tunneled, and SSH-backed access methods.
- **Native modules**: five local Expo modules with Swift and Kotlin
  implementations — composer editor, markdown text, native review diff,
  native terminal (vendored emulator core), native controls — plus a patched
  `react-native-nitro-markdown` vendored as a tarball.
- **iOS Live Activities**: lock-screen agent status
  (`src/widgets/AgentActivity.tsx`), fed by the server's
  `AgentAwarenessRelay` through the relay's APNs delivery queue. Recent
  commits tune it ("show up to 5 Live Activity banner rows"). [source]
  [history]
- **Design**: iOS-26-era "liquid glass" aesthetics (`@callstack/liquid-glass`,
  `@expo/ui`), an `experiments/messages-glass-lab` Xcode lab, iPad split
  view. [source] [history]

The mobile thesis matches Cursor's Remote Control and the OpenAgents mobile
lane: users want to *supervise* running agents from the phone — status,
approvals, steering — not mirror a desktop. T3's Live Activity investment is
additional market evidence that ambient agent-status on the lock screen is a
differentiating supervision surface. [inferred]

## 12. MCP: tools handed to the wrapped agents

T3 runs its own MCP HTTP server (`effect/unstable/ai` `McpServer`/`Tool` —
the API its `effect` patch extends with DELETE-session support) to give the
wrapped harnesses tools, gated by a provider-scoped bearer credential. The
shipped toolkit is **preview automation**: the agent can drive and screenshot
the embedded preview browser, brokered through `PreviewAutomationBroker` and
streamed live to the owner. Skills and slash commands are surfaced read-only
from the providers. There is no third-party plugin system for T3 itself.
[source] [history] [inferred]

This is a small but sharp idea: the control plane feeds its own capabilities
back into the harnesses it supervises through the harnesses' native extension
protocol, rather than forking them. [inferred]

## 13. Verification and release engineering

- `@effect/vitest` (patched) everywhere; colocated tests at high density —
  every data-transforming migration has a test, release scripts have tests,
  a dev-launch regression got its own test after breaking. [source] [test]
- CI on Blacksmith runners: `vp check`, typecheck, desktop build, and a
  grep-based preload sanity check over the built Electron preload. [test]
- Releases: tag-triggered stable plus **nightly every three hours**; four
  desktop artifacts in parallel; GitHub Releases with electron-updater
  metadata; npm OIDC trusted publishing of `t3` (`latest`/`nightly`
  dist-tags); hosted-web channel aliasing on Vercel; Discord release
  notifications. [test] [docs]
- Supply chain: pnpm 11 `allowBuilds` and `minimumReleaseAgeExclude`
  hardening; Clerk crypto-wallet subdependencies stripped via overrides; 13
  audited patches instead of forks. [source]

## 14. The agent-operated factory

The development process is itself a finding. [history] [source]

- 1,929 commits in ~5 months; merged PR numbers past #3899.
- 277 commits carry agent-tool prefixes (`[codex]` and similar), including
  the HEAD Android-support commit; `Cursor Agent` and `cursor[bot]` appear as
  committers; `t3-code[bot]` handles releases.
- `.plans/` holds ~30 numbered implementation-plan markdown docs — a
  checked-in agent planning trail.
- `.repos/` vendors framework source with explicit instructions that agents
  prefer it over guesses or web search; `AGENTS.md` requires reading the
  Effect LLM docs before writing Effect code; `CLAUDE.md` symlinks to
  `AGENTS.md`; `.cursor/rules/` configures Cursor Cloud environments.
- Community input is vouch-gated and size-labeled by CI rather than open.

T3 Code is substantially built *by* the class of agents it supervises, at a
cadence (3-hourly nightlies) that presumes agents in the loop. This is the
most complete public example yet of the software-factory operating model
OpenAgents runs internally with its Khala fleet. [inferred]

## 15. Security assessment

### Strong choices

- Renderer hardening: contextIsolation/sandbox/no-nodeIntegration baseline
  with one documented, force-checked webview exception. [source]
- Per-client, scope-limited, DPoP-bound environment credentials with token
  exchange and pairing; tokens in URL hashes for hosted pairing. [docs]
  [source]
- One Clerk audience/template for relay JWTs; relay holds no session state.
  [docs]
- Boundary schema validation on every push/request with structured decode
  diagnostics. [docs]
- Isolated per-instance provider homes (no clobbering a user's live
  `~/.codex`). [source]
- Supply-chain hygiene: pinned managed `cloudflared`, npm OIDC publishing,
  pnpm build allowlists, dependency-age gates. [source] [test]

### Residual risks and gaps

- **Default `danger-full-access` + `approvalPolicy: never`.** The product
  default gives every wrapped agent unsandboxed host authority; "Supervised"
  is opt-in. [schema] [docs]
- No containment of T3's own: no sandbox, egress policy, or execution
  profile; effective authority is whatever each harness enforces, invisible
  to the user at the T3 surface. [inferred]
- No authority manifests, execution receipts, or delivery receipts; token
  usage snapshots are the only per-run accounting. [inferred]
- Unsigned desktop artifacts can ship when signing material is absent.
  [docs]
- Hand-written wire contract without version negotiation or compatibility
  fixtures. [limitation]
- The relay/identity plane, while thin, is a closed hosted dependency for
  remote/mobile convenience paths (tunnels, push); direct ws/wss and SSH
  remain account-free. [docs] [inferred]
- Framework risk: a patched Effect 4 beta, TypeScript-native preview
  compiler, and Vite Plus toolchain are all pre-1.0 load-bearing
  dependencies. [source]

## 16. Comparison with the reference set

| Dimension | T3 Code | OpenCode | Codex | OpenAgents direction |
| --- | --- | --- | --- | --- |
| What it is | Control plane over five foreign harnesses | Engine + workbench (own model loop) | Engine + protocol (own model loop) | Control plane *and* owned runtime, receipted |
| Engine seam | Driver/adapter per harness → neutral event vocabulary | Own server contract | app-server Thread/Turn/Item | Runtime Gateway + harness adapters (same shape as T3) |
| Core state | Event-sourced commands/events/projections in SQLite | V2 durable events + projections | JSONL rollouts + SQLite index | Append log + indexed canonical graph |
| Wire contract | Hand-written Effect RPC over WebSocket | Generated Promise/Effect clients from HttpApi | Generated TS/JSON Schema from Rust | Generated clients from Effect Schema |
| Effect | v4 beta.78, patched, whole-app | v4 beta.83, whole-app | n/a (Rust) | v4 beta, Effect Native everywhere |
| Containment | None of its own; default danger-full-access | Host tools, no OS sandbox | Seatbelt/bubblewrap/Windows + egress proxy | Fail-closed profiles + containment receipts |
| Remote | Environment/endpoint model; thin Clerk/CF relay; Tailscale; SSH | managed local service; SSH/WSL connections | remote-control relay w/ seq/ACK/pairing | Portable sessions as receipted authority transfer |
| Mobile | Real shipping Expo client w/ Live Activities | none | closed iOS surfaces | OpenAgents mobile on Khala Sync |
| Session portability | Environment-local threads; no host-to-host movement | Location-scoped | thread resume local to `CODEX_HOME` | Rev 30/31 host-portable sessions (unclaimed by all) |
| Receipts | Internal async signals only | none user-facing | usage + audit events | Authority/execution/delivery receipts as product |
| Economics | None (BYOK, free) | none | subscription-bound | Usage-truth pre-spend + settlement rails |

[inferred] The competitive read: T3 Code has already shipped a polished,
distributed version of the *supervision* half of the OpenAgents P0 — parallel
harnesses, worktrees, diff review, remote/mobile control — while leaving the
*authority* half (containment, receipts, provenance, portability, economics)
entirely unclaimed. It confirms the market and sharpens what OpenAgents must
be better at, not merely equal to.

## 17. What OpenAgents should adapt

### Adapt directly

1. **The environment/endpoint vocabulary.** `ExecutionEnvironment` /
   `KnownEnvironment` / `AccessEndpoint` / `AdvertisedEndpoint`, with access
   and launch as separate concerns and endpoint providers (Tailscale first)
   as plugins outside the core model, is cleaner language than "remote
   server" and matches the existing Tailnet runbook posture. Fold it into the
   portable-sessions pathway vocabulary.
2. **DPoP-bound, scope-limited local-server credentials.** T3 proves a local
   agent server can issue per-client capability tokens with proof of
   possession and token exchange. This should replace the "random password in
   a file" pattern wherever Pylon/Runtime Gateway exposes a socket, and
   inform Khala Sync device grants.
3. **Neutral provider-runtime event vocabulary with versioning.** T3's
   `ProviderRuntimeEventV2` union across five harnesses is convergent
   evidence for the OpenAgents harness-adapter seam; keep ours generated and
   version-negotiated where theirs is hand-written.
4. **Hidden-ref turn checkpoints.** Checkpoints as hidden Git refs captured
   by a reactor at turn boundaries — no user-visible commits, typed revert
   through the event model — are a cheap, honest middle ground between
   nothing and Claude Code's full file-history store.
5. **Receipts as test-visible completion signals.** The
   `RuntimeReceiptBus`/`DrainableWorker.drain()` pattern — every async
   pipeline emits a typed completion signal tests can await — is exactly the
   deterministic-verification ergonomics OpenAgents' oracles need at the
   runtime seam.
6. **Feeding owned capabilities to wrapped harnesses over MCP.** Exposing
   preview automation (and, for OpenAgents: receipts, policy queries, fleet
   context) to foreign harnesses through their native MCP support avoids
   forking them.
7. **Lock-screen agent presence.** Live Activities for running-agent status
   is the second incumbent signal (after Cursor Remote Control) that ambient
   mobile supervision is a differentiating surface for the OpenAgents mobile
   app.

### Adapt with stronger boundaries

- **Multi-instance provider homes** — already the Pylon pattern; T3 confirms
  it; keep account health/quota typed rather than implicit.
- **The relay shape** — a thin identity/tunnel/push plane with no execution
  or session custody is compatible with OpenAgents' local-first tier, but
  OpenAgents' equivalent must keep the account an opt-in upgrade and carry
  receipts, not just packets.
- **Event-sourced core** — adopt the command/causation/correlation metadata
  discipline, but add what T3 lacks: durable admission before scheduling,
  explicit steer/queue delivery, replay-to-live markers, and public/private
  projection classes.

### Do not copy

1. **Default-YOLO execution.** `full-access` as `DEFAULT_RUNTIME_MODE` with
   `danger-full-access` sandbox is the exact posture OpenAgents law forbids:
   owner-local danger mode must be explicit, local, visually persistent, and
   never a default.
2. **Containment by delegation.** "The harness has a sandbox" is not an
   authority model. OpenAgents keeps its own profiles, fail-closed
   negotiation, and effective-containment receipts regardless of harness.
3. **No receipts.** Turn completion, checkpoint capture, and PR creation
   without authority manifests or execution/delivery receipts leaves users
   trusting prose; this remains the differentiation seam.
4. **Hand-written wire contracts without negotiation.** Generate clients and
   ship compatibility fixtures from day one.
5. **Unsigned release fallbacks.** A missing certificate should fail the
   release lane, not ship an unsigned artifact.
6. **A stack of patched pre-1.0 toolchains as load-bearing dependencies**
   without regression gates. OpenAgents' Effect upgrades keep contract,
   startup, and resource-finalization gates; patching the framework is a
   last resort with an upstream PR attached.

## 18. Final assessment

T3 Code answers a narrow question extremely well: *given that the frontier
labs ship the engines, what does the owner-side control plane look like?* Its
answer — one local Effect server per environment, five harnesses behind one
event-sourced core, worktree-parallel threads, hidden-ref checkpoints, one
typed WebSocket contract for web/desktop/mobile, a thin identity/tunnel/push
relay, and agents building the product itself — is the strongest shipping
validation yet of the architecture OpenAgents chose for its own P0.

What it declines to answer is the half OpenAgents considers the product:
authority, containment, receipts, provenance, session portability, and
economic participation. T3's defaults trust every harness with the host; its
completion states are prose and diffs; its threads are forever bound to one
environment; its only economics is that it is free.

The correct OpenAgents response is neither to dismiss it (it is fast, polished,
and distributed) nor to chase its surface (the features are converging table
stakes — Cursor, OpenCode, and T3 now all ship worktree-parallel agents).
It is to land the same supervision ergonomics on top of the typed authority,
receipts, cross-device durable truth, and portable identity that none of the
incumbents — closed or open — has claimed.

## Addendum (2026-07-13, evening): Sidebar v2 concepts — live-state-grounded supervision design

A same-day design artifact surfaced that is worth pinning beside the source
audit: a "T3 Code Sidebar v2 Concepts" document at
[hsyscdqldmk5.postplan.dev](https://hsyscdqldmk5.postplan.dev/), presenting
five alternative thread-sidebar treatments for T3 Code, each rendered as a
full mock against real local data. [public]

Its own provenance line is the interesting part: "thread titles, branches,
models, statuses, diff stats, and message snippets are from your live
`~/.t3/userdata/state.sqlite` (Jul 13)," with states not present in the live
data (approval, input, plan-ready, failed) explicitly labeled as mocked onto
real threads "so every state renders in every concept." Design exploration is
grounded in the product's actual event-store projections rather than lorem
data, and the mock honestly discloses which states are synthetic. It also
confirms the local state-directory layout (`~/.t3/userdata/state.sqlite`)
from outside the repository. [public]

The five concepts, per the document [public]:

1. **Status Rail** — recency-sorted three-line cards with a colored status
   edge; the middle line answers "what is it doing?" with live activity text;
   ~10 threads per screen.
2. **Inbox** — email-shaped triage: project avatars, bold unread titles,
   two-line latest-assistant-message snippets; the lowest density (~72 px
   rows).
3. **Attention Tiers** — three automatic tiers: "Needs you" (pinned, with
   inline actions such as an Approve button carrying the exact command),
   "Running," and "Recent"; actionable items cannot scroll away.
4. **Adaptive Density** — one recency list whose row height varies by state:
   live threads expand with spinners, tools, and elapsed timers; settled
   threads collapse to one-liners ("the list breathes").
5. **Ops Grid** — a two-line monospace ledger with fixed columns (status
   glyph, branch, PR, diff stats, model); ~15 threads per screen, "highest
   information per pixel."

Locked decisions shared by all five: recency sorting, always-visible status
text, surfaced errors, visible wait-time for blocked threads, and "nothing
actionable is ever collapsed" / active work never hidden behind "show more."
The stated likely ship is a hybrid: "concept 4's adaptive density + concept
3's needs-you pinning + concept 1's meta row." [public]

Three observations for OpenAgents:

1. **Independent convergence on the attention model.** "Needs you" pinning
   with inline approval actions, recency-first ordering with no age ceiling
   in sight, explicit blocked-wait-time, and never-collapse-actionable are,
   nearly clause for clause, the episode 248/249 product calibration this
   repository already encodes (metadata-first startup, recent-first
   loss-accounted disclosure, causal inline activity, one action path). A
   competitor's design process reaching the same rules from its own live
   data strengthens those contracts as market truth rather than house taste.
   The delta remains authority: T3's inline Approve button dispatches a
   command string; OpenAgents' equivalent must dispatch a typed intent whose
   approval is policy-checked and receipted. [inferred]
2. **The factory designs itself.** The rendered live data visibly contains a
   running thread named "Sidebar v2 UX Redesign" building these very mocks —
   the sidebar redesign is being executed as a T3 Code agent thread inside
   T3 Code, and the artifact ships with machine-readable provenance of what
   is real versus mocked. This extends the §14 finding: not just
   agent-authored commits, but agent-run product design grounded in the
   product's own state store. [public] [inferred]
3. **Design-from-live-projections is worth adopting; the disclosure rule is
   the transferable part.** Rendering design concepts from real projection
   rows — with synthetic states explicitly labeled — is a cheap honesty
   convention OpenAgents design lanes (Desktop workbench, fleet cockpit,
   mobile supervision) should copy outright: every mock states which data is
   live, which is mocked, and from which store it came. [inferred]

`[limitation]` The document carries no explicit author attribution and is
hosted on an ephemeral-looking artifact domain; it is treated here as a
design-evidence snapshot fetched 2026-07-13, not as a committed T3 Code
roadmap.

## Primary source map

All paths relative to the pinned clone at `projects/repos/t3code`.

| Concern | Primary evidence |
| --- | --- |
| Product identity, installs | `README.md`; `AGENTS.md`; `apps/server/package.json` |
| Architecture overview | `docs/architecture/overview.md`; `docs/architecture/providers.md` |
| Provider drivers/adapters | `apps/server/src/provider/Drivers/`; `apps/server/src/provider/Layers/` |
| Codex protocol client | `packages/effect-codex-app-server/src/client.ts` + `_generated/` |
| ACP implementation | `packages/effect-acp/src/{agent,client,protocol,terminal}.ts` |
| Orchestration contracts | `packages/contracts/src/orchestration.ts`, `providerRuntime.ts`, `provider.ts`, `rpc.ts`, `git.ts` |
| Engine pipeline | `apps/server/src/orchestration/` (`decider.ts`, `projector.ts`, `commandInvariants.ts`) |
| Persistence | `apps/server/src/persistence/Layers/Sqlite.ts`; `Migrations/001…032`; `Services/Projection*.ts` |
| Runtime modes / approvals | `docs/architecture/runtime-modes.md`; `packages/contracts/src/orchestration.ts` |
| Checkpoints | `apps/server/src/checkpointing/CheckpointStore.ts`; `CheckpointDiffQuery.ts` |
| Remote model | `docs/architecture/remote.md`; `docs/architecture/connection-runtime.md`; `packages/client-runtime/` |
| Environment auth | `docs/cloud/environment-auth.md`; `apps/server/src/auth/dpop.ts` |
| Relay / T3 Connect | `infra/relay/`; `docs/cloud/t3-connect-clerk.md`; `apps/server/src/cloud/ManagedEndpointRuntime.ts` |
| Desktop security | `apps/desktop/src/window/DesktopWindow.ts`; `apps/desktop/src/preview/WebviewPreferences.ts` |
| Mobile | `apps/mobile/app.config.ts`; `apps/mobile/modules/`; `apps/mobile/src/widgets/AgentActivity.tsx` |
| MCP server | `apps/server/src/mcp/McpHttpServer.ts`; `apps/server/src/mcp/toolkits/preview/tools.ts` |
| Effect patch | `patches/effect@4.0.0-beta.78.patch`; `pnpm-workspace.yaml` |
| Lint enforcement | `oxlint-plugin-t3code/rules/` |
| Release engineering | `.github/workflows/{ci,release,pr-vouch}.yml`; `docs/operations/release.md`; `scripts/` |

Public sources: [T3 Code repository](https://github.com/pingdotgg/t3code),
[t3.codes](https://t3.codes/), [T3 Code docs](https://pingdotgg-t3code.mintlify.app/),
[Better Stack guide](https://betterstack.com/community/guides/ai/t3-code/),
launch/community coverage (BestofAI, daily.dev, FOSSHUNTER, addROM) fetched
2026-07-13. Public claims are bounded by those sources as of that date.
