# Amp Code Teardown — 2026-07-16

This is a read-only architecture and product audit of the Amp CLI installed on
this Mac, Amp's public manual and security material, its published package and
SDK surfaces, and the small amount of current public integration source. It did
not sign in, run an agent, inspect private threads, read user configuration or
credentials, connect an IDE, start a runner, create an Orb, or exercise a paid
model or tool.

## TL.DR

Amp is no longer best understood as a terminal coding agent. It is a
**model-routed, cloud-backed thread system whose local CLI is both a client and
an execution worker**. One thread can be created or continued from the TUI,
driven through Claude-compatible streaming JSON, inspected and reviewed on the
web or mobile, remote-controlled while its CLI remains alive, or placed on a
runner or managed Orb. The current product routes different jobs through
different frontier and specialist models, automatically delegates isolated
subagents, and lets TypeScript plugins add tools, commands, UI, modes, and new
agents. [public] [cli-binary]

The installed artifact makes part of that architecture unusually visible even
though the core source is closed:

```text
CLI / stream JSON / SDK / IDE bridge / web / mobile
                    |
          Amp thread and plugin protocols
                    |
        server-issued thread actor credentials
                    |
       stateful actor connection over WebSocket
                    |
     local tools and plugins | remote runner / Orb
                    |
       routed model and specialist services
```

The 71.3 MB macOS executable is a Bun 1.3.14 standalone program. Its Mach-O
contains a 7.76 MB `__BUN,__bun` payload with a large minified JavaScript bundle,
RivetKit actor and workflow packages, WebSocket and gRPC clients, Commander,
MCP, a native keyring binding, telemetry, update code, and the CLI/TUI product.
The payload is inspectable, not encrypted, but it is not public source with
history, tests, readable module boundaries, or a license permitting the core to
be treated as open. Amp's former public-looking repository URL returns 404.
the current npm CLI packages distribute wrappers and prebuilt executables.
[cli-binary] [registry] [limitation]

Amp's strongest product idea is the thread fabric. A long thread remains a
searchable source record even after compaction. Other threads can reference it.
`read_thread` searches the original history and distinguishes attempts from
later outcomes. Queue, steer, and force-interrupt are separate user actions.
and web/mobile review deepens the same work rather than starting another chat.
OpenAgents should adapt that shape: durable work history should be remotely
controllable, semantically searchable, and readable by bounded agents without
flattening exact execution evidence into a summary. [public]

Its strongest negative lessons are equally concrete:

- Amp runs tools without asking by default and recommends a policy plugin or
  external isolation for untrusted workspaces. Permission rules are
  authorization callbacks, not demonstrated OS containment. [public]
- Threads are a cloud data substrate. They may retain messages, model output,
  tool calls/results, attachments, and snippets or entire files. Workspace
  defaults and administrator visibility materially affect privacy. [public]
- The same extension plane can add executable tools, lifecycle continuation,
  selectable modes, and subagents. Public types are not evidence of process,
  filesystem, credential, egress, or spend isolation. [public] [limitation]
- The direct installer verifies a SHA-256 value fetched from the same vendor
  origin as the executable. Its minisign implementation is commented out with
  release signing disabled. The installed macOS binary is only ad-hoc/linker-
  signed and fails both `codesign --verify` and Gatekeeper assessment. A passed
  vendor checksum does not provide independent publisher or Apple notarization
  assurance. [installer] [runtime]
- Amp explicitly optimizes for evolution over compatibility. In months it has
  removed its editor extension, Tab completion, public thread discovery,
  custom commands, fork, TODO, walkthrough, and old mode names. That can be a
  coherent product strategy, but it makes protocols, settings, plugins, docs,
  and saved work compatibility a first-class risk. [public]
- Core load-bearing behavior remains closed. The bundle can establish shipped
  client branches and dependencies. It cannot prove server authorization,
  actor durability, exactly-once admission, model routing, deletion, training,
  retention, or isolation. [limitation]

The central OpenAgents decision is: **adapt Amp's durable, cross-surface thread
fabric, semantic history reading, explicit steer/queue distinction, and
specialist-agent composition. Reject cloud transcript authority, default-open
execution, opaque durability, unsigned release authority, and extension-driven
privilege amplification.**

## 1. Snapshot identity, provenance, and confidence

### 1.1 Audited artifacts

| Artifact | Exact identity | What it establishes |
| --- | --- | --- |
| Installed CLI | `0.0.1784247472-g76909f`. Release timestamp `2026-07-17T00:17:52Z`. MacOS arm64 | Shipped command surface, embedded runtime, package fingerprints, strings, linkage, and local release posture |
| Installed CLI digest | SHA-256 `521a9473876d488a5f05f9ea8fca20c9686d3321422dea5f3f0283576f4d9bdc` | Exact local binary audited here |
| Installed CLI size | 71,256,290 bytes. Mach-O UUID `C7E7A979-F99B-3466-9AD6-E56A63373A35` | Exact artifact and platform identity |
| Direct installer | `https://ampcode.com/install.sh`, read 2026-07-16 | Version selection, platform detection, checksum, signature, install, and PATH behavior |
| npm CLI packages | `@ampcode/cli@0.0.1784247472-g76909f` and `@ampcode/cli-darwin-arm64@0.0.1784247472-g76909f` | Wrapper/postinstall and platform-binary distribution. No core source |
| Legacy npm alias | `@sourcegraph/amp@0.0.1784247472-g76909f` | 630-byte rename shell pointing users to `@ampcode/cli` |
| TypeScript SDK | `@ampcode/sdk@0.1.0-20260605144103-g77da114` | Published execution/thread/permission wrapper, declarations, and compatibility code under the Amp Commercial License |
| Plugin API | `@ampcode/plugin@0.0.0-20260717002806-g76909f1` | 42 KB public type contract for tools, events, UI, threads, modes, and agents |
| Public Neovim bridge | `ampcode/amp.nvim`, read 2026-07-16 | Current inspectable editor-bridge behavior. Not the Amp engine |
| Manual, models, Chronicle, security, and privacy pages | read 2026-07-16 | Current intended product behavior, provider/data disclosures, pricing, and product-change history |

The version's numeric prefix corresponds to a UTC release instant that falls
on July 16 in the local America/Chicago timezone. The embedded build metadata
reports `2026-07-17T00:22:42.639Z`, about five minutes after the version's
release timestamp. This document uses the local audit date in its title.
[cli-binary]

The npm registry integrity for the exact `@ampcode/cli` wrapper is
`sha512-Zp77HaYp2WVuNKsC5mzgGEwZBMSaepfYUiAs4V4VWpij9aF2i8vcpE82VTzi4A3I5Sukh6FIcYJcc1++fWhJNw==`.
The platform package contains the same-sized 71,256,290-byte `amp` executable,
but this audit did not download and byte-compare that package with the direct-
installer artifact. [registry] [limitation]

### 1.2 Evidence labels

- **`[cli-binary]`** — static evidence from the installed Mach-O, embedded Bun
  bundle, command help, or version output.
- **`[runtime]`** — observed macOS verification or isolated non-agent command
  behavior.
- **`[installer]`** — the current official direct-install shell script.
- **`[registry]`** — current npm manifests, tarball inventories, integrity, or
  published declarations.
- **`[source]`** — current public source such as `amp.nvim`.
- **`[public]`** — Amp's current manual, model page, Chronicle, security,
  privacy, pricing, or news pages.
- **`[inferred]`** — a conclusion supported by multiple observations but not a
  vendor claim. And
- **`[limitation]`** — something the available artifacts cannot establish.

### 1.3 Audit boundary and limitations

Only `amp --version`, `amp --help`, and nested `--help` paths were executed.
They did not authenticate or start an agent, but they touched an empty
`~/.cache/amp/logs/cli.log`. No real settings, secrets, thread records, user
history, IDE selection, plugin, MCP credential, repository content, or Amp
account state was read. No remote request with an Amp credential was made.
[runtime]

The executable is stripped to one exported Mach-O symbol, but Bun preserves a
large minified application bundle and many literals. Static evidence can prove
that the client includes a branch, schema, endpoint, dependency, or local
decision. It cannot prove that a remote endpoint is enabled, that ordinary
users can call it, that the server enforces the same schema, or that a storage
and deletion claim holds under every failure. [cli-binary] [limitation]

## 2. Public-source reality: open interfaces around a closed core

Amp's current core is not open source. The repository URL present in published
SDK metadata, `https://github.com/sourcegraph/amp`, returns 404 to the public.
The May 2026 packaging announcement says the CLI changed from a JavaScript
source package to a Bun-compiled single-file executable. The old npm package is
now a tiny rename shell. `@ampcode/cli` is a wrapper plus platform-specific
binary packages. [public] [registry]

The published surface divides into four different evidence classes:

| Surface | Publicly readable? | Source status | Architectural value |
| --- | --- | --- | --- |
| Core CLI/TUI/runner | Binary and minified embedded bundle | Closed production artifact | Strong shipped-client evidence, weak development/process evidence |
| Hosted thread/model services | Documentation and client protocol traces | Closed service | Product contract and data-flow claims only |
| TypeScript SDK and plugin API | Distributed JS/declarations and manual | Commercially licensed interface package | Useful client and extension contract, not server or engine source |
| `amp.nvim` and examples | Git history and readable source | Open public integration/example code | Direct editor bridge and usage evidence, not core authority |

This matters because “the source is in the binary” would be an overstatement.
The embedded minified program has no public review history, readable original
module graph, source maps established by this audit, test suite, issue-to-fix
trace, or permission to treat the core implementation as an open dependency.
Conversely, “Amp is opaque” would also be inaccurate: the Bun payload exposes
substantial client code, types, prompts, endpoints, dependency identities,
settings, and failure messages. [cli-binary]

## 3. Product thesis: the frontier moves, the thread survives

Amp's manual names four principles: use unconstrained token budgets, use the
best available models, expose raw model power, and evolve with new models. Its
product copy operationalizes those principles through four current modes—
`low`, `medium`, `high`, and `ultra`—rather than a user-selected provider/model
matrix. The model page separately routes review, search, Oracle, Librarian,
thread reading, media, image generation, titling, and compaction. [public]

The durable product concept is not the mode. It is the thread. Models and
features can be replaced while the thread URL, history, references, search,
review, and remote control remain the collaboration object. That yields a
distinct strategy:

```text
stable-ish social/work identity: project -> thread -> messages/tools/changes
rapidly moving execution policy: mode -> model + prompt + tools + specialists
```

This is the inverse of products that promise a stable local engine and let the
user select providers. Amp promises the current product's judgment about the
best composition and treats backward compatibility as a cost. [public]

The strategy has real benefits. A specialist can be upgraded without migrating
the visible thread format. Web and mobile can review work without reimplementing
the local tool loop. And the CLI can become a runner without inventing a second
job product. It also concentrates authority in changing server-side routing and
closed compatibility decisions. A saved thread does not by itself prove which
model, prompt, catalog, policy, provider retention mode, or execution generation
produced each consequential effect. [inferred] [limitation]

## 4. Packaging, startup, and update shape

### 4.1 One Bun executable

The local binary is a thin arm64 Mach-O linked only against Apple system ICU,
resolver, C++, and System libraries. Its `__BUN` segment carries the JavaScript
application. Strings identify Bun 1.3.14, JavaScriptCore, Node/Bun built-ins,
`ws`, Commander, `@grpc/grpc-js@1.14.4`, `@napi-rs/keyring@1.1.10`,
`cbor-extract@2.2.2`, `pino@9.14.0`, and `thread-stream@3.1.0`.
[cli-binary]

This gives Amp a fast, dependency-light deployment artifact while retaining a
TypeScript/JavaScript product implementation and native packages. It is closer
to Factory's Bun-compiled Droid than to Codex's Rust workspace or Command
Code's Node-distributed minified module. [inferred]

### 4.2 Distribution matrix

The npm wrapper declares optional packages for macOS arm64/x64, Linux
arm64/x64, and Windows x64. The manual supports Windows through WSL rather than
claiming the native Windows executable as the preferred interactive product.
The direct script adds x64 baseline variants and selects arm64 under Rosetta.
[registry] [installer] [public]

The direct installer:

1. fetches `cli-version.txt` from `static.ampcode.com`.
2. fetches the versioned checksum.
3. fetches the versioned platform executable.
4. compares SHA-256.
5. makes the file executable. And
6. links it into a PATH directory, preferring `~/.local/bin`.

It downloads into a temporary file before rename, which is good installation
hygiene. It also labels itself “EXPERIMENTAL - NOT DOCUMENTED” in a comment even
though Amp's May announcement calls direct installation recommended.
[installer] [public]

### 4.3 Update authority

The CLI has explicit `update`, update-mode settings (`warn`, `disabled`, or
`auto`), version endpoints, download-temporary paths, checksum names, relaunch
commands, and minimum-open-duration logic. That is evidence of a staged local
updater, not evidence of a signed release transaction, compatibility ledger,
last-known-good slot, or rollback receipt. [cli-binary] [limitation]

The installer contains a `verify_signature` function using minisign, but the
call is commented out with “Disabled until release signing is enabled.” The
downloaded checksum and executable share the vendor-controlled static origin.
Compromise of that publication authority can replace both. [installer]

On this Mac, `codesign -dv` reports identifier `a.out`, ad-hoc/linker-signed,
no Team ID, and no bound resources. `codesign --verify --verbose=4` and
Gatekeeper `spctl` both report an invalid signature. This may be a Bun
standalone construction artifact rather than evidence that the download was
modified—the installer's expected checksum matched—but it means the artifact
does not carry working Apple Developer ID/notarization assurance. [runtime]

## 5. Runtime architecture: a local worker attached to a thread actor

The bundle includes RivetKit and agent-actor packages:

- `rivetkit@0.0.0-fix-ws-size.7d55b4f`.
- `@rivet-dev/agent-os-core` and `@rivet-dev/agent-os-pi`.
- `@rivetkit/workflow-engine`, virtual WebSocket, engine protocol, traces, and
  N-API/Wasm packages. And
- client routes for `/api/thread-actors` and `/api/user-actor-credentials`.

Runtime messages describe opening a `threadActor` connection over WebSocket.
The CLI also exposes raw actor thread export. Together, these establish a
stateful actor/client protocol in the shipped product, not a CLI that merely
POSTs one prompt and streams one answer. [cli-binary]

Amp's “Agents, Everywhere” announcement calls the current system distributed
and its execution durable. The actor packages make that claim architecturally
plausible. They do not establish the storage engine, transaction boundaries,
idempotency keys, lease/fencing law, crash semantics, actor migration,
reconciliation, or whether every accepted local effect is durably admitted
before execution. [public] [cli-binary] [limitation]

The likely responsibility split is:

| Plane | Observed responsibility |
| --- | --- |
| CLI process | TUI, local commands/tools, plugin and MCP clients, IDE context, notifications, logs, updates, actor connection |
| Amp service | identity, projects, thread lookup/sharing/search, actor credentials, model routing, usage/billing, remote-control coordination |
| Thread actor | live thread command/event coordination and remote client attachment |
| Runner | accepts remotely created threads in one configured working directory |
| Orb | Amp-managed remote machine and service/portal lifecycle |
| Web/mobile | thread inspection, messaging/remote control, diff review/staging, sharing and collaboration |

The table combines documented product behavior with client evidence. It is not
a recovered server deployment diagram. [public] [cli-binary] [inferred]

## 6. Threads, persistence, and work history

### 6.1 The thread is the canonical user object

The CLI can create, continue, list, search, label, share, rename, archive,
delete, render as Markdown, export as JSON, and export raw actor data. A thread
contains user messages, model responses, context, tool calls/results, and
attachments. It has a stable `T-...` identifier and URL and can be referenced
from another prompt. [public] [cli-binary]

The server stores thread conversations in multi-tenant GCP PostgreSQL according
to the security reference. The client stores settings, local prompt history,
logs, credentials, MCP OAuth material, plugins, skills, and IDE/session bridge
state in separate platform paths. This is a cloud-canonical thread product,
not a local JSONL-first engine like Claude Code or Codex. [public]

The manual does not publish a thread event schema with stable Turn/Item/effect
identities, an append-only admission contract, projection cursor, or repair
law. Stream JSON compatibility describes an automation projection, not
necessarily the canonical actor log. Raw export is useful for recovery and
debugging, but exportability alone does not prove portable resume on a foreign
runtime. [public] [limitation]

### 6.2 Search and cross-thread reading

Amp searches threads by text, file, project/repository, ref, author, label,
archive state, and date. Mentioning a thread invokes a dedicated `read_thread`
agent. Amp's published account of very long histories says the reader searches
original thread material, checks later revisions and reverts, and treats tool
calls as attempts rather than outcomes. [public]

That is a strong design distinction:

```text
compaction summary = orientation and context-budget artifact
original event     = exact historical evidence
later event        = may supersede, revert, or invalidate an earlier attempt
```

OpenAgents should adopt this law directly. Neither transcript order nor tool
invocation implies accepted current state. Retrieval should return evidence
with causal and supersession context. [inferred]

### 6.3 Queue, steer, and interrupt

The interactive client distinguishes three follow-up intents:

- normal send while busy queues until the current turn ends.
- double Enter marks a steer for the next safe interruption point. And
- double Escape stops current work and sends immediately.

Streaming JSON input exposes the same steer bit programmatically. This is much
clearer than products that infer semantics from whether a request arrived while
a spinner was visible. [public]

The remaining unknown is server truth: the manual does not establish whether a
queued or steered message is admitted durably before acknowledgment, how it is
ordered against reconnecting clients, how duplicate submission is handled, or
what exact safe boundary fences a steer. OpenAgents should adapt the UX only
above durable idempotent command identities and explicit accepted/queued/
delivered/interrupted outcomes. [limitation]

### 6.4 Visibility is collaboration policy

Visibility can be private, workspace, group, or unlisted. “Unlisted” is
internet-readable to anyone with the link. Workspace members can receive
workspace sharing by default. Admins can govern defaults and external sharing.
Private workspace threads remain visible to administrators under stated
conditions, with passkey requirements for privileged viewing. [public]

The Chronicle's “End of Public Threads” removed discoverable public thread
sharing, not the unlisted-link capability still documented by the manual. The
terminology should not be collapsed into “threads are no longer public.”
[public] [inferred]

## 7. Model routing and specialist composition

At this snapshot, Amp's public model page maps:

| Role | Current model |
| --- | --- |
| low mode | GLM-5.2 |
| medium mode | GPT-5.6 Sol |
| high mode | GPT-5.6 Sol |
| ultra mode | Claude Fable 5 |
| review | GPT-5.5 |
| search | GPT-5.6 Terra |
| Oracle | GPT-5.6 Sol, or Claude Fable 5 when high mode already uses Sol |
| Librarian | GPT-5.6 Sol |
| read thread | GLM-5.2 |
| media | Gemini 3 Flash |
| Painter | GPT Image 2 |
| titling | Claude Haiku 4.5 |
| compaction | GPT-5.6 Sol |

These are point-in-time routing facts, not protocol identities. Amp changed its
main modes and model assignments repeatedly in the retained Chronicle. A thread
and receipt system needs to preserve exact routed model, provider, prompt/
catalog generation, reasoning policy, cost, and retention class per call even
when the friendly mode name stays stable. [public] [inferred]

The Oracle is an especially good product pattern: it deliberately uses a
different frontier model from the main agent when high mode would otherwise
duplicate it. That creates genuine architectural diversity rather than asking
the same model to restate its answer. It still produces advice, not independent
verification or acceptance authority. [public] [inferred]

Pricing is usage-based. The manual says individual and non-enterprise usage is
passed through at provider/tool cost with zero markup, a $5 minimum purchase,
and no subscription. Unused credits expire after one year of inactivity.
Enterprise usage is 50% higher and begins with a one-time $1,000 purchase that
also grants $1,000 in credits. [public]

## 8. Subagents, review, and orchestration

### 8.1 Ordinary subagents

Amp automatically spawns subagents for suitable work, mostly in medium mode.
Each has a separate context and tools. The manual is unusually candid about the
limits: children cannot communicate with one another, cannot be steered while
running, start without the accumulated parent conversation, and return only a
final summary to the main agent. [public]

Stream JSON represents child messages with `parent_tool_use_id` pointing at the
Task tool and waits for all children before the final result. Plugin-created
agents can receive an explicit `parentThreadID`. These are useful lineage
fields, but the public projection does not establish a complete durable graph,
mailboxes, incremental child transcript navigation, delivery acknowledgment,
authority intersection, or acceptance state. [public] [limitation]

### 8.2 Review as a fan-out program

`amp review` can run built-in or project-defined checks. Markdown check files
declare name, description, default severity, and allowed tools. A separate
subagent executes each check. This is a clean way to turn team review criteria
into parallel bounded analysis without bloating the main thread. [public]

OpenAgents should adapt the manifest shape while preserving a stricter proof
law. A check result is an observation bound to exact source and environment.
it is not a release verdict until admitted evidence satisfies an AssuranceSpec.
Tool restriction in Markdown also needs enforcement evidence from the runtime,
not just prompt/catalog selection. [inferred]

### 8.3 Plugin-created agents and modes

The plugin API can create one-shot or conversational agents, connect them to a
parent thread, run them locally, on an Orb, or on a named runner, and register
custom modes visible beside built-ins. Static `@amp-agent-mode` comments help
clients discover modes before execution and warn when runtime registration
disagrees. [public] [registry]

Static-to-runtime consistency is a valuable compatibility check. It is not a
signature, capability manifest, or authority proof. A plugin that can define a
model, all tools, reasoning effort, UI, lifecycle continuation, and placement
is an executable agent package and must be governed accordingly. [inferred]

## 9. Tools, skills, MCP, and plugins

### 9.1 Tool runtime

The CLI exposes built-in, MCP, and plugin tools through one active catalog.
Settings can enable glob-selected tools or disable exact/built-in identities.
The bundle includes tool schema validation, shell/process code, browser/media
paths, background work, and specialized agents. The exact active catalog is
mode- and plugin-dependent and discoverable with `amp tools list`. No
authenticated catalog was enumerated in this audit. [public] [cli-binary]

Tool names are not authority identities. OpenAgents should bind advertised
schema, implementation digest, provider, account, execution placement,
containment profile, egress, and generation into an exact catalog receipt.

### 9.2 Skills and lazy MCP

Amp loads project and user Agent Skills, including compatible Claude Code skill
locations. A skill can contain `mcp.json`. Its server starts with Amp but its
tools remain hidden until the skill is loaded. This reduces model-context noise
without delaying process startup. [public]

The progressive-disclosure pattern is good. Starting executable MCP code before
activation is a larger trust decision than merely hiding its tool descriptions.
OpenAgents should defer both process admission and tool projection unless a
skill explicitly requires warm startup under an approved immutable generation.
[inferred]

Workspace MCP configuration requires approval. User-global servers and servers
passed on the command line do not. OAuth credentials are stored locally and
refreshed. The bundle's MCP client includes stdio/HTTP handling, prompt and
sampling capabilities, launcher ecosystems, and retry logic. [public]

### 9.3 Plugins are the real operating system extension

Amp plugins are TypeScript and can:

- subscribe to agent, tool, and lifecycle events.
- allow, reject, ask about, or delegate tool calls.
- append a follow-up and continue after an agent turn.
- add commands and change their availability.
- add tools and synchronized UI.
- ask an Amp model for a policy judgment.
- define selectable modes. And
- create/run conversational agents on local, runner, or Orb executors.

The API is inspired by pi's extension API, and the binary retains a direct link
to pi-mono documentation. Amp adds distributed threads, clients, placement, and
synced UI above that pattern. [public] [cli-binary]

The bundle names a plugin runtime and worker/process facilities, but public
evidence does not establish a fail-closed OS sandbox or capability broker for
plugin filesystem, network, process, credential, thread, spend, or UI access.
TypeScript types constrain well-behaved callers. They do not contain hostile
code. [cli-binary] [limitation]

## 10. Permission, policy, and containment

Amp's manual says plainly that tools do not ask for approval by default. It
warns that repositories, MCP servers, and external inputs can influence the
agent and recommends a policy plugin or an isolated development environment
for untrusted sources. [public]

The current permission system supports ordered rules over tool identity and
arguments with `allow`, `reject`, `ask`, and `delegate`, optionally scoped to
main thread or subagent. Guarded-file allowlists and a dangerous allow-all mode
exist. The bundle also exposes allow-once/session/every-session decisions and a
hidden dangerous bypass path. [public] [cli-binary]

This is flexible authorization UX, but the evidence supports no ordinary local
Seatbelt, bubblewrap, Landlock, AppContainer, VM, or container boundary. Orbs
and E2B sandboxes are separate remote placements, not proof that the laptop
worker is contained. [public] [limitation]

The key law is:

```text
plugin permission decision = whether Amp intends to invoke an effect
OS/guest containment       = what the invoked code can actually reach
effect receipt             = what enforcement and outcome were observed
```

An AI-assisted permission plugin is useful for explanation or escalation but
must not be the mandatory deny boundary. Model failure, injection, ambiguity,
or plugin startup timeout cannot convert policy into allow. [inferred]

## 11. CLI, SDK, and protocol surface

### 11.1 Interactive and headless

The same executable supports:

- interactive TUI.
- `-x/--execute` final-answer mode.
- Claude Code-compatible stream JSON.
- optional thinking blocks outside that compatibility schema.
- multi-message JSONL input with explicit steer.
- thread continuation and raw/Markdown/JSON export.
- review mode.
- runner-only `--no-tui` operation. And
- SDK-spawned execution.

This is the right convergence: terminal, automation, and remote execution are
host modes over one thread product. [public] [cli-binary]

### 11.2 Stream JSON is a compatibility projection

Amp intentionally approximates Claude Code's stream schema. Child messages use
`parent_tool_use_id`. Final result waits for children. Thinking is an explicit
non-compatible extension. Compatibility reduces integration friction, but a
foreign transcript schema should not become the canonical internal event
algebra. Amp's richer thread actors, plugins, modes, placement, remote control,
and review states cannot be losslessly expressed as ordinary Claude messages.
[public] [inferred]

### 11.3 SDK lifecycle

The TypeScript SDK publishes execute streaming, thread APIs, permissions, MCP,
skills, and CLI installation/version compatibility helpers. It is a process/
service client around Amp, not an independent local engine. The package is
commercially licensed and its repository metadata points at the unavailable
core repository. [registry]

OpenAgents should keep its canonical protocol public and generated even when a
convenience SDK spawns a binary. Client compatibility must be derived from an
exact protocol/component ledger, not only from a semver helper inside a closed
release train.

## 12. IDE integration and client convergence

Amp deliberately killed its large editor extension and Tab completion. The
current product is CLI-first and connects to VS Code-family editors, JetBrains,
Neovim, and Zed to receive open-file, cursor, selection, diagnostics, and edit
integration. `--no-ide` disables the default connection behavior. [public]

The public `amp.nvim` bridge uses a local WebSocket protocol and exposes buffer
state, cursor/selection, edits, and LSP diagnostics through a companion plugin.
That is a narrow, inspectable example of keeping editor-native state in the
editor while the agent loop stays in the CLI. [source]

The general risk remains minimization. The CLI help says open IDE file and text
selection are automatically included with every message when IDE integration
is active. The serving bridge should enforce workspace bounds, selection caps,
sensitive-file policy, peer identity, generation, and explicit visibility. A
client-side filter after a same-user socket is not enough. This audit did not
exercise or fully recover those boundaries for every supported editor.
[cli-binary] [limitation]

## 13. Remote control, runners, and Orbs

### 13.1 Remote control

A live CLI thread can be continued from ampcode.com on desktop or mobile.
Passkey reauthentication can be enabled by a user or required by an enterprise
workspace for remote control. This is a strong product realization of “local
executor, remote controller”: the browser does not need the repository or
local shell credential to send the next thread command. [public]

The unresolved protocol questions are replay and authority: exact client
identity, command idempotency, offline queues, concurrent controller ordering,
revocation latency, generation fencing, and what happens when the local worker
dies after an external effect but before acknowledgment. [limitation]

### 13.2 Runners

Any interactive TUI can accept remotely created work when the default-off
`amp.remoteThreadCreation.enabled` setting is enabled. `amp --no-tui` starts a
runner-only process in the current directory. A stable runner ID can target it.
This turns an existing developer machine into a placement target without a
separate daemon product. [public] [cli-binary]

That simplicity is useful, but a directory and runner ID are not a complete
WorkContext. OpenAgents needs repository revision, dirty-state policy,
identity/account, authority profile, containment, network, secrets, capacity,
lease/fence, component generation, and evidence destinations bound before a
remote command is admitted.

### 13.3 Orbs

Orbs are Amp-managed remote machines. The CLI manages portal URLs, long-lived
systemd services declared in `.amp/services.yaml`, health, logs, pause/resume,
and short-lived audience-bound OIDC tokens. This is more than “run the CLI over
SSH”. It is a managed agent-computer product with service lifecycle. [public]

The public security page names E2B as a sandbox subprocessor that sees code when
sandboxes are used. It does not publish an Orb image, attestation, isolation,
tenant, secret-injection, network, snapshot, or destruction contract detailed
enough to treat an Orb as verified containment. [public] [limitation]

## 14. Data flow, privacy, and security

### 14.1 “No full clone” is not “no code storage”

Amp says its service does not clone, index, or store an entire codebase as a
standing corpus. It also says a thread can store snippets or entire files used
as context, messages, model responses, tool results, and attachments. Multiple
model providers receive partial code. E2B sees code for sandbox use. Pierre
stores Git repositories used for plugins/documents. These statements can all be
true, but only if “repository copy,” “selected thread content,” and “plugin
artifact repository” remain separate data classes. [public]

### 14.2 Provider and subprocessor fan-out

The current security reference lists Anthropic, OpenAI, xAI, Meta, Gemini
Enterprise, Amazon Bedrock, Baseten, and Fireworks as possible inference paths
for partial code. Parallel handles web retrieval without code. WorkOS and
Stripe handle identity and billing. [public]

Model routing therefore changes privacy properties as well as quality and cost.
An exact turn should expose provider, region, BYOK/platform key, zero-retention
class, cache window, training choice, and content categories before execution,
not only after opening a billing breakdown. [inferred]

### 14.3 Retention, ownership, and sharing

The security reference says deleted thread data is removed within 30 days.
Non-enterprise threads are deleted with account deletion under the policy.
enterprise workspace threads belong to the enterprise and persist when a user
leaves. Joining a workspace with the same account can place existing personal
threads under workspace visibility unless the user separates accounts or
changes visibility. [public]

These are consequential ownership semantics. A local repo can be private while
its work thread is workspace-visible and administrator-accessible. OpenAgents
should make data owner, controller, visibility, retention, and deletion state
visible per thread rather than infer them from the current login.

### 14.4 Secret handling

Amp documents local credentials under `~/.local/share/amp/secrets.json` and MCP
OAuth state under separate local paths. The binary includes a native keyring
binding and recognizes API and MCP secret classes. Documentation says the
client makes a best effort to avoid known sensitive files and uses low-level
secret-pattern redaction before thread/cache/provider/server transmission, but
nonstandard, encoded, obfuscated, or unknown secrets may pass through. A leaked
secret must be rotated. Changing thread visibility does not erase it from the
service. [public] [cli-binary]

This is appropriately candid. Pattern redaction is a last defense, not a
capability boundary. Sensitive file serving, command output, MCP results,
screenshots, and plugin access need source-side minimization and explicit
declassification.

### 14.5 Telemetry and audit

The bundle records structured events and posts batched JSON to an Amp telemetry
endpoint, compressing larger payloads and attaching auth when present. It
contains OpenTelemetry libraries and local logging controls. `DO_NOT_TRACK`
appears in the bundle, but this audit did not trace enough call sites to claim
the exact Amp telemetry opt-out behavior. [cli-binary] [limitation]

Amp says its internal application audit logs are retained at least 30 days and
are available to enterprise customers on request, while thread history itself
acts as an audit trail for prompts, model output, tools, and attachments. A
transcript is useful forensic evidence. It is not an admitted effect receipt or
proof that the recorded tool outcome matches external state. [public]

## 15. Product change velocity and compatibility risk

The Chronicle is unusually direct about removal. Recent changes killed or
replaced:

- the old editor extension.
- Amp Tab completion.
- discoverable public threads.
- slash/custom commands in favor of the palette and skills.
- fork and TODO UI.
- walkthrough. And
- smart/deep/rush/large as primary modes, replaced by low/medium/high/ultra.

At the same time Amp added actor-backed distributed agents, runners, Orbs,
plugins, custom modes and agents, web/mobile review, and arbitrarily large
thread reading. [public]

This is evidence of real product focus, not merely instability. The risk is
that durable work outlives rapidly changing control and extension surfaces.
Every saved thread, plugin, SDK client, automation stream, permission rule,
runner, and model mode needs explicit schema/component compatibility and a
deprecation outcome. “No backcompat” is acceptable for a disposable experiment.
it is not by itself a recovery policy for enterprise work history. [inferred]

## 16. Security and architectural assessment

### 16.1 What Amp gets exceptionally right

1. **The thread is cross-surface work, not a chat transcript skin.** Search,
   references, web/mobile control, review, staging, raw export, and remote
   placement deepen the same object.
2. **Queue, steer, and interrupt are distinct.** The semantic choice exists in
   both the interactive and automation clients.
3. **Long history remains evidence.** Compaction is not allowed to erase the
   original events used by `read_thread`.
4. **Model diversity is composed deliberately.** Oracle changes model family
   when needed, and retrieval/review/media/compaction are routed separately.
5. **Subagent limitations are stated plainly.** The docs do not pretend that a
   final child summary is a live collaborative graph.
6. **The CLI is a real client and worker.** Interactive, stream, SDK, runner,
   editor, and remote-control modes converge on one thread product.
7. **Plugins can become real product extensions.** UI, lifecycle events,
   commands, modes, agents, and placement are one coherent API rather than a
   pile of prompt files.
8. **Data-flow documentation names subprocessors and code exposure.** The
   security page is more specific than generic “your code is secure” claims.

### 16.2 Where the design is fragile or under-evidenced

1. **Default-open tool execution carries the whole prompt-injection burden.**
   Optional policy and external isolation do not make local host effects safe
   by default.
2. **Cloud thread truth is opaque.** No public canonical event/admission/replay
   contract establishes exact durability or portable recovery.
3. **Plugins combine too many authorities.** Code, tools, UI, model calls,
   lifecycle continuation, agents, thread access, and remote placement need a
   stronger capability and isolation story.
4. **The supply chain lacks independent release proof.** Same-origin checksum,
   disabled minisign, and a locally invalid ad-hoc signature are below the bar
   for an auto-updating privileged coding agent.
5. **Mode names hide changing execution facts.** Quality routing is a product
   strength. Reproducibility and retention transparency suffer without exact
   receipts.
6. **Workspace sharing changes the privacy unit.** A thread may be visible to
   teammates/admins even when the local repository's permissions suggest a
   narrower audience.
7. **Rapid deletion creates compatibility debt.** Durable threads and external
   plugins/SDKs need migration laws even if UI features are intentionally
   ephemeral.
8. **The core is closed.** Binary inspection cannot replace tests, source
   history, review, or a public protocol boundary.

## 17. Comparison with the reference set

| Concern | Amp | Closest or stronger reference | OpenAgents consequence |
| --- | --- | --- | --- |
| Core product unit | Cloud-backed searchable Thread | Codex Thread/Turn/Item. OpenCode V2 durable aggregate | Keep stable work identity but publish exact event/admission schemas |
| Client convergence | CLI, stream, SDK, web/mobile, IDE, runner/Orb | Factory daemon clients. Codex app-server | One engine protocol, generated and public, across every host |
| Durable remote execution | Actor/WebSocket thread fabric and runners | Factory daemon. OpenCode V2 managed service | Add leases, fencing, idempotency, replay, and observed receipts |
| Queue versus steer | Explicit UI and JSONL semantics | OpenCode V2. T3 gap analysis | Adopt directly above durable command identities |
| Very long history | Search-agent rereads originals after compaction | Claude JSONL. Codex JSONL+SQLite | Summaries orient. Original accepted events remain evidence |
| Model composition | Opinionated routing plus Oracle/specialists | T3 provider harness. Grok/Codex explicit model state | Route semantically but receipt exact model/provider/policy facts |
| Subagents | Isolated, final-summary return, limited steering | Codex explicit agent graph. Claude sidechains | Preserve complete graph/transcripts and delivery state |
| Review fan-out | One subagent per Markdown check | OpenAgents Assurance Manifest lanes | Treat outputs as observations, not self-minted verdicts |
| Plugins | Tools, events, UI, modes, agents, placement | OpenCode plugin generations. Executor authored functions | Immutable signed capability generations in isolation |
| Permissions | Plugin rules. No approval by default | Codex sandbox/exec policy. Factory whole-process sandbox | Default fail-closed containment, separate from approval UX |
| IDE | Narrow CLI bridge. Large extension removed | Command Code bounded socket. OpenCode generated server client | Host-owned typed context with source-side minimization |
| Remote work | Local runner, managed Orb, web/mobile control | T3 environments. Factory remote computers | Bind full WorkContext, authority, containment, and generation |
| Persistence visibility | Server Postgres plus raw actor export | Codex local log/index. OpenCode V2 admission/replay/projection | Local portable evidence log beside optional sync |
| Release | Bun binary, checksum, auto-update. Invalid ad-hoc signature | T3/OpenCode build/update. OpenAgents signed manifest | Signed provenance, compatibility, staged activation, rollback |
| Learned preference | No Command Code-style evidence found | Command Code Taste | Do not mislabel thread retrieval as preference learning |
| Change policy | Explicitly removes legacy surfaces | Grok/Claude/Codex compatibility layers | Delete aggressively only with state/plugin/protocol migrations |

## 18. What OpenAgents should adapt

### 18.1 Make work history agent-readable without making it authority

Build a bounded reader over exact thread/work-unit history that:

- searches semantically and by typed fields.
- returns source event refs, not only prose.
- includes later supersession, revert, review, and acceptance state.
- treats tool calls as attempts until an observed outcome is admitted.
- uses compaction only for orientation. And
- never turns retrieval confidence into effect or release authority.

### 18.2 Put queue, steer, and interrupt in the canonical command schema

Each must carry a stable command id, target thread/turn generation, admission
time, origin client, ordering key, requested delivery boundary, and terminal
outcome. The UI can remain as simple as Amp's Enter/double-Enter/double-Escape
gesture while Sync and Runtime Gateway preserve exact semantics.

### 18.3 Treat the terminal as a worker and client, not the product authority

Amp validates the convergence of interactive CLI, headless stream, SDK, remote
controller, and runner. OpenAgents should expose those hosts over one public
Runtime Gateway contract while keeping Desktop/mobile the primary product
surfaces and workroom authority typed outside terminal rendering.

### 18.4 Add specialist diversity intentionally

Oracle's different-model rule is worth adapting. Planning, code retrieval,
review, media, and verification may use different model families when that
improves independence. The selector must be typed and semantic, with exact
provider/model/prompt/catalog/cost/retention receipts. A second model remains
advice unless admitted verification evidence says otherwise.

### 18.5 Add a bounded thread-reader role

Long history should be read by a retrieval role whose capabilities are limited
to relevant private-safe evidence and whose output carries citations into the
canonical log. It should not receive ordinary shell, publication, credential,
or settlement authority merely because it is a subagent.

### 18.6 Make review checks a typed manifest

Preserve Amp's one-check-per-agent ergonomics. Compile Markdown/user intent into
an exact Assurance Manifest with source revision, environment, tool catalog,
budgets, adapter, observation schema, and evidence destination. Fan-out cannot
self-admit or self-accept.

### 18.7 Make runners ordinary typed placements

A named local runner, Cloud VM, or future managed workroom should implement one
placement contract. Admission binds the exact WorkContext, authority profile,
containment, component ledger, lease/fence, capacity, secret refs, egress, and
receipt channel. Remote creation remains default-off per device/workspace.

### 18.8 Let extensions deepen the product under one law

Amp proves that plugins can add more than tools: commands, UI, lifecycle,
modes, and agents belong to one extension grammar. OpenAgents should support
that breadth only through immutable publisher identity, reviewed source or
provenance, declared capabilities, isolated runtime, scoped accounts, bounded
resources, generation fencing, explicit activation, and rollback.

### 18.9 Preserve data-flow candor

For each work unit, display local reads, selected uploaded context, inference
providers, thread storage, visibility/administrator access, retention,
telemetry, sandbox placement, plugin artifact storage, and training state as
separate facts. “Local,” “private,” “ZDR,” and “no codebase copy” must never be
umbrella claims.

### 18.10 Borrow the product velocity, not compatibility denial

Remove weak features when evidence says they should go, but version durable
state and external contracts. Every retired command, mode, plugin API, stream
field, or client must have a migration, explicit incompatibility, export path,
and deletion gate.

## 19. What OpenAgents should reject

1. **No default tool execution on the host.** Default profiles must combine
   narrow authorization with observed fail-closed containment.
2. **No cloud transcript as sole thread authority.** Retain portable local
   accepted events and receipts with optional encrypted synchronization.
3. **No closed canonical protocol.** The Runtime Gateway schemas, authority
   law, and compatibility matrix remain public and generated.
4. **No plugin privilege bundle.** UI, tools, model calls, lifecycle
   continuation, thread access, placement, filesystem, egress, and spend are
   separately declared capabilities.
5. **No same-origin checksum as release authority.** Require publisher-signed
   manifests, platform trust, provenance/SBOM, compatibility, immutable
   candidates, staged activation, retained rollback, and receipts.
6. **No model mode without exact execution identity.** Friendly names cannot
   hide provider, model revision, prompt/catalog generation, cost, or data
   policy in evidence.
7. **No compaction as history.** A summary cannot replace source events or mint
   current-state truth.
8. **No workspace join that silently changes old-thread disclosure.** Require
   preview, explicit disposition, and a receipt for ownership/visibility
   migration.
9. **No unlisted-link ambiguity.** Say internet-readable when possession of a
   URL is the only access condition.
10. **No AI permission judge as mandatory policy.** Model/plugin unavailable,
    malformed, injected, or timed out must fail closed at mandatory denies.
11. **No child summary as child history.** Preserve complete navigable child
    topology, execution, delivery, review, and acceptance state.
12. **No “no backcompat” shortcut for durable work.** Product UI may evolve.
    accepted state needs explicit migration and recovery.

## 20. Recommended OpenAgents sequence

1. Extend the canonical Runtime Gateway command model with explicit queue,
   steer, interrupt, acknowledgment, and terminal outcome identities.
2. Add a provider-neutral, read-only thread-history query interface over exact
   accepted events with supersession/revert/acceptance refs.
3. Implement a bounded thread-reader agent that returns citations and cannot
   widen authority.
4. Project the same command/history semantics into Desktop and mobile before
   adding a new remote CLI controller.
5. Bind Pylon/local/Cloud workers to one typed placement lease and component
   generation, then add replay-safe remote creation.
6. Compile review checks into Assurance Manifests and run them as observation-
   only fan-out lanes.
7. Add explicit specialist/model-diversity policy with exact routing and data-
   class receipts.
8. Define one extension capability manifest for commands, UI, tools, events,
   agents, models, and placement before widening the plugin surface.
9. Keep signed component-ledger and rollback gates ahead of self-update UX.
10. Add a thread privacy/ownership panel that makes Sync, providers,
    visibility, administrator access, retention, and deletion separately
    legible.

These are product and architecture inputs. They do not supersede the current
Sol roadmap, ProductSpec, accepted invariants, or implementation evidence.

## Final assessment

Amp's important innovation is not that it can edit code from a terminal. It is
that a coding thread has become a distributed collaboration and execution
object: model-routed, searchable, cross-referenceable, remotely controllable,
reviewable on other clients, and placeable on another machine. The dedicated
thread reader, explicit steer semantics, different-model Oracle, and plugin-
created agents show a coherent product built around that object.

The architecture also demonstrates what happens when the thread becomes the
service. Code and tool evidence move into a cloud record. Workspace visibility
and administrator access become repository-security concerns. Local workers
execute without approval by default. Remote routing changes model and privacy
facts. And an extremely powerful extension plane sits on closed runtime and
release seams. The current direct-install chain and invalid macOS signature are
especially weak for a privileged auto-updating binary.

OpenAgents should take the thread fabric and strengthen its laws. The result
should retain exact local events and receipts, publish its protocol, separate
summary from evidence, separate permission from containment, fence every
worker and extension generation, disclose every data plane, and make release
identity independently verifiable. That would preserve Amp's most compelling
product insight without making a vendor-controlled transcript, plugin, model
router, or updater the authority for accepted work.

## Primary source map

### Local and package artifacts

- `/Users/christopherdavid/.amp/bin/amp` at
  `0.0.1784247472-g76909f`, SHA-256
  `521a9473876d488a5f05f9ea8fca20c9686d3321422dea5f3f0283576f4d9bdc`
- `@ampcode/cli@0.0.1784247472-g76909f`
- `@ampcode/cli-darwin-arm64@0.0.1784247472-g76909f`
- `@sourcegraph/amp@0.0.1784247472-g76909f`
- `@ampcode/sdk@0.1.0-20260605144103-g77da114`
- `@ampcode/plugin@0.0.0-20260717002806-g76909f1`

### Current public source

- <https://github.com/ampcode/amp.nvim>
- <https://github.com/sourcegraph/amp-examples-and-guides>
- <https://github.com/badlogic/pi-mono>

### Official Amp references

- <https://ampcode.com/manual>
- <https://ampcode.com/manual#why-amp>
- <https://ampcode.com/models>
- <https://ampcode.com/chronicle>
- <https://ampcode.com/security>
- <https://ampcode.com/privacy-policy>
- <https://ampcode.com/install.sh>
- <https://ampcode.com/news/npm-package-changes>
- <https://ampcode.com/news/agents-everywhere>
- <https://ampcode.com/news/read-bigger-threads>
- <https://ampcode.com/news/end-of-public-threads>
- <https://ampcode.com/manual/sdk>
- <https://ampcode.com/manual/sdk/typescript>
- <https://ampcode.com/manual/plugin-api>
- <https://www.npmjs.com/package/@ampcode/cli>
- <https://www.npmjs.com/package/@ampcode/sdk>
- <https://www.npmjs.com/package/@ampcode/plugin>
