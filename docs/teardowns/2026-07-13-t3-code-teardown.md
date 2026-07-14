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
core** persisted in SQLite. The React/Vite web renderer is also the Electron
desktop renderer; Expo mobile is a separate React Native implementation. All
three surfaces share the hand-written Effect RPC contract and client runtime,
but web and mobile do not share UI components or theme tokens. [source]

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

### Adapt as a program: the Vite Plus toolchain contract

T3 Code runs its entire monorepo on Voidzero's **Vite Plus** (`vp`) — the
unified vite/vitest/oxlint/oxfmt toolchain — and the way it is wired is more
interesting than the tool choice itself. The facts [source]:

- **One binary, one verb set.** Every package script is `vp run` / `vp test`
  / `vp lint` / `vp fmt` / `vp pack`; install is `vp i`; the recursive runner
  is `vpr`. The pnpm catalog aliases `vite` itself to
  `npm:@voidzero-dev/vite-plus-core@0.2.2`, and `packageExtensions` inject
  `vite-plus` into `@effect/vitest` while making upstream `vitest` optional —
  the test framework runs on the unified toolchain without forking it.
  Contributor bootstrap is one line (`curl -fsSL https://vite.plus | bash`).
- **One root config owns the whole repo.** A single root `vite.config.ts`
  configures tests (environment, excludes, timeouts), the formatter
  (ignore patterns, package.json sorting, per-file overrides), the linter
  (plugin sets, category severities, rule table), and the staged hook — no
  per-package lint/fmt/test config drift across 15+ packages.
- **The same verb is the human command, the CI command, and the agent
  gate.** `.github/workflows/ci.yml` runs `voidzero-dev/setup-vp@v1` →
  `vp check` → `vpr typecheck`; `AGENTS.md` line one tells every coding
  agent "`vp check` and `vp run typecheck` must pass before considering
  tasks completed"; a contributor types the identical commands. There is
  exactly one definition of "green."
- **Architecture invariants are lint rules, not prose.** The custom
  `oxlint-plugin-t3code` is loaded as a `jsPlugins` entry in the root
  config and enforces Effect/architecture law mechanically:
  `no-manual-effect-runtime-in-tests` (error), `no-global-process-runtime`
  (error), `namespace-node-imports` (error), `no-inline-schema-compile`
  (warn), plus `eslint/no-restricted-imports` making
  `@t3tools/client-runtime` subpath-only. Agents cannot merge a violation
  because the same `vp check` that gates their task completion runs the
  plugin.
- **The commit/push gradient is deliberate.** The pre-commit hook is two
  words (`vp staged`) and the staged config runs the formatter only —
  commits stay instant; correctness gating lives in `vp check` at CI and at
  the agent task boundary.
- **The boundaries are honest.** Type-aware linting is explicitly off with a
  comment ("revisit once Oxlint's tsgolint path can integrate with
  `@effect/tsgo` diagnostics"), and generated/vendored trees (`.repos/`,
  `routeTree.gen.ts`, native mobile dirs) are ignored rather than
  half-linted.

`[inferred]` The insight is not "use Vite." It is that a monorepo built and
maintained by coding agents needs **one machine-checkable definition of
done**, cheap enough to run on every task, with the project's architectural
laws compiled into it. T3's velocity (§14) is downstream of this: an agent
that ships hundreds of PRs a month cannot be governed by a style guide, only
by a gate.

How OpenAgents should mimic, in order:

1. **Unify the verb set first (no new tools required).** Define one root
   `check` (and `test`, `lint`, `fmt`, `typecheck`) whose meaning is
   identical for a human, CI, and the agent-contract gate in
   `CLAUDE.md`/`AGENTS.md` — collapsing today's per-app script spread
   (`check:deploy`, per-package sweeps) behind one entrypoint on the
   existing Bun toolchain. The T3 rule to copy verbatim: the agent
   task-completion gate names exactly the commands CI runs, and nothing
   else.
2. **Build `oxlint-plugin-openagents`.** Oxlint and its JS-plugin API run
   standalone (no vp dependency). Encode the workspace laws that currently
   live in prose and review memory as error-level rules in the standard
   sweep: no ad hoc keyword/string routing for intent or tool selection, no
   `Effect.runPromise`/manual runtimes outside named perimeter modules, no
   inline schema compilation in hot paths, schema-only packages stay
   runtime-free, subpath-only packages reject root imports, no renderer
   import of runtime credentials or provider SDKs. Each rule cites the
   owning invariant. This is the highest-leverage single item: it converts
   INVARIANTS.md from documentation into enforcement at agent speed.
3. **Adopt the staged gradient.** Format-only on commit (fast, unskippable),
   full `check` at push/CI/agent-completion — the current heavier pre-push
   guard keeps its role while commits stop paying the latency.
4. **Evaluate `@effect/tsgo` in a bounded lane.** T3 typechecks ~531k lines
   of deep Effect code with the native-preview compiler; as an Effect shop
   with growing typecheck times, OpenAgents should pilot it on one package
   with a drift comparison against `tsc` before any wide cutover, and
   inherit T3's honesty about the type-aware-lint gap.
5. **Pilot `vp` itself only where Vite already is.** The OpenAgents monorepo
   is Bun-first; `vp` is pnpm/Node-shaped and 0.2.x. Wholesale adoption
   would trade a working toolchain for a pre-1.0 one. The right experiment
   is one Vite-built surface (web or desktop renderer build) behind the
   unified verb set, judged on speed and config deletion — while the
   *contract* (one verb, one config, laws-as-lint, agent gate) is adopted
   everywhere immediately, tool-independently.

This plan is now tracked as GitHub issues: epic
[#8777](https://github.com/OpenAgentsInc/openagents/issues/8777), with the
five steps above filed as ordered leaves
[#8772](https://github.com/OpenAgentsInc/openagents/issues/8772) (TC-1,
unified root verb set),
[#8773](https://github.com/OpenAgentsInc/openagents/issues/8773) (TC-2,
`oxlint-plugin-openagents`),
[#8774](https://github.com/OpenAgentsInc/openagents/issues/8774) (TC-3,
fmt-on-commit staged gradient),
[#8775](https://github.com/OpenAgentsInc/openagents/issues/8775) (TC-4,
`@effect/tsgo` pilot), and
[#8776](https://github.com/OpenAgentsInc/openagents/issues/8776) (TC-5,
bounded `vp` pilot on `apps/aiur`).

What not to copy: aliasing the ecosystem's core package to a fork
(`vite` → `vite-plus-core`) and `packageExtensions` rewiring of a test
framework are clever but couple the whole repo to one vendor's pre-1.0
release train — T3 already stacks that bet on top of a patched Effect beta
and a preview compiler. OpenAgents takes the contract, not the coupling.

**Owner disposition, 2026-07-14 — the last paragraph is superseded.** After
the bounded TC-5 result and a source-level audit of T3's migration history and
Vite Plus itself, OpenAgents selected the full integrated topology, including
the exact Vite Plus core alias and test-framework wiring when required by the
chosen versions. TC-5 tested an intentionally additive one-app topology that
kept Bun and direct Vite/Vitest authority and prohibited T3's workspace
replacement; its footprint/config result does not test the system above.

The pattern to copy is T3's actual sequence: Node-native runtime first,
Effect TSGo second, one atomic pnpm/Vite Plus replacement third, then focused
stabilization. Root policy composes with host-specific configs; `vp run` owns
the graph; format-only staging stays cheap; Effect-aware typecheck remains a
separate gate; bundled engine versions are explicit provenance. OpenAgents'
Node-only destination then deletes the optional Bun adapters T3 retains. The
binding implementation and payment-removal contract is the
[`Sol full-conversion plan`](../sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md).

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

### Follow-on issue sequence (status ledger)

The toolchain lanes above are filed (#8772–#8777). Sequence status
(2026-07-13, late): ENV-1 is landed (#8778); ENV-2, GIT-1, SIG-1, FEED-1,
NPX-1, MAINT-1, and DMG-1 are opened (#8780–#8786) with implementation
dispatched in collision-aware order; EVT-1 remains a draft; LIVE-1 is
deprioritized to the end at owner direction.

1. **remote(ENV-1): adopt the ExecutionEnvironment/AccessEndpoint vocabulary
   in the portable-sessions pathway.** **Opened and implemented as
   [#8778](https://github.com/OpenAgentsInc/openagents/issues/8778)**
   (landed on main: the pathway doc now defines the four terms, an explicit
   access-versus-launch subsection, and the OpenAgents strengthenings).
   Fold T3's
   environment/endpoint/advertised-endpoint model — access and launch as
   separate concerns, endpoint providers (Tailscale first) as plugins
   outside the core model — into the portable-sessions pathway contracts as
   the canonical language for "where a session can run" versus "how to
   reach it". Owning surfaces:
   `docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md`-class
   contracts. T3 reference:
   `projects/repos/t3code/docs/architecture/remote.md`. Dependencies: none;
   vocabulary-first, unblocks ENV-2.
2. **auth(ENV-2): DPoP-bound, scope-limited capability tokens for local
   runtime sockets and Khala Sync device grants.** **Opened as [#8780](https://github.com/OpenAgentsInc/openagents/issues/8780); implementation dispatched.**
   Replace the
   password/env-token local-server pattern with per-client capability
   tokens carrying explicit scopes, RFC 8693-style token exchange, and DPoP
   proof-of-possession, so a leaked token is useless without the client
   key. Owning surfaces: Pylon socket exposure, Runtime Gateway, and Khala
   Sync device grants. T3 reference:
   `projects/repos/t3code/docs/cloud/environment-auth.md` and
   `apps/server/src/auth/dpop.ts`. Dependencies: ENV-1 for the endpoint
   vocabulary the scopes attach to.
3. **desktop(GIT-1): hidden-ref turn checkpoints.** **Opened as [#8781](https://github.com/OpenAgentsInc/openagents/issues/8781); implementation dispatched.**
   Capture workspace
   checkpoints as hidden Git refs at turn boundaries via a reactor, with
   typed revert through the event model and no user-visible commits — the
   cheap, honest middle ground between nothing and Claude Code's full
   file-history store. Owning surfaces: the desktop workbench D3 lane,
   combined with the existing update/rollback contracts. T3 reference:
   `projects/repos/t3code/apps/server/src/checkpointing/CheckpointStore.ts`.
   Dependencies: none.
4. **runtime(SIG-1): typed completion receipts for async pipelines as test
   oracles.** **Opened as [#8782](https://github.com/OpenAgentsInc/openagents/issues/8782); implementation dispatched.**
   Add a DrainableWorker/RuntimeReceiptBus-style seam so every
   async pipeline emits a typed milestone signal tests can await instead of
   polling — the deterministic-verification ergonomics OpenAgents' oracles
   need at the runtime seam. Owning surfaces: Runtime Gateway and the
   khala-tools dispatcher. T3 reference:
   `packages/shared/src/DrainableWorker.ts` and
   `docs/architecture/overview.md` in the pinned clone. Dependencies: none;
   strengthens the oracles the later lanes rely on.
5. **mcp(FEED-1): serve owned capabilities to wrapped harnesses over
   MCP.** **Opened as [#8783](https://github.com/OpenAgentsInc/openagents/issues/8783); implementation dispatched.**
   Build an OpenAgents MCP server that hands receipts, policy
   queries, fleet context, and preview tools to the Codex/Claude sessions
   we supervise, through the harnesses' native MCP support rather than
   forks, gated by provider-scoped bearer credentials. T3 reference:
   `projects/repos/t3code/apps/server/src/mcp/McpHttpServer.ts`.
   Dependencies: ENV-2 for the credential shape.
6. **onboarding(NPX-1): zero-install front door — one npx-shaped command
   boots the local runtime, migrates its store, prints a pairing URL with a
   fragment token.** **Opened as [#8784](https://github.com/OpenAgentsInc/openagents/issues/8784); implementation dispatched.**
   Give the local-first tier the entry point the night
   addendum records: fully usable before any account exists, pairing gate
   on by default, auth staying an upgrade rather than a gate. Owning
   surfaces: the khala CLI / Pylon bootstrap. T3 reference: the observed
   `npx t3@latest` flow (night addendum). Dependencies: ENV-2 recommended
   so the pairing token is a scoped capability from day one.
7. **fleet(MAINT-1): one-click provider install/update with ledger pinning
   and provenance receipts.** **Opened as [#8785](https://github.com/OpenAgentsInc/openagents/issues/8785); implementation dispatched.**
   Make harness install/update a typed
   per-harness maintenance action (detect installed version, resolve
   channel, execute update, re-probe capability) surfaced as one click in
   Desktop Settings, with the two additions T3 does not show: version
   pinning against the component ledger, and provenance verification plus a
   receipt for the binary just swapped under the fleet. T3 reference: the
   driver maintenance resolvers in `apps/server/src/provider/Drivers/`.
   Dependencies: the component ledger; SIG-1-style receipts.
8. **release(DMG-1): Gatekeeper release oracles — notarize+staple the DMG,
   fail closed.** **Opened as [#8786](https://github.com/OpenAgentsInc/openagents/issues/8786); implementation dispatched.**
   Mechanize the night-addendum rule: notarize the DMG
   (covering the nested app), staple the ticket to both artifacts, and gate
   publish on `codesign --verify --deep --strict` (app), `spctl -a -t open`
   on the image, `spctl -a -t exec` on the app, and `xcrun stapler
   validate` on both — refusing to publish when identity or notary
   credentials are absent. Also fold in the post-update
   launch-receipt/rollback lesson from
   `docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md`. Owning
   surfaces: `apps/openagents-desktop/forge.config.ts`,
   `scripts/release-preflight.ts`, `scripts/publish-release.ts` (CUT-26
   lane). Dependencies: none; the T3 DMG failure is live evidence of the
   cost.
9. **protocol(EVT-1): version-negotiation audit of the provider-runtime
    event vocabulary.** Compare the OpenAgents harness-adapter event union
    against T3's versioned `ProviderRuntimeEventV2`
    (`packages/contracts/src/providerRuntime.ts`) and add explicit
    version/compat fixtures wherever ours is implicit — keeping ours
    generated and version-negotiated where theirs is hand-written. Owning
    surfaces: the harness-adapter seam and its contract fixtures.
    Dependencies: none.
10. **mobile(LIVE-1): lock-screen agent presence (Live Activities) for fleet
   status.** **Deprioritized to the end of the sequence (owner direction
  2026-07-13: not needed for a while); remains a draft.**
   Ship running-agent presence to the lock screen as typed status
   projections only — never completion authority — following the second
   incumbent signal (after Cursor Remote Control) that ambient mobile
   supervision is a differentiating surface. Owning surfaces:
   `apps/openagents-mobile` over Khala Sync projections. T3 reference:
   `apps/mobile/src/widgets/AgentActivity.tsx` plus the relay APNs
   pipeline. Dependencies: Khala Sync projection classes.

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

## Addendum (2026-07-13, night): installed-artifact verification and two flows to copy

A first-run pass of the released product on this Mac (macOS 26.4) adds
`[runtime]`-class evidence in three directions: one distribution failure the
source audit had only predicted, and two product flows worth adopting.

### The macOS download is Gatekeeper-dead on arrival — and the app inside is innocent

Downloading `T3-Code-0.0.28-arm64.dmg` from t3.codes (a GitHub release
asset) and double-clicking it produces the macOS dialog:

> "T3-Code-0.0.28-arm64.dmg" is damaged and can’t be opened. You should
> move it to the Trash.

Artifact inspection of that exact download [runtime]:

- the DMG container is **completely unsigned**: `codesign` reports "code
  object is not signed at all" and `spctl -a -t open --context
  context:primary-signature` rejects it with "no usable signature"; the
  Chrome quarantine attribute is present;
- the app **inside** the image is fully correct: `Developer ID Application:
  T3 Tools, Inc. (ARK85ZXQ4Z)`, hardened runtime, `codesign --verify --deep
  --strict` passes ("valid on disk … satisfies its Designated
  Requirement"), and `xcrun stapler validate` confirms a **stapled
  notarization ticket**.

Diagnosis [inferred]: the release pipeline signs and notarizes the `.app`
during packaging but ships the DMG container unsigned and un-notarized —
electron-builder’s `dmg.sign` defaults to false, and nothing submits or
staples the image. On current macOS, Gatekeeper assesses the quarantined
disk image itself at open time, and an unsigned, un-notarized DMG fails with
the misleading "damaged" wording — so the correctly notarized app inside is
unreachable through the normal double-click path. Users who know the
`xattr -d com.apple.quarantine` folklore get in; everyone else trashes the
download, exactly as the dialog instructs.

This upgrades the §13/§15 release-engineering finding from a `[docs]` risk
("unsigned artifacts still release") to a `[runtime]`-confirmed distribution
failure with a sharper lesson: **notarizing the app is not shipping a
notarized product — the outermost quarantined artifact is what Gatekeeper
judges.** A pipeline can be 95% correct on signing and still deliver a 100%
broken first-run experience.

Deployment consequence for OpenAgents (CUT-26/#8706 lane): the current
`apps/openagents-desktop/forge.config.ts` is already ahead of T3’s default —
it signs the DMG through the maker’s `code-sign` option and notarizes the
app when credentials are present [source] — but three gaps would leave
OpenAgents exposed to a cousin of the same failure:

1. signing and notarization are env-conditional (`OA_DEVELOPER_ID_APPLICATION`,
   `ASC_API_*`), so a build without them still produces an artifact instead
   of failing closed;
2. the DMG is signed but not itself notarized/stapled — on macOS 15+ a
   signed-but-unnotarized image still draws a Gatekeeper block, just with
   different wording; and
3. the publish lane (`publish-release.ts`, `release-preflight.ts`) enforces
   manifest signing and version monotonicity but has no Gatekeeper oracle on
   the artifact.

The rule to encode as release oracles, not checklist prose: **notarize the
DMG (which covers the nested app), staple the ticket to both the `.dmg` and
the `.app`, then gate publish on `codesign --verify --deep --strict` (app),
`spctl -a -t open --context context:primary-signature` (image),
`spctl -a -t exec` (app), and `xcrun stapler validate` (both) — and refuse to
publish when the identity or notary credentials are absent.** This is the
concrete mechanization of Adapt-with-stronger-boundaries item 5 ("unsigned
release fallbacks") and joins the Cursor teardown’s plain-HTTP-update-URL
finding as artifact-level evidence for the signed component ledger.

### `npx t3@latest` is the onboarding bar

Running `npx t3@latest` on a machine that had never installed T3 Code
produced, in one command and a few seconds [runtime]:

1. package fetch, then `Running all migrations...` (the SQLite event
   store/projections initialize in place — migration `32` at this version,
   matching the source audit);
2. `Listening on http://127.0.0.1:3773`;
3. a provider session reaper and agent-activity standby starting; and
4. `Authentication required. Open T3 Code using the pairing URL.` with
   `http://localhost:3773/pair#token=...` — a short token in the URL
   fragment, per the pairing design in §6.

Zero install, zero account, zero configuration, and the security-relevant
pairing gate is on by default rather than an open localhost port. The gap
between this and the broken DMG above is instructive: their npm path is
excellent while their macOS artifact path is broken, and most users will
judge the product by whichever door they happen to try first.

**Adapt:** the owner wants this exact gradient for OpenAgents — one
`npx`-shaped command that boots the local runtime, migrates its store,
prints a pairing URL with a fragment token, and is fully usable before any
account exists. The `khala` CLI onboarding
(`npm install -g @openagentsinc/khala` → `khala fleet connect`) is close but
still install-first and fleet-scoped; the local-first tier recorded in the
adaptation analysis (auth as an upgrade, not a gate) should get this
zero-install front door as its entry point. [runtime] [inferred]

### One-click provider maintenance is the harness-fleet flow to steal

On first open, the UI surfaced the connected harnesses with an
install/update affordance, and one click updated both the local Codex CLI
and OpenCode — no terminal, no docs page, no version hunting. The source
audit shows the mechanism: each provider driver owns a maintenance resolver
(e.g. the Claude driver’s package-managed resolver knows both the npm
`@anthropic-ai/claude-code` path and the native `claude update` installer)
so install/update is a typed per-harness capability, not a support FAQ.
[runtime] [source]

**Adapt:** Pylon/Desktop should own harness install/update lifecycle the
same way — a typed maintenance action per provider (detect installed
version, resolve channel, execute update, re-probe capability) surfaced as
one click in Settings — with the two additions T3 does not show: version
pinning against the component ledger, and provenance verification plus a
receipt for the binary that was just swapped under the user’s agent fleet.
An unverified auto-updater for the tools that hold `danger-full-access` is
also a supply-chain lesson in the other direction; adopt the ergonomics,
keep the ledger. [inferred]

## Addendum (2026-07-14): frontend implementation deep dive

The initial audit treated the three clients mostly as projections of the
server architecture. A second source pass traced the actual render trees,
state owners, cache layers, component libraries, styling systems, responsive
rules, and hot-path performance work at the pinned commit. The main correction
is that T3 does **not** have three independent frontends: Electron hosts the
same DOM renderer as web, while mobile is a second React Native renderer over
the shared contracts and client runtime. [source]

```text
browser                                  Electron main process
  |                                             |
  | browser history                     hash history + preload bridge
  |                                             |
  +---------- React/Vite web renderer ----------+
             Tailwind DOM components
             Base UI / Lexical / xterm / Pierre
                         |
                Effect AtomRegistry
                         |
              packages/client-runtime
                         |
              Effect RPC / snapshots / deltas
                         |
                      T3 server
                         |
              packages/client-runtime
                         |
                Effect AtomRegistry
                         |
                Expo / React Native
             Uniwind + native modules
                     mobile
```

The shared boundary is substantial but precise: contracts, connection
supervision, snapshots, deltas, commands, and domain query state are shared;
component trees, navigation, theming, local interaction state, and platform
hosts are not. [source] [inferred]

### Render and navigation topology

#### Web and Electron: one renderer, two histories

`apps/web/src/main.tsx` imports DM Sans, JetBrains Mono, xterm's stylesheet,
and the global Tailwind stylesheet before creating a React 19 root. It selects
TanStack Router hash history inside Electron and browser history on the hosted
web surface, then conditionally wraps the application in Clerk authentication.
Before that root exists, `index.html` applies the stored/system light or dark
class and paints a theme-aware splash, avoiding a light-theme flash.
`AppRoot.tsx` creates one Effect `AtomRegistry` for the renderer and mounts the
router, preview-automation hosts, and `ElectronBrowserHost`. The latter sits
outside route content deliberately, so embedded browser webviews survive route
transitions while still using the same atom registry. [source]

The generated TanStack route tree is file-based. `routes/__root.tsx` performs
pairing, hosted-auth, and environment gates before presenting the main shell:
command palette, `AppSidebarLayout`, route outlet, toasts, onboarding, SSH and
update coordinators, plus the server-event router. The thread route reads the
environment shell, thread detail, and draft state, delays stale-route repair
until bootstrap is settled, and mounts `ChatView` inside the sidebar inset.
This keeps URL identity explicit without making route state the conversation
authority. [source]

`AppSidebarLayout.tsx` is a resizable off-canvas shell with persisted width,
window-control insets for Electron, a 13 rem sidebar minimum, and a 40 rem
minimum main pane. The right-side file/diff/terminal/preview surfaces choose an
inline panel up to 980 px or a responsive sheet up to 760 px. The renderer is
therefore a workbench with route-addressable conversation at the center, not a
collection of independent pages. [source]

Electron does not install a second React tree. `apps/desktop` loads the web
renderer and contributes privileged capabilities through an explicit preload
bridge: settings, connection catalog, SSH, updater, preview, window, and secure
storage operations. The main window is 1100×780 with an 840×620 minimum;
`contextIsolation`, sandboxing, and disabled Node integration remain the
default. Preview webviews are separately preference-gated. This is the right
privilege shape: desktop-specific authority stays in main/preload instead of
entering ordinary renderer modules. [source]

#### Mobile: shared domain runtime, separate UI product

`apps/mobile/src/App.tsx` builds a different tree: Effect `AtomRegistry`, Clerk
cloud auth, native appearance, gesture and keyboard providers, safe-area
context, native navigation theme, blur targets, and portal hosts. `Stack.tsx`
uses React Navigation's static native stacks and iOS-specific glass/solid
header presets; flat thread routes preserve native shared-header transitions.
There is no reuse of the web DOM component library. [source]

The mobile shell is adaptive rather than device-name-based. `lib/layout.ts`
derives split mode from available space (720×600 threshold), constrains the
sidebar to 280–460 px, and suppresses side panes when they would crush the
conversation. `AdaptiveWorkspaceLayout.tsx` animates persisted sidebar and
inspector widths with Reanimated, while focus-scoped portals avoid rendering
through screens frozen by `react-native-screens`. This is stronger than a
simple “tablet breakpoint” implementation. [source]

Mobile is also not a thin WebView. Five local Expo modules provide native
composer, Markdown, review diff, terminal, and controls implementations.
`ThreadFeed.tsx` uses `KeyboardAwareLegendList`, patched scroll anchoring and
inset behavior, and native diff rendering. The price is two implementations of
the most complex interaction surfaces, not merely two platform adapters.
[source] [inferred]

### State ownership: five layers with mostly clear authority

The frontend uses several state technologies, but they are not interchangeable.
At the pinned commit their effective ownership is:

| Layer | Technology | Owns | Does not own |
| --- | --- | --- | --- |
| Durable product truth | Server SQLite event store and projections | projects, threads, turns, commands, plans, activities, provider state | renderer layout or draft presentation |
| Connection and synchronized domain state | Effect services, `packages/client-runtime`, `@effect/atom-react` | environment sessions, connection generation, shell/thread snapshots and deltas, queries, typed commands | durable acceptance or final product authority |
| Renderer persistence | IndexedDB on web; SQLite and secure storage on mobile | connection catalog, cached shells/threads, server config, VCS refs, schema-versioned recovery data | canonical server facts |
| Local workbench state | Zustand plus local storage | drafts, right-panel tabs, selection, terminal/diff/preview preferences, pane layout | server-owned thread or run lifecycle |
| Interaction and address state | React state/refs and TanStack/React Navigation | open controls, focus, cursor, transient gestures, current route | domain truth |

All [source], with the authority interpretation [inferred]. This separation is
the strongest part of the frontend architecture. The server remains canonical;
client persistence accelerates startup and reconnect; Zustand is generally
kept to renderer-local concerns.

#### Shared Effect client runtime

`apps/web/src/rpc/atomRegistry.ts` and
`apps/mobile/src/state/atom-registry.ts` each provide one unstable Effect
reactivity `AtomRegistry`. Entity hooks select environment, project, thread,
message, activity, plan, and session atoms. Query hooks convert `AsyncResult`
into the familiar data/error/pending/refresh shape, while command hooks run
typed `AtomCommand`s through the registry. [source]

The deeper behavior lives in `packages/client-runtime/src/state/`:

- **Queries are environment-scoped atom families.** A connection generation
  change restarts them; stale-while-revalidate defaults to a 30-second stale
  window, atoms receive an idle TTL, and callers can opt into refresh and
  subscription behavior. [source]
- **Commands declare scheduling semantics.** The runtime supports parallel,
  serial, single-flight, latest-only, and keyed execution; defects become
  `AsyncResult` failures instead of escaping into components. [source]
- **The environment shell is cache-first.** A valid cached shell paints first,
  then a cold path obtains a gzip-friendly HTTP snapshot and a live WebSocket
  subscription resumes after its sequence. A warm cache skips the network
  snapshot and resumes directly from its stored sequence. [source]
- **Thread state follows the same snapshot-plus-delta protocol.** Sequence
  deduplication protects projections, reconnect resumes after the last cached
  sequence, and deletion/cached/live states are explicit. [source]
- **Persistence is a platform contract.** `platform/persistence.ts` defines
  typed Effect services for connection targets and schema-versioned shell,
  thread, server-config, and VCS-ref caches. Web and mobile supply different
  implementations without changing the synchronization algorithm. [source]
- **Writes are deliberately coalesced.** Shell persistence uses a sliding queue
  of one plus a 500 ms debounce, preventing every delta from becoming storage
  traffic. [source]

This is more than “shared networking.” It is a reusable client projection
kernel: cache lifecycle, freshness, reconnection, query identity, command
concurrency, and platform persistence are part of one typed contract.
[inferred]

#### Web persistence and Zustand

Web persistence uses IndexedDB database `t3code:connection-runtime`, version 4,
with stores for the connection catalog, shell, thread, server configuration,
and VCS refs. Records are decoded through versioned Effect Schemas. Corrupt
catalog entries are quarantined instead of trusted, and catalog mutations are
serialized with an Effect semaphore. Electron may replace the browser catalog
with secure storage exposed by the desktop bridge. [source]

Zustand fills the intentionally local gap, but at considerable scale.
`composerDraftStore.ts` is 3,563 lines and persists a versioned, migrated,
partialized representation of Lexical content and attachment/context tokens.
`rightPanelStore.ts` persists per-thread file, diff, terminal, and preview
surfaces. Other stores cover thread selection, terminal UI, preview state, and
general UI migration. This avoids contaminating server projections with local
ergonomics, yet leaves contributors reasoning across Effect atoms, Zustand,
React state, router state, IndexedDB, and local storage in the same feature.
[source] [inferred]

Mobile implements the same cache contract over Expo SQLite
(`t3code-client.db`) with WAL and foreign keys, plus secure storage for
sensitive connection data. Legacy cache records are migrated; schema-invalid
records are logged and deleted. That is a stronger mobile restart story than
relying on React Native async key/value storage alone. [source]

The source also exposes seven weak seams to avoid as this architecture grows:

1. `useAtomCommand` returns a settled promise but no shared reactive
   pending/error atom. `ChatView` and settings therefore build local busy or
   optimistic state, and the server-settings patch path can discard the
   command result. Command acknowledgement remains convention rather than one
   reusable presentation contract. [source]
2. Freshness is inconsistent. Shell and thread projections expose explicit
   cached/synchronizing/live/error phases, while VCS refresh failure keeps the
   old value and only logs. User-visible caches need one envelope containing
   source, update time, freshness, and error. [source] [inferred]
3. Desktop encrypts the connection catalog with Electron `safeStorage`, but
   hosted web stores the catalog—including credentials and remote DPoP token
   material—as schema-validated JSON in IndexedDB. This makes the browser XSS
   and at-rest threat assumptions load-bearing. [source]
4. Persisted Zustand stores and the module-level settings store do not broadly
   reconcile native `storage` changes. The singleton Electron window mitigates
   this today; a future multi-window renderer would need main/server ownership
   or versioned BroadcastChannel/IPC synchronization. [source] [inferred]
5. Desktop topology changes are polled every three seconds because the preload
   bridge has no topology event. A host-pushed event with polling only as
   recovery would reduce latency and background work. [source] [inferred]
6. Generic query identity uses raw `JSON.stringify` and mutation invalidation
   is feature-specific. Schema-canonical keys plus a typed dependency/
   invalidation map would scale more safely. [source] [inferred]
7. `docs/architecture/overview.md` still names deleted `wsTransport.ts` and
   `wsNativeApi.ts` paths. The implemented connection authority is now
   `EnvironmentRegistry`/`EnvironmentSupervisor` plus Effect RPC. For this
   frontend pass, source supersedes that stale overview. [docs] [source]

### Component and rendering stack

The frontend library choices reveal which work T3 considers product-defining:

| Concern | Web / Electron | Mobile |
| --- | --- | --- |
| View/runtime | React 19.2, React DOM, React Compiler | React 19.2, React Native 0.85, Expo 56 |
| Navigation | TanStack Router, generated file routes | React Navigation static native stack |
| Domain state | Effect 4 + `@effect/atom-react` + shared client runtime | same |
| Local state | Zustand, local storage | React state, native persistence, feature stores |
| Primitive UI | Base UI React plus local shadcn/COSS-style adapters, Lucide | React Native, Expo UI, `@callstack/liquid-glass`, local native controls |
| Composer | Lexical with custom decorator nodes | local native composer module |
| Long feeds | Legend List | Legend List / KeyboardAwareLegendList |
| Markdown/code | `react-markdown`, remark GFM/breaks, rehype raw+sanitize, Shiki | local native Markdown plus Shiki paths |
| Diff/tree | `@pierre/diffs`, `@pierre/trees` | local native review diff plus Pierre paths |
| Terminal | xterm 6 | local native terminal module |
| Motion/interaction | dnd-kit, AutoAnimate, TanStack Pacer | Reanimated, Gesture Handler, native screens/keyboard |
| Styling | Tailwind CSS 4, CSS variables, one global stylesheet | Uniwind/Tailwind-class bridge plus native dynamic colors |

All [source]. `apps/web/components.json` confirms the shadcn-compatible
primitive setup: `base-mira` style, Zinc base, CSS variables, Lucide, no React
Server Components, and COSS/spell registries. The 42 primitives under
`components/ui/` are local adapters, mostly around Base UI, rather than a stock
component package dropped into feature code. [source]

#### Hot-path rendering work

The workbench contains serious performance engineering rather than naive chat
rendering:

- `MessagesTimeline.tsx` renders typed rows through Legend List with a stable
  item renderer, estimated heights, end anchoring, visible-content retention,
  and a minimap. Mobile independently implements the equivalent hard scroll
  problem around keyboard and native inset changes. [source]
- `ChatMarkdown.tsx` places Shiki/Pierre highlighting behind Suspense and an
  error boundary, caps its highlight cache at 500 entries / 50 MB, and runs raw
  HTML through a customized `rehype-sanitize` schema. [source]
- `DiffWorkerPoolProvider.tsx` sizes a Pierre worker pool to half the available
  logical processors, clamped from two to six, caps its AST LRU at 240 entries,
  and synchronizes the active theme. [source]
- Heavy diff, preview, and file panels are lazy-loaded. React Compiler,
  `memo`, stable callbacks, shallow Zustand selectors, and prewarming appear on
  the main conversation/sidebar paths. [source]
- The Lexical prompt editor uses plain-text/history/on-change plugins and
  custom decorator nodes for file mentions, skills, and terminal context,
  reconciling externally controlled draft state with selection position.
  [source]
- The xterm host uses incremental buffer appends, 5,000 lines of scrollback,
  fit/link addons, explicit disposal, and live theme synchronization. [source]
- The embedded Electron browser host is kept outside the route outlet, avoiding
  webview teardown and recreation during navigation. [source]

The counterweight is concentration. `ChatView.tsx` is 5,370 lines,
`Sidebar.tsx` 3,751, `MessagesTimeline.tsx` 2,057,
`ComposerPromptEditor.tsx` 1,697, and mobile `ThreadFeed.tsx` 1,796. React
Compiler and virtualization reduce runtime work; they do not reduce the human
state space or the probability that unrelated behaviors collide inside a
feature module. [source] [inferred]

Three bundle/network choices leave clear optimization work. The chat graph
statically imports the xterm drawer even when no terminal is open; the header's
“open in” menu pulls a large editor-icon source graph; and visible external
Markdown links request favicons from Google's favicon service, disclosing the
linked hostname to a third party. Diff, preview, and file chunks are correctly
lazy, but their Suspense fallbacks are `null`, so a cold chunk can present a
blank panel rather than progress feedback. [source]

### CSS, themes, and responsive behavior

The web renderer uses Tailwind CSS 4 through the Vite plugin, with 893 lines of
global CSS. `index.css` defines semantic CSS variables for light and dark
themes, maps them into Tailwind's `@theme inline` vocabulary, defines the DM
Sans/monospace typography, safe-area and Electron window-control variables,
focus defaults, Markdown/code/table rules, and the application-height/overflow
contract. Colors use OKLCH, Tailwind colors, and `color-mix`; radii and shadow
levels are centralized. [source]

`useTheme.ts` schema-validates `light | dark | system`, subscribes to OS theme
changes with `useSyncExternalStore`, toggles the root `.dark` class, updates the
browser theme-color, and informs the Electron shell. It briefly applies a
`no-transitions` class during theme changes. `useMediaQuery.ts` provides typed
breakpoints and coarse/fine-pointer queries. Safe-area utilities and
`svh`-based sizing make the browser surface usable on mobile and installed-app
viewports. [source]

Responsive behavior is split between container queries, Tailwind breakpoints,
off-canvas sidebars, right-panel geometry, safe-area variables, and
pointer-capability rules. That is more deliberate than width-only responsive
CSS. Mobile then carries a second independent token sheet (`apps/mobile/global.css`,
237 lines) with its own light/dark values, font scale, and utilities, plus
hard-coded native dynamic colors for some headers. [source]

Visually, the web defaults lean on translucent “glass” composer surfaces,
large soft shadows, blur, a data-URI SVG `feTurbulence` grain over the body,
and an “ultrathink” spectrum treatment with animated gradient border and
gradient-clipped text. Mobile intentionally follows iOS liquid-glass language.
These choices make the surfaces feel related, but no shared token package or
shared component contract guarantees that relationship. [source] [inferred]

### Source-only interface health audit

This score is a static implementation audit, not a runtime accessibility,
contrast, bundle, or device-lab certification. The pinned code was not served
or exercised in this pass. [limitation]

| Category | Score (0–4) | Evidence-based assessment |
| --- | ---: | --- |
| Accessibility | 2 | Base UI, semantic controls, labels/live regions, keyboard commands, focus-visible rules, sanitization, and native accessibility props provide a strong base, but core composer, drag, modal, terminal, resize, and motion paths retain material gaps. |
| Performance | 3 | Virtualized anchored feeds, capped caches, workers, React Compiler, memoization, cache-first projections, and webview lifetime control are strong; xterm/icon graph loading, blank lazy fallbacks, third-party favicons, and component concentration leave measurable work. |
| Responsive/adaptive | 3 | Container/media/pointer queries, safe areas, off-canvas geometry, sheets, and available-space mobile layout cover all major shapes; custom compact/resize controls bypass the reusable coarse-pointer policy. |
| Theming/system consistency | 3 | Each renderer has coherent semantic tokens and light/dark/system synchronization, but web and mobile duplicate token vocabularies and terminal/special-effect colors bypass the shared web roles. |
| Anti-patterns/maintainability | 2 | Credible workbench foundations are offset by global glass/grain/animated-gradient decoration, duplicated complex renderers, and very large feature/store modules. |
| **Total** | **13/20 — acceptable** | Strong runtime mechanics and adaptivity with significant accessibility, cross-renderer consistency, and decomposition work remaining. |

Prioritized findings:

1. **P1 — the composer is not persistently named and its placeholder is too
   faint.** Lexical `ContentEditable` receives `aria-placeholder` but no
   `aria-label` or `aria-labelledby`; the visible placeholder uses
   `text-muted-foreground/35` (approximately 1.6:1 from the declared theme
   tokens). Add a stable “Message” label and a contrast-checked placeholder
   role. [source]
2. **P1 — manual project ordering is pointer-only.** `Sidebar.tsx` registers
   only dnd-kit's `PointerSensor`; no keyboard sensor, sortable keyboard
   coordinates, or move-up/down actions provide an equivalent path. [source]
3. **P1 — expanded-image modal lacks a complete focus lifecycle.**
   `components/chat/ExpandedImageDialog.tsx` creates a custom
   `role="dialog"` overlay and global Escape/arrow handler, but does not trap
   focus, focus an initial control, restore focus, or inert the background.
   Replace it with the shared Base UI dialog primitive or reproduce that
   primitive's tested focus behavior. [source]
4. **P1 — reduced-motion coverage is incomplete.** Many component transitions
   use Tailwind `motion-reduce` variants and mobile workspace animation honors
   `ReduceMotion.System`, but the infinite `ultrathink` rainbow/chroma
   animations, provider countdown, dialog/toast motion, and status pulses have
   no complete global `prefers-reduced-motion` fallback or pause path. [source]
5. **P1 — terminal output is not configured for screen readers.** xterm is
   created without `screenReaderMode` or a session label, and mounts into an
   unlabeled `div`. Enable its accessibility mode, name each terminal, and test
   output announcements. [source]
6. **P2 — custom navigation and resize controls bypass shared semantics.** The
   primary sidebar roots are generic `div`s rather than a labelled navigation
   landmark. Its resize rail is removed from tab order and pointer-only;
   terminal resize handles and several 24–28 px action controls similarly skip
   keyboard and coarse-pointer behavior already encoded in `Button`. Use
   landmarks, separator/range semantics, keyboard increments, and 44 px coarse-
   pointer hit areas. [source]
7. **P2 — diff filename opening is pointer-only.** The Pierre title region is
   styled as clickable and handled through `onClickCapture`, but it is not a
   button/link or keyboard target. Render a semantic control with Enter/Space
   activation. [source]
8. **P2 — core-graph loading and remote favicon behavior are avoidable.**
   Lazy-load xterm and the editor icon catalog, give lazy panels visible loading
   states, and replace Google's Markdown favicon endpoint with a local icon,
   owned cache/proxy, or explicit privacy contract. [source]
9. **P2 — renderer design drift is structurally permitted.** Web and mobile
   share contracts and domain runtime but not tokens, primitives, navigation
   intents, or complex composer/diff/terminal hosts. Visual and accessibility
   parity therefore depend on duplicate review rather than one executable
   component contract. [source] [inferred]
10. **P2 — oversized feature modules and browser-test gaps increase regression
    radius.** The main conversation, sidebar, feed, and draft store combine
    rendering, projection, persistence, keyboard, layout, and provider-specific
    branches. Extract lifecycle-owned feature services and smaller
    render/presenter boundaries; retain the current pure helpers and colocated
    tests, and add browser/axe coverage for focus, keyboard, contrast, and
    motion on these surfaces. [source] [inferred]
11. **P3 — decoration competes with the otherwise quiet workbench.** Default
    glass, noise, animated spectrum borders, and gradient text add GPU work and
    make platform styles harder to normalize. Reserve them for a named semantic
    state, and provide a flat token fallback. [source] [inferred]

Positive patterns worth retaining are the shared Base UI adapters, explicit
keyboard command layer, route/bootstrap repair, coarse-pointer awareness,
safe-area handling, sanitized rich text, virtualized anchored feeds, bounded
highlight/diff caches, adaptive mobile geometry, and clear renderer/main-process
privilege separation. [source]

### What the frontend evidence changes for OpenAgents

#### Adapt directly

1. **Treat the client runtime as a projection kernel, not a fetch helper.**
   Environment-scoped query identity, cached-first shell/thread rendering,
   snapshot-plus-sequenced-delta repair, schema-versioned platform persistence,
   explicit freshness/idle policies, and command concurrency modes belong in
   the shared Effect Native application services. OpenAgents should add its
   stronger replay-to-live marker, acknowledgement, worker epoch, authority,
   and receipt rules at that seam. [inferred]
2. **Make long conversation feeds a named systems problem.** Retain T3's
   virtualization, stable typed row renderer, end anchoring, visible-content
   retention, capped rich-text caches, background diff work, and separate
   mobile keyboard/inset oracles. These are supervision primitives, not visual
   polish. [inferred]
3. **Keep desktop privilege outside the shared renderer.** T3's single web/
   Electron renderer plus explicit preload capabilities is the right sharing
   boundary. OpenAgents strengthens it with generated bridge contracts,
   capability grants, Effect lifecycles, and receipts rather than exposing a
   generic invoke surface. [inferred]
4. **Use platform-native hosts where the interaction justifies them.** Native
   terminal, diff, Markdown, composer, and iOS status surfaces are evidence for
   Effect Native foreign hosts—provided one typed host contract owns lifecycle,
   events, accessibility, and fallback behavior across renderers. [inferred]

#### Refuse the implementation split

1. **Do not copy the two-design-system topology.** OpenAgents' Effect Native
   mandate is one typed component and token set with swappable web, React
   Native, native, and canvas renderers. T3's shared client runtime is a model
   for the service layer; its independent Tailwind DOM and Uniwind/native
   component trees are evidence of the drift that mandate must prevent.
2. **Do not let Zustand become a second domain runtime.** A small renderer-
   local store is defensible for drafts and geometry, but server facts,
   connection state, command lifecycle, approvals, and receipts stay in typed
   Effect services/projections. State owners and persistence policy should be
   declared per feature.
3. **Do not make giant components the integration boundary.** `ChatView`- or
   `Sidebar`-scale modules should decompose into Effect-owned feature services,
   typed intents, foreign-host adapters, and small renderers before the second
   platform implementation appears.
4. **Do not encode product identity as glass/noise/gradient defaults.** The
   OpenAgents design system should remain semantic and token-driven, respect
   reduced motion and contrast at the renderer boundary, and reserve expensive
   decoration for an explicit state rather than the whole shell.

All four are [inferred] adaptations from the source. They do not replace the
Effect Native dossier, Sol roadmap, typed contracts, or executable guarantees.

#### React renderer reconciliation (2026-07-14)

“Refuse the implementation split” does **not** mean “refuse React.” T3's
React/Vite renderer, Base UI primitives, TanStack integration, virtualization,
Lexical, xterm, and diff tooling are valuable implementation evidence. The
split to refuse is React becoming a second application/component/state/token
authority beside Effect Native.

The source-grounded
[Effect Native + React web renderer gap analysis](../effect-native/2026-07-14-react-web-renderer-harmonization-gap-analysis.md)
therefore recommends the same layering that Effect Native already ships on
mobile: retain the Schema `View`, Effect state/services, typed intents, and
tokens; lower them to React elements inside the DOM renderer; and admit
specialist libraries only through renderer-private implementations or closed
typed hosts. Today's React route shell around an imperative direct-DOM island
is migration glue, not that destination. [inferred]

#### OpenAgents Desktop implementation disposition (2026-07-14)

The first Desktop convergence now applies that recommendation rather than
copying T3's whole client architecture:

| T3 layer | OpenAgents Desktop disposition | Reason |
| --- | --- | --- |
| React 19 / React DOM | adopted as the Effect Native renderer root and lifecycle host | the first surface retains the proven direct catalog lowering internally; React still cannot own the View grammar |
| Vite + React plugin | adopted for the renderer bundle | main/preload/workers remain on the existing host build and the signed asset names remain fixed |
| Tailwind CSS 4 | adopted as a renderer compiler with `--en-*` semantic aliases | no Tailwind class enters the portable View schema and no second theme is created |
| Base UI | deferred to reviewed renderer-private primitive adapters | do not add it until a catalog primitive genuinely delegates; never expose `ReactNode` or callbacks in core |
| Effect Atom React / Zustand | not adopted | `SubscriptionRef`, `ViewProgram`, typed intents, and existing persistence already have one owner |
| TanStack Router | not adopted | Desktop navigation is typed application state, not a browser URL hierarchy |
| Lexical / LegendList / Pierre / xterm | deferred until a catalog node or typed Host genuinely delegates to each library | dependencies must follow a real lifecycle-owned integration, not stack mimicry |
| DM Sans / JetBrains Mono / Lucide | rejected | Desktop's enforced system-font, khala-token, and closed Apps SDK icon contracts remain authoritative |

The Electron host still serves only `index.html`, `boot.js`, and `app.css`
under the existing restrictive CSP. The pure `shell.ts` application and its
workspace projections remain React-free; the renderer-host boundary test now
allows React/Base UI only in named host files and scans `.tsx` so the exception
cannot become an unreviewed second UI layer. Tailwind's semantic theme roles
map to canonical Effect Native variables rather than duplicating T3's color,
radius, typography, noise, glass, or gradient defaults. [source] [inferred]

This is the practical meaning of “adapt the implementation ecosystem, refuse
the implementation split”: React can host and progressively reconcile Effect
Native Views, but it cannot become an alternative product model. The initial
hybrid is the R1 bridge from the gap analysis, not a claim that the R2–R6
native React lowering, SSR, or specialist-library work is complete.

### Frontend evidence limitations

This addendum inspected source and dependency manifests at the pinned commit.
It did not run Lighthouse, axe, VoiceOver/TalkBack, keyboard-only task flows,
contrast calculation, bundle profiling, memory traces, or representative
device labs. CSS and component structure establish implementation intent, not
the performance or accessibility of a shipped binary. Findings tied to
explicit attributes, input sensors, xterm configuration, and CSS animation are
direct source defects; perceived visual quality and the severity of module-size
risk remain partly interpretive.
[limitation]

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
| Shared client projections | `packages/client-runtime/src/state/{runtime,shell,threads,threadReducer,threadDetail}.ts`; `packages/client-runtime/src/platform/persistence.ts` |
| Web boot / navigation | `apps/web/index.html`; `apps/web/src/main.tsx`; `apps/web/src/AppRoot.tsx`; `apps/web/src/router.ts`; `apps/web/src/routes/{__root,_chat.$environmentId.$threadId}.tsx`; `apps/web/src/components/AppSidebarLayout.tsx` |
| Web domain and local state | `apps/web/src/state/`; `apps/web/src/rpc/atomRegistry.ts`; `apps/web/src/composerDraftStore.ts`; `apps/web/src/rightPanelStore.ts`; `apps/web/src/uiStateStore.ts` |
| Web persistence | `apps/web/src/connection/storage.ts`; `apps/desktop/src/app/DesktopConnectionCatalogStore.ts`; `apps/desktop/src/electron/ElectronSafeStorage.ts` |
| Web components / styling | `apps/web/src/components/ui/`; `apps/web/components.json`; `apps/web/src/index.css`; `apps/web/src/hooks/useTheme.ts`; `apps/web/src/rightPanelLayout.ts` |
| Conversation rendering | `apps/web/src/components/{ChatView,ChatMarkdown,ComposerPromptEditor,DiffWorkerPoolProvider,ThreadTerminalDrawer}.tsx`; `apps/web/src/components/chat/MessagesTimeline.tsx` |
| Environment auth | `docs/cloud/environment-auth.md`; `apps/server/src/auth/dpop.ts` |
| Relay / T3 Connect | `infra/relay/`; `docs/cloud/t3-connect-clerk.md`; `apps/server/src/cloud/ManagedEndpointRuntime.ts` |
| Desktop security | `apps/desktop/src/window/DesktopWindow.ts`; `apps/desktop/src/preview/WebviewPreferences.ts` |
| Desktop renderer bridge | `apps/desktop/src/preload.ts`; `apps/desktop/src/ipc/DesktopIpc.ts`; `apps/web/src/browser/ElectronBrowserHost.tsx` |
| Mobile renderer / navigation | `apps/mobile/src/{App,Stack}.tsx`; `apps/mobile/src/features/layout/AdaptiveWorkspaceLayout.tsx`; `apps/mobile/src/features/threads/ThreadFeed.tsx` |
| Mobile state / styling | `apps/mobile/src/state/`; `apps/mobile/src/connection/runtime.ts`; `apps/mobile/src/persistence/mobile-database.ts`; `apps/mobile/global.css` |
| Mobile native hosts | `apps/mobile/app.config.ts`; `apps/mobile/modules/`; `apps/mobile/src/widgets/AgentActivity.tsx` |
| MCP server | `apps/server/src/mcp/McpHttpServer.ts`; `apps/server/src/mcp/toolkits/preview/tools.ts` |
| Effect patch | `patches/effect@4.0.0-beta.78.patch`; `pnpm-workspace.yaml` |
| Lint enforcement | `oxlint-plugin-t3code/rules/` |
| Release engineering | `.github/workflows/{ci,release,pr-vouch}.yml`; `docs/operations/release.md`; `scripts/` |

Public sources: [T3 Code repository](https://github.com/pingdotgg/t3code),
[t3.codes](https://t3.codes/), [T3 Code docs](https://pingdotgg-t3code.mintlify.app/),
[Better Stack guide](https://betterstack.com/community/guides/ai/t3-code/),
launch/community coverage (BestofAI, daily.dev, FOSSHUNTER, addROM) fetched
2026-07-13. Public claims are bounded by those sources as of that date.
