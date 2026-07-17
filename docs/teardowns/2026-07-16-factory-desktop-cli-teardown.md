# Factory Desktop and Droid CLI Teardown — 2026-07-16

Read-only audit of the installed Factory Desktop application, the installed
`droid` CLI, Factory's commit-pinned public repositories, and isolated CLI
behavior. No real Factory login, model request, synced session, remote
computer, MCP server, update installation, or user project was used.

## TL;DR

Factory is one of the clearest commercial examples of a coding-agent product
whose terminal, Desktop, automation, remote-computer, and integration surfaces
converge on one local agent runtime. The Desktop application is a stock
Electron workbench. It ships its own newer `droid` executable, starts or
connects to a local daemon, and exposes sessions, files, Git, permissions,
MCP, skills, plugins, worktrees, Missions, automations, remote access, browser
panels, and update state through a narrow preload bridge and a much larger
typed daemon protocol. The standalone CLI uses the same runtime in interactive,
headless JSON/JSON-RPC, and multiplexed daemon modes.

The strongest pattern to adapt is not Factory's UI. It is the product seam:

```text
terminal / Desktop / SDK / automation / remote control
                         |
          authenticated local daemon protocol
                         |
        one session, tool, policy, and work graph
                         |
       local host or explicitly selected computer
```

Factory also supplies important counterexamples:

- Desktop `0.131.0` embeds Droid `0.174.0`, while the separately installed
  CLI was `0.173.0`; the host and engine are separate release trains without
  a user-visible compatibility ledger in the audited artifacts;
- the Desktop ASAR is integrity-pinned and the app is notarized, but both the
  CLI and embedded CLI carry unusually permissive production entitlements,
  including `get-task-allow`, disabled library validation, unsigned executable
  memory, and DYLD environment-variable access;
- the OS sandbox is an opt-in beta, and its default per-command mode leaves
  the main Droid process outside the OS boundary;
- sessions, settings, memories, task state, logs, and parts of the extension
  system are ordinary local files under `~/.factory`; cloud session sync is
  documented as on by default;
- plugins can contain executable hooks and MCP processes, installed plugins
  follow marketplace branches unless SHA-pinned, and an isolated failed-auth
  first run cloned the default public plugin marketplace without asking;
- the official privacy, telemetry, security, and EU-deployment pages disagree
  about which transcript/tool data Factory stores and whether metrics always
  reach Factory; and
- binary update metadata exposes checksums, but the closed updater does not
  let this audit establish detached-signature, provenance, rollback, or
  compatibility-ledger enforcement.

The OpenAgents decision is therefore: **adapt Factory's one-engine/many-client
shape, daemon lifecycle, headless protocol, worktree and orchestration UX, and
hierarchical non-weakening policy; reject implicit sync, startup marketplace
mutation, opt-in containment, moving extension provenance, permissive release
entitlements, and ambiguous data-flow claims.**

## 1. Snapshot, provenance, and limitations

### 1.1 Audited artifacts

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Factory Desktop | `/Applications/Factory.app`, `0.131.0` build `7132`, arm64 | Signed application bundle, Electron host, renderer/preload/main bundles, embedded CLI, updater and daemon integration |
| Desktop executable | SHA-256 `aa48c79c3c300236598ab2bc58c5ca8e94f6e7bc0edae145b275c419903dea7e` | Exact native Electron launcher |
| Desktop ASAR | 38,256,970 bytes, SHA-256 `0b60f0d2d4577ee5012be11945de13c2e04a8b96be84e63637b486d0038f9750`; declared Electron integrity root `dc09aa799c33727e03c5285419033b3f023fea858d9831d6255c9b03b7952eed` | Exact shipped JavaScript/assets and Electron ASAR integrity binding |
| Downloaded installer | `Factory-0.131.0-arm64.dmg`, SHA-256 `7cfb8b7f18821b7d0789b4143da880d22a88f600f4b9c38d6d7c7674d2b60c67` | Installer provenance for this local snapshot |
| Embedded Droid | `0.174.0`, SHA-256 `4dee9b4ab3b1a89c2638bcb36c234aeadf82e466679d291524d34ce3de5c27b8` | Engine shipped with Desktop |
| Standalone Droid at smoke start | `0.173.0`, 116,957,808 bytes, SHA-256 `12777afca679a612eceb0d5ad476ce1dab65ac0b77e304649f410752ac1d2a5a` | Exact CLI used for isolated runtime behavior |
| Standalone Droid at audit close | `0.174.0`, 117,023,472 bytes, SHA-256 `957a95e85714205bcc1877017d8f83f546d5aef1bbacbf9d3c77eb71713dfea4` | Exact current installed CLI after a mid-audit replacement |
| Public Factory docs repository | commit `80d2d21a56b89f2bd814a1c801184af0970630bc` | Current official documentation; no Desktop or CLI implementation source |
| TypeScript Droid SDK | commit `d960f18f3a5a3bdbbc867a2177275a794663b175` | Public daemon/exec clients, protocol schemas, local daemon lifecycle, and session discovery |
| Official plugin marketplace | commit `e8801fa1020fbcd332c48aa068a80833bbe53e2e` | Plugin packaging, executable extension surface, and exact default marketplace snapshot observed by the smoke |
| Public skills repository | commit `21a74dec467a1424e3e067712d004cd987d6a0ce` | Reusable skill package examples |

Both application and CLI signatures chain to `Developer ID Application: The
San Francisco AI Factory Inc. (SW6TL4V6Q5)`. Desktop passed Gatekeeper
assessment, has a stapled notarization ticket, and uses the hardened runtime.
The standalone and embedded CLIs are independently signed hardened-runtime
binaries. [bundle]

### 1.2 Evidence labels

- **`[desktop-bundle]`** — observed in the installed signed app or DMG;
- **`[cli-binary]`** — observed in the installed or embedded native CLI;
- **`[runtime]`** — observed with an isolated temporary home and invalid test
  credential, without a real account or user data;
- **`[source]`** — observed in a commit-pinned public Factory repository;
- **`[public]`** — stated in official Factory documentation;
- **`[inferred]`** — supported by several observations but not directly
  asserted by Factory; and
- **`[limitation]`** — something this evidence cannot establish.

### 1.3 What is and is not public

The `projects/factoryai` lane contained all 20 repositories in its manifest,
but none contains the Factory Desktop or Droid CLI implementation. The main
`factory` repository is documentation, images, README, and GitHub automation.
The SDKs expose a large amount of the control protocol, not the agent loop or
Desktop source. The TypeScript SDK explicitly warns that its API can lag active
development. Runtime/doc/schema differences below are therefore evidence of
release and documentation drift, not automatically runtime vulnerabilities.
[source] [limitation]

The Desktop package is minified but not opaque. Its ASAR contains 387 files,
including 361 JavaScript files, Vite main and preload builds, a local renderer,
and many code-split workbench assets. No JavaScript source maps were present.
Preserved Zod
schemas, RPC method strings, route names, settings keys, prompts, state paths,
feature flags, metrics, and error branches still expose the shipped client
contract. The Bun-compiled CLI similarly retains exact internal module paths,
tool descriptions, prompts, schemas, settings, endpoints, and error strings,
but not a clean source tree. [desktop-bundle] [cli-binary]

This is enough to analyze client authority and packaging. It cannot prove
server-side authorization, routing, retention, deletion, model-provider
contracts, feature-flag values, or the behavior of a successful cloud run.
[limitation]

The standalone path changed from `0.173.0` to `0.174.0` during the audit. The
explicit `droid update --check` invocation did not install it and subsequently
reported `0.174.0` current. The causal updater could not be attributed. The
isolated runtime observations below belong to `0.173.0`; final signature and
static observations were repeated on `0.174.0`. [runtime] [limitation]

## 2. Whole-product architecture

Factory describes Droid as a filesystem-native local agent spanning CLI,
Desktop/Web, Slack/Teams, Linear/Jira, Mobile, CI, and remote computers. The
public SDK makes the shared-runtime claim concrete. It offers two transports:

1. `droid exec --input-format stream-jsonrpc --output-format stream-jsonrpc`
   starts one child process for one session and exchanges newline-delimited
   JSON-RPC over stdio; and
2. `droid daemon` exposes a multiplexed WebSocket endpoint for concurrent
   sessions and integrations.

The SDK documents the daemon as the path used by Slack, Linear, the REST API,
and Automations. The Desktop bundle ships the same daemon-capable executable
and its preload exposes daemon readiness, transport, send, receive, and
disconnect operations. [source] [desktop-bundle]

The public daemon method catalog is broad: session initialize/load/message,
queue, interrupt, close, search, archive, rewind, compact, and fork; PTY
create/write/resize/list; files and repository search; Git diff/commit/push/PR;
MCP discovery/configuration/OAuth; skills, commands, plugins, marketplaces;
automations and crons; permissions and user questions; workspace file reads;
proxy tokens, SSH keys, updates, and relay lifecycle. This is a real engine API,
not a chat-only IPC bridge. [source]

## 3. Factory Desktop

### 3.1 Host and trust boundary

Factory Desktop is stock Electron 42.3.3 / Chromium 148.0.7778.218 with an
entirely local Vite renderer. It is not a remotely served privileged website.
The bundle uses the custom
`factory-desktop://` callback scheme and has local app, auth, MCP OAuth, window,
file, notification, audio, browser-panel, stats, software-factory, session,
daemon, and updater bridges. [desktop-bundle]

The preload uses `contextBridge.exposeInMainWorld("electronAPI", ...)` rather
than exposing Node wholesale. The bridge is capability-shaped: a renderer can
request a directory picker, read a session file, search sessions, operate a
window, send a daemon message, or request an update action. That is a better
shape than renderer-owned child processes or a general IPC escape hatch.
[desktop-bundle]

The bridge is nevertheless high impact. It can open and reveal paths, write a
size- and extension-bounded base64 artifact, resolve audio files, operate
browser-panel caches and localhost proxying, toggle remote access, export
bug-report logs, and carry arbitrary
daemon protocol messages. Its safety therefore depends on main-process
validation and daemon authentication, not merely on `contextBridge`.
[desktop-bundle] [inferred]

The primary window enables context isolation, disables Node integration and
`webviewTag`, and relies on Electron's renderer-sandbox default. Strong fuses
disable RunAsNode, `NODE_OPTIONS`, and Node inspect arguments; require loading
from the integrity-bound ASAR; and enable cookie encryption. Those are good
host controls. DevTools remain enabled, `GrantFileProtocolExtraPrivileges` is
on, and the app itself is not under the macOS App Sandbox. [desktop-bundle]

Two missing checks make the broad preload more consequential. No explicit
top-level `will-navigate` denial was found, and the main bundle contained no
`senderFrame`, sender URL, or `webContents.getURL()` validation in its roughly
69 IPC handlers. A navigation-start listener disconnects daemon routing but is
not an origin gate. Child windows are denied after allowlisted external schemes
are opened, which is useful but does not cover top-level navigation. No
`setPermissionRequestHandler` was found. [desktop-bundle]

Factory installs a CSP, but it permits `unsafe-inline`, `unsafe-eval`, blob
scripts, a long third-party/Factory origin list, and localhost. Daemon
responses on known/current loopback ports are explicitly exempt from CSP
injection. Combined with absent navigation and IPC-origin checks, renderer
content integrity is a load-bearing authority boundary rather than defense in
depth. [desktop-bundle] [inferred]

### 3.2 The Desktop is a daemon client

Desktop embeds Droid `0.174.0` under `Contents/Resources/bin/droid`. The app
tracks renderer-client readiness, daemon availability and transport, daemon
PID/startup failures, parent liveness, reconnection, diagnostics, and logs. The
preload's typed session schemas include pending permissions, pending user
questions, queued messages, working state, Git/worktree metadata, mission
state, worker sessions, token use, subagent invocations, compaction, rewind,
skills, tools, and MCP. [desktop-bundle]

The public SDK corroborates the lifecycle. A local SDK daemon normally binds
`127.0.0.1`, probes from port `37643`, persists the selected port in
`~/.factory/sdk/daemon.port` with mode `0600`, detaches, and writes daemon
stderr under `~/.factory/sdk/logs`. A client authenticates over localhost
WebSocket using either a WorkOS JWT or an `fk-*` API key; success returns user
and organization identity. Plain loopback WebSocket is therefore protected by
an application credential, not left completely ambient. [source]

The native binary also supports Unix IPC, inherited child IPC, parent/liveness
monitoring, an explicit `--remote-access` mode, and runtime settings injection.
This is a stronger lifecycle surface than “spawn a CLI and scrape stdout,” but
the audited public SDK does not establish peer-bound credentials, per-client
authority scopes, bounded mailboxes, or durable admission semantics for every
daemon method. [cli-binary] [limitation]

Desktop normally spawns the embedded binary as `droid daemon` with inherited
child IPC and its own embedded Droid path. It disables Droid self-update for
that child, removes `FACTORY_API_KEY` from the spawned environment, points it
at production Factory endpoints, and shares the user's `~/.factory` state.
This makes the two release trains explicit: Desktop owns its engine version,
while the standalone CLI can move independently. [desktop-bundle]

### 3.3 Workbench breadth

The shipped renderer contains dedicated chunks or protocol state for files,
syntax-aware diffing, Git, code review, hooks, agents, skills, plugins,
marketplaces, MCP, settings, browser panels, local-site proxying, PowerPoint,
Word, PDF and spreadsheet previews, worktrees, automations, and Mission
Control. The public Missions documentation describes a visual orchestrator,
workers and validators, pause/replan/resume, terminal and thought output, and
selection of a remote Droid Computer. [desktop-bundle] [public]

This is closer to a persistent engineering workroom than a Desktop chat shell.
The useful OpenAgents lesson is progressive disclosure: one session can deepen
into files, diffs, terminals, child topology, automation, or remote placement
without moving the engine into those panels. [inferred]

### 3.4 Release security and update posture

Positive evidence:

- notarized Developer ID bundle with stapled ticket;
- hardened runtime;
- Electron ASAR integrity recorded in `Info.plist`;
- separately signed embedded CLI; and
- explicit updater states for checking, downloading, installing, errors,
  diagnostics, recovery, and rollback-oriented changelog behavior.

Material concerns:

- bundle id remains the generic `com.electron.factory`;
- `NSAllowsArbitraryLoads` is enabled;
- the app has client and server networking plus JIT, unsigned executable
  memory, and disabled library validation entitlements; and
- the embedded CLI additionally carries `get-task-allow` and DYLD environment
  entitlement in a production Developer ID build.

Electron and Bun JITs explain some relaxed code-signing needs. They do not
explain why a production CLI must remain attachable to a debugger or accept
DYLD injection with library validation disabled. OpenAgents should treat
release entitlements as a deny-by-default manifest and gate unexpected
additions. [desktop-bundle] [cli-binary]

The app asks Factory's API for current Desktop version and configures
Electron/Squirrel update feeds. Public update protocol types include a binary
URL and checksum URL. The implementation that proves signature, checksum,
publisher, provenance, compatibility, atomic activation, and rollback is not
public. Notarization proves the installed snapshot's publisher; it does not
prove the entire update channel. [desktop-bundle] [source] [limitation]

The exact macOS arm64 feed is
`https://downloads.factory.ai/factory-desktop/updates/darwin/arm64/RELEASES.json`,
with a separate `LATEST` marker and Factory API fallback. The client validates
semantic-version agreement before handing activation to Squirrel. No embedded
update public key or application-level signature pin was found; Apple code
signing is the visible publisher boundary. [desktop-bundle]

## 4. Droid CLI

### 4.1 Packaging and modes

The standalone CLI is a 112 MB arm64 Mach-O produced with Bun 1.3.14. Its only
dynamic libraries are macOS system ICU, resolver, C++, and System libraries.
It has three primary product modes:

- interactive Ink/React TUI;
- `droid exec` for text, JSON, streaming JSON, or bidirectional streaming
  JSON-RPC automation; and
- `droid daemon` for multiplexed WebSocket or IPC clients.

Top-level commands also cover session search, MCP, plugins, remote computers,
and updates. Exec supports continuation, fork, exact session identity,
worktrees, spec mode, model/reasoning selection, tool enable/disable/restrict,
tags, mission orchestrators/workers/validators, and appended system prompts.
[cli-binary] [public]

The final `0.174.0` binary retained 389 recognizable internal source paths,
including auth, settings, sandbox, daemon WebSocket, relay, plugin registry,
provider routing, tool, telemetry, updater, and session modules. Runtime errors
emit source-level paths and line numbers. This is highly inspectable compiled
code, but no complete recoverable source map was established. [cli-binary]

`--list-tools` worked without authentication and returned 29 tool definitions,
including full descriptions and current allow state. That is good automation
discoverability, though the tool schema and effective authority should be a
versioned protocol response rather than an incidental help dump. [runtime]

### 4.2 Sessions, recovery, and local state

Public SDK code discovers plaintext session JSONL and sibling settings JSON
under `~/.factory/sessions`. Session metadata includes owner, working
directory, mission linkage and tags; per-session settings retain model/provider,
timestamps, token/credit and child usage, archive state, tool overrides,
compaction, and routing choices. Favorites live in a separate file. Resume,
fork, rewind, compact, search, archive, queue, and interruption are engine
operations available to other clients. [source]

Factory does not document an implicit learned cross-session memory system.
Durable user/project memory is explicit Markdown in `~/.factory/memories.md`,
`.factory/memories.md`, rules, AGENTS files, skills, and hooks. This separation
is good: session history, authored instruction, reusable capability, and
preference are not collapsed into one magical memory plane. [public]

An isolated invalid-key run still created a host UUID, installation telemetry
UUID, feature-flag and certificate caches, logs, a JSONL session and settings
snapshot, built-in Droid Markdown, and task/background stores. It attempted
three session-create requests and then a telemetry flush. These are observed
startup effects, not merely documented possibilities. [runtime]

### 4.3 Missions, subagents, and worktrees

Missions are a first-class multi-agent mode with separate orchestrator, worker,
and validation models and reasoning effort. Settings can skip scrutiny or
user-testing validation; Mission mode requires high autonomy or the unsafe
permission bypass. Ordinary Task subagents have light/medium/heavy model
routing and an autonomy level that defaults to inheriting the parent. Custom
Droids can bypass tier routing with an explicit model. [public] [cli-binary]

Both interactive and exec modes can create a sibling Git worktree on a named
branch, with a configurable root. The isolation is concrete Git topology, not
just a session label. The available evidence does not show an atomic contract
tying worktree creation, child admission, authority intersection, result
acceptance, review, commit, push, and cleanup into one receipted workflow.
[public] [limitation]

### 4.4 Permissions are not containment

Factory now distinguishes interaction mode from autonomy level. `off` keeps
manual approval; low, medium, and high pre-authorize progressively riskier
effects. `--skip-permissions-unsafe` deliberately bypasses prompts. Commands
can be allowlisted, placed on a confirmable denylist, or put on an absolute
blocklist that documentation says survives full autonomy and the unsafe flag.
Org policy can clamp maximum autonomy and subagent autonomy. [public]

The OS sandbox is a separate opt-in beta:

| Aspect | Documented behavior |
| --- | --- |
| macOS | Seatbelt |
| Linux / WSL2 | bubblewrap plus seccomp |
| Network | filtered HTTP/SOCKS proxy |
| Default mode | per-command; main Droid process remains outside the OS sandbox |
| Whole-process mode | agent plus MCP and children inside the boundary; refuses startup if isolation cannot be established |
| Default reads | allow, except explicit denies |
| Default writes | current working directory only |
| Default egress | Factory domains always allowed; other domains denied |

This is a meaningful containment implementation, especially its fail-closed
whole-process startup. It is not a safe default for a daemon holding auth,
telemetry, plugin, MCP, hook, model, session, and remote-computer authority.
OpenAgents should retain named fail-closed containment profiles and record
effective enforcement separately from permission UI. [public] [inferred]

## 5. Extension system and supply chain

### 5.1 MCP, skills, Droids, and hooks

MCP supports local stdio, HTTP streamable, legacy SSE, OAuth discovery and
registration, and layered user/project/folder configuration. OAuth credentials
are global in the OS keyring or a fallback file, so authorization can outlive
or cross project boundaries even when MCP configuration is project-local.
Project secrets may expand from environment variables in memory. [public]

Skills use progressive disclosure and can be model-selected from semantic
descriptions unless model invocation is disabled. Factory recognizes both its
own skill roots and `.agent/skills`. Custom Droids and commands add reusable
agents and workflows. Executable hooks receive sensitive lifecycle payloads,
including prompts, transcript paths, file/tool inputs, and results. [public]

These are useful compatibility surfaces, but they all need the same authority
compiler. A Markdown skill, hook command, MCP process, plugin-provided agent,
or built-in tool must not acquire broader filesystem, egress, credential,
publication, or spend authority merely because it was discovered through a
trusted-looking catalog. [inferred]

### 5.2 Plugins are executable moving dependencies

Plugins may bundle skills, slash commands, agents, executable hooks, and MCP
configuration. They can be installed at user, project, or organization scope.
Factory supports exact commit pins, but the default marketplace tracks its
default branch and installed plugins are documented to refresh on CLI startup.
[public]

The isolated failed-auth smoke silently cloned `Factory-AI/factory-plugins` at
commit `e8801fa1020fbcd332c48aa068a80833bbe53e2e` into the temporary
`~/.factory/plugins` tree. Authentication failure therefore did not prevent
startup network access and local executable-catalog mutation. [runtime]

The official marketplace includes a preinstalled core and plugins with shell
scripts, MCP processes, code-review/security automation, and a `droid-control`
package whose architecture drives PTYs, Playwright/Electron, and desktop
computer-control daemons. Moving-branch auto-update is too weak a default for
that authority. OpenAgents should require immutable content identity,
publisher and provenance evidence, declared capabilities, review state,
compatibility bounds, explicit activation, and rollback. [source] [inferred]

## 6. Data flow, privacy, and telemetry audit

The official documentation does not currently support one unambiguous answer
to “what leaves this machine?”

| Topic | Official statements that need reconciliation |
| --- | --- |
| Session sync | Settings document `cloudSessionSync=true` by default and say every CLI session is mirrored to Factory web. EU deployment says prompts, assistant messages, tool calls/results, and checkpoints are stored in the regional database. |
| Code custody | Privacy says no static/cold codebase copy is uploaded and file access stays local, while CLI security says necessary context and diffs are sent to Factory's cloud. These can both be true only with a precise distinction between repository indexing, model context, synced transcripts, tool payloads, and diffs. |
| Model path | Privacy says configured enterprise requests can go directly to providers without Factory storing prompts/responses. That does not explain the separate default session-sync envelope. |
| Telemetry | Telemetry export says metrics fan out to Factory and a customer collector and that failures are isolated. Privacy says high-security customers may never send telemetry to Factory and Factory analytics are optional. Airgap behavior is a third mode. |
| Content telemetry | Message/tool content export is off by default and, when explicitly enabled, documented to go only to the customer's OTEL endpoint. This is clearer than the ordinary metrics story. |

The right fix is not another blanket “local-first” sentence. Factory needs a
mode-by-mode data-flow table covering authentication metadata, feature flags,
repository files, selected context, prompt/system instructions, model traffic,
assistant output, tool arguments/results, checkpoints, synced session state,
logs, crash reports, metrics, plugins, updates, retention, deletion, region,
training use, and administrator controls. [public] [inferred]

OpenAgents should preserve an even stronger product rule: local artifact
custody, local execution, remote inference, cloud sync, telemetry, and training
use are separate typed facts. Each should be independently visible and
governed.

## 7. What OpenAgents should adapt

1. **One engine, several real clients.** TUI, headless automation, Desktop,
   mobile/remote control, SDKs, and integrations should share one Thread/Turn/
   Item/Work Unit/Interaction/Receipt authority rather than parallel chat
   loops.
2. **A long-lived authenticated local supervisor.** Adapt explicit startup,
   port/socket discovery, identity, capability negotiation, reconnect,
   liveness, diagnostics, shutdown, and update handoff. Add client-scoped
   authority, protected generation secrets, bounded queues, overload outcomes,
   durable admission, and receipts.
3. **Bidirectional headless JSON-RPC.** Permission and question callbacks,
   streaming updates, exact session ids, cancellation, and structured output
   belong in a generated protocol usable from CI and SDKs.
4. **Workbench projection over engine state.** Files, diffs, Git, terminals,
   child agents, worktrees, automations, browser panels, and remote placement
   should deepen one session without becoming renderer authority.
5. **Hierarchical non-weakening policy.** Org/project/folder/user layers should
   compose with explicit precedence, unions for mandatory denies, locked
   object keys, and a machine-managed pre-login policy path.
6. **Separate permission, policy, and containment.** Keep interaction prompts,
   deterministic block policy, and OS enforcement as distinct artifacts.
   Children receive the intersection of parent authority and named role scope.
7. **Engine-owned worktrees and rewind.** Retain typed create/reuse/fail,
   snapshot, fork, compact, restore, external-conflict detection, and cleanup
   semantics, strengthened by review and effect receipts.
8. **Exporter-side privacy enforcement.** Content-free metrics should be a
   closed schema; content export requires a separate destination and explicit
   opt-in. UI must state whether Factory/OpenAgents, a customer collector, or
   neither receives each class.

## 8. What OpenAgents should reject

1. Opt-in workload containment or a default mode that leaves the authority-
   carrying engine unsandboxed.
2. Any unsafe permission switch that can override non-negotiable policy.
3. Moving marketplace branches and silent first-run executable-catalog clones.
4. Globally reusable integration credentials without visible project scope and
   revocation.
5. Production code-signing entitlements that permit debugging and injection
   without a narrowly documented, tested need.
6. A renderer-to-daemon raw-message bridge without schema validation,
   capability scoping, cancellation, bounds, and generation identity.
7. “Local” or “airgapped” as umbrella claims that hide sync, model, telemetry,
   update, and plugin traffic.
8. Checksums alone as release provenance; require signed manifests, artifact
   identity, SBOM/provenance, compatibility, staged activation, last-known-good
   state, and rollback receipts.

## 9. Final assessment

Factory's architectural contribution is substantial. It demonstrates that a
terminal agent can grow into a multi-surface engineering system without making
Desktop the engine: the daemon owns sessions and tools, the preload owns host
capabilities, and the renderer owns presentation. Its headless JSON-RPC,
remote-computer path, worktrees, Mission topology, and hierarchical controls
are all serious product references.

Its weakest seams are equally instructive. Authority remains concentrated in
an engine that is unsandboxed by default; executable extensions can move on
startup; the two shipped engine versions are not explained by a public
compatibility ledger; local/cloud/telemetry claims do not reconcile; and
release hardening is undermined by permissive CLI entitlements and a closed
update-verification path.

OpenAgents should copy the convergence and improve the laws: one generated
protocol, one durable authority, explicit client and runtime generations,
default fail-closed containment, immutable extensions, precise data-flow
truth, signed component ledgers, and receipts for every consequential state
transition.
