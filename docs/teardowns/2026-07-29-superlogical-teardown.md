# Superlogical Teardown — 2026-07-29

This is a read-only product, architecture, and team-background analysis of
Superlogical at its public announcement. Superlogical has not published a
product build, source repository, protocol, package, security design, license,
price, or release date. This document therefore separates announced intent
from forecasts and tests that could disprove those forecasts.

## TL.DR

Superlogical is not announcing only a better `tmux`. It is proposing a durable
session as the common object for interactive work, automatic work, and
production work. The first product will organize terminal blocks in a
long-lived session. A user will be able to close a client, connect from a
different device, and continue the same session. The announced clients include
the web and native macOS and iOS applications. Live sharing is part of the
initial product thesis. [announcement]

The larger thesis is more important than the first interface. Superlogical
wants one session to retain relevant context, structured data, actions, and
history. Software can drive the session, while people can see and control it.
The announced plan has three stages:

1. Build a high-quality multiplexer.
2. Make all parts composable.
3. Make the system safe and operable in production. [announcement]

The founding team makes several implementation directions unusually likely.
These are forecasts, not source facts:

- A shared terminal engine will probably use `libghostty`, or a close Ghostty
  derivative, for terminal parsing and state. [forecast: high]
- A local or remote session service will probably own the PTY and process.
  Native and web applications will attach as clients. [forecast: high]
- The Apple applications will probably use native UI, with a Swift client and
  careful touch, keyboard, split, selection, and scroll behavior. [forecast:
  high]
- A hosted control plane will probably handle discovery, identity, sharing,
  and remote access. Workload bytes can still remain on user-controlled hosts.
  [forecast: medium]
- The second stage will probably add typed events, actions, APIs, and new block
  kinds above terminal byte streams. [forecast: high]
- The third stage will probably add enterprise identity, policy, audit,
  approvals, retention, and production access controls. [forecast: medium]
- Some local components will probably be open source, while hosted
  collaboration and production controls form the paid product. [forecast:
  medium]

The combined prior work is the strongest basis for these forecasts. Mitchell
Hashimoto created Ghostty and helped create a family of open developer tools
with API and ecosystem surfaces. Jack Pearkes helped build and operate that
company, and he later described a BYOC system with long-lived CLI agents behind
web and iOS clients. Alasdair Monk and Hector Simpson already ship Echo, a
native iOS and iPadOS SSH and Mosh client that uses Ghostty. Monk also
publishes a native tab and split-pane component. [founder-source]

This combination suggests a likely first system:

```text
native macOS / native iOS / web / software client
                         |
          identity, discovery, sharing, control
                         |
       session owner on a local or remote host
                         |
       PTYs, processes, terminal state, history
                         |
     shells / agents / jobs / services / production
```

The central OpenAgents decision is: **track Superlogical as a high-relevance
product thesis and future interoperability peer. Do not adopt an architecture,
protocol, dependency, or security claim before public artifacts exist.** The
important shared idea is the durable work session. OpenAgents should retain its
own stronger separation between transcript, command admission, effect,
containment, evidence, verification, and acceptance.

The OpenAgents opportunity is also larger than a defensive comparison.
Superlogical describes a multiplexer for all work. OpenAgents and Omega can
become the **application for all work**: one native work environment, one typed
control system, and one API and SDK ecosystem. The repository already contains
many of the required parts. The present gap is that these parts do not yet form
one complete product projection. Section 13 defines this counter-position and
a bounded way to start now.

## 1. Snapshot, provenance, and limits

### 1.1 Audited public corpus

The audit fetched the public corpus on 2026-07-29. HTML digests identify the
exact bytes that the audit read. These digests do not make a mutable web page
an immutable release.

| Artifact                                                                                                     | Point-in-time identity                                                                                          | What it establishes                                                                                                                      |
| ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| [Superlogical announcement](https://www.superlogical.com/)                                                   | Fetched `2026-07-29T15:58:18Z`. HTML SHA-256 `c5cf30725613a33d2c32dd2e9476e91d7f2771758fb5d2f79dd25b985813a8ad` | Company thesis, three-stage plan, initial clients, sharing claim, team, funders, private beta, and unspecified future OSS releases       |
| [Superlogical press kit](https://blob.superlogical.com/public/Press-Kit/PressKit.zip)                        | ZIP SHA-256 `3868ac8edfcb2fcfa221a1876009998a8f8c497f0aa650b92381604962970d63`. 20 archive entries              | Logos and three team photographs only. No product image, protocol, source, security design, or product specification                     |
| [Launch post](https://x.com/mitchellh/status/2082489600715661389)                                            | Public post shown in the owner-provided screenshot on 2026-07-29                                                | Mitchell Hashimoto started the company and described the terminal multiplexer as the foundation for a larger vision                      |
| [Mitchell Hashimoto](https://mitchellh.com/)                                                                 | HTML SHA-256 `b735695df7a7cc5c9ce4146056134079738fe36deca628c2dca420970cf16b97`                                 | Ghostty and HashiCorp product and operating background                                                                                   |
| [`libghostty` plan](https://mitchellh.com/writing/libghostty-is-coming)                                      | HTML SHA-256 `f104ba9534c186847d2d26c42433e9dafa6c82cd4276943b3d946511f269e127`                                 | Library-first terminal architecture and its portability goal                                                                             |
| [Ghostty source](https://github.com/ghostty-org/ghostty/tree/6ad1fe7d8cbda36c77b337a96c9bea8a77883699)       | Public `main` at `6ad1fe7d8cbda36c77b337a96c9bea8a77883699` when fetched                                        | Current public `libghostty` availability and Ghostty architecture. It is not Superlogical source                                         |
| [Jack Pearkes at HashiCorp](https://www.jackpearkes.com/posts/goodbye-letter-hashicorp)                      | HTML SHA-256 `41cc09168e1af258bce2399f87ac868c0cc2182e6b3f9fc528fc1a2e47eca9d3`                                 | Early APIs, admin UI, remote team, product creation, enterprise security, operations, and company-scale experience                       |
| [Jack Pearkes AI assistant essay](https://www.jackpearkes.com/posts/this-is-my-ai-assistant)                 | HTML SHA-256 `37aef298000c83be9112a096d3f4e3363c5a881ba869e06b6ebb443585fbd4d6`                                 | Prior BYOC worker, persistent CLI agent, web, iOS, structured UI, and sandbox design ideas                                               |
| [Jack Pearkes LLM team essay](https://www.jackpearkes.com/posts/how-will-llms-change-the-shape-of-our-teams) | HTML SHA-256 `b4e1c354352edeff7d333f93aad7dcbfe2610c3c142ea406a693c0f9ac9d9c95`                                 | His view that system design, interfaces, guardrails, and validation become more important with agents                                    |
| [Alasdair Monk](https://www.alasdairmonk.com/)                                                               | HTML SHA-256 `6d4058bc0da75a6d12e65aacba71a97abe492f44b9239847c227dfc4adc78b1a`                                 | Developer-tool design, native Apple application, SSH, and split-pane background                                                          |
| [Hector Simpson](https://hector.me/)                                                                         | HTML SHA-256 `55cd8489c910793d62f82e254938744be9af9298cfe2bfca0e264a6da76bfa07`                                 | Developer interface design and implementation background                                                                                 |
| [Echo](https://replay.software/echo)                                                                         | HTML SHA-256 `9fa5369cdcd1f89eef1737a7b6054caa19217c085b98fd5670eb4dcdeb792d65`                                 | Existing native iOS and iPadOS SSH and Mosh product from Monk and Simpson, with Ghostty, mobile agent use, and multiple terminal windows |
| [Bonsplit](https://bonsplit.alasdairmonk.com/)                                                               | HTML SHA-256 `972e048aac1816ddfdc8e5b02375b12bdea1a65296926256247221a7eb3e2df4`                                 | Existing native macOS tab and split-pane library from Monk, including external geometry and state APIs                                   |

### 1.2 Evidence labels

- **`[announcement]`** — a claim on the official Superlogical site.
- **`[public]`** — a linked official public page or public repository.
- **`[founder-source]`** — public prior work or writing by a founder.
- **`[forecast: high]`** — the announcement and multiple prior-work signals
  support the forecast.
- **`[forecast: medium]`** — the product thesis supports the forecast, but
  several materially different designs remain possible.
- **`[forecast: low]`** — a plausible path with little direct support.
- **`[inferred]`** — a conclusion from several observations, not a company
  statement.
- **`[limitation]`** — a fact that the available corpus cannot establish.

### 1.3 Audit limits

There is no public Superlogical codebase to inspect. The audit did not join the
mailing list, submit the hiring command, access a private beta, contact the
team, inspect private DNS or certificate data, or infer hidden endpoints. It
did not treat source code from Ghostty or Bonsplit as Superlogical code.

The official site is one launch page. The press kit contains brand and team
assets only. The site had no linked documentation, security page, privacy
policy, terms, pricing, status page, protocol, SDK, package, or release
artifact at the audit time. Absence on the launch page does not prove that
internal designs or private implementations do not exist. [limitation]

Founder history can show capability and product taste. It cannot prove a
specific language, protocol, deployment model, license, business model, or
security property. Each forecast below therefore includes evidence that would
lower its confidence.

## 2. What Superlogical announced

### 2.1 The problem is fragmented work, not terminal windows

The announcement divides software work across these locations and modes:

- local machines.
- remote hosts.
- sandboxes.
- services.
- production systems.
- interactive human work.
- CI and background work.
- parallel agent work. [announcement]

The team says these activities belong to one related body of work. Current
tools separate interactive interfaces, automatic jobs and logs, and production
controls. AI increases the cost of this split, but did not create it.
[announcement]

This framing makes the terminal multiplexer a wedge, not the complete market.
The durable session is the proposed missing layer. It should span applications
and environments, provide relevant context, expose structured data and
actions, preserve history, accept software control, and remain visible to
people. [announcement]

### 2.2 The first product contract is specific

The first release has five explicit properties:

1. It organizes multiple terminal blocks in one long-lived session.
2. The session survives application closure.
3. A user can reconnect from another device and continue.
4. Web, native macOS, and native iOS clients can access the session.
5. Live sharing with other people is present from the start. [announcement]

The announcement also calls out scrollback, selection, and scrolling. It says
these operations will behave natively. These details are not minor. They show
that the first product must own terminal state and client interaction, not only
place a prettier frame around an existing `tmux` client. [announcement]
[inferred]

The site does not announce Linux, Windows, Android, SSH, Mosh, offline mode,
self-hosting, end-to-end encryption, session recording, search, an API, or a
price. It also does not state where the PTY or durable state will run.
[limitation]

### 2.3 The rotating headline is a scope map

The visible headline rotates through these phrases in the launch page source:

- local development.
- remote access.
- coding agents.
- background jobs.
- production applications.
- live debugging.
- sandboxes.
- shared terminals.
- incident response.
- humans and machines.
- operational history.
- multiplayer work. [announcement]

This list is broader than a terminal product. It is an early category map. It
connects the durable session to remote development, agent supervision, job
history, production operation, and collaboration. It does not announce that
all these product surfaces exist in the first beta. [inferred] [limitation]

### 2.4 “Multiplexer” has three possible levels

The term can describe three different systems:

| Level                   | Independent streams                                                    | Common interface                       | Superlogical evidence                            |
| ----------------------- | ---------------------------------------------------------------------- | -------------------------------------- | ------------------------------------------------ |
| Terminal multiplexing   | PTYs and terminal blocks                                               | panes, tabs, input, and terminal state | Explicit first product                           |
| Work multiplexing       | human turns, agent turns, jobs, logs, artifacts, and actions           | durable session and structured history | Explicit vision, no public protocol              |
| Production multiplexing | deployments, live services, incidents, approvals, and operator actions | policy-controlled operational session  | Explicit stage-three direction, no public design |

The first level can ship without solving the next two. The company thesis
depends on a session model that can expand without damaging the terminal
product. That is the main architecture challenge.

## 3. Founder-background signals

### 3.1 Mitchell Hashimoto

Hashimoto has two relevant bodies of prior work.

First, Ghostty separates a cross-platform Zig core from native GUI clients.
The current Ghostty repository describes `libghostty` as a zero-dependency C
and Zig library for terminal emulation. It supports macOS, Linux, Windows, and
WebAssembly. The behavior is mature, but the public API has not received a
stable version tag. The Ghostty macOS client uses Swift, AppKit, and SwiftUI
above this core. [public]

Second, HashiCorp products repeatedly used a narrow technical primitive, a CLI
workflow, an API, and an integration ecosystem. Hashimoto's public biography
names Vagrant, Packer, Consul, Terraform, Vault, Nomad, and Waypoint. That
history makes a small session core with broad adapters more likely than one
large client application. [founder-source] [inferred]

His Ghostty launch history also supports a long private beta and a high release
quality bar. Ghostty used almost two years of private beta before its public
1.0 release. Superlogical has already announced a private beta before a public
product. This is a process signal, not a release-date prediction.
[founder-source] [forecast: medium]

What would lower confidence:

- Superlogical publishes a terminal core that does not use Ghostty code or
  interfaces.
- the product uses a web terminal engine for every client.
- the first public build uses one client process with no library boundary.

### 3.2 Jack Pearkes

Pearkes describes early HashiCorp work that included APIs, an admin UI, remote
work, open-source product experiments, security assessments for banks,
incident communication, escalation procedures, support, and public-company
operation. This history supports the announcement's movement from a developer
tool toward a production system. [founder-source]

His 2025 AI assistant essay is a more direct product signal. It describes:

- a self-hosted or platform-hosted web process.
- a worker on user-controlled compute.
- web and native iOS clients.
- persistent CLI agents selected from a pool.
- WebSocket event paths.
- structured cards, files, progress, forms, and prompts.
- explicit concern about mTLS, secrets, authentication, and sandboxing.
  [founder-source]

This earlier design resembles the boundary that Superlogical now describes.
It connects native mobile control to long-lived command-line work on another
machine. It also keeps the worker data plane separate from hosted
connectivity. Superlogical has not said that it uses this design or code.
[inferred] [limitation]

What would lower confidence:

- all first-party sessions run only in Superlogical-managed compute.
- clients receive only video or terminal pixels from a centralized service.
- the product does not permit a host under user control.

### 3.3 Alasdair Monk and Hector Simpson

Monk and Simpson jointly operate Replay, a small studio for native Mac and iOS
applications. Their current Echo product is a native iOS and iPadOS SSH and
Mosh client. It uses Keychain, Metal, Face ID, hardware-keyboard support, and
Ghostty. Its page explicitly presents mobile monitoring and control of terminal
coding agents. [founder-source]

Monk also publishes Bonsplit, a SwiftUI macOS library for native tabs and split
panes. It supports focus movement, drag and drop, geometry snapshots, tree
snapshots, external divider control, and typed callback surfaces. These are
close to the visible needs of a native terminal multiplexer. [founder-source]

Simpson describes himself as an interface designer and developer. His prior
developer-product companies include Heroku, HashiCorp, and Vercel. The
Superlogical biography also names agent experience at Poolside and work at
Clearbit. Monk's background includes Poolside, Vercel, HashiCorp, Heroku, and
developer-focused native applications. [announcement] [founder-source]

This team composition makes interaction quality part of the product strategy.
Native selection, scroll, focus, keyboard, touch, and collaboration behavior
will probably receive first-order engineering attention. [forecast: high]

What would lower confidence:

- the announced native clients become wrappers around one web view.
- early releases expose terminal behavior without native selection, scroll,
  touch, keyboard, and accessibility integration.

### 3.4 The combined signal

The four founders cover four connected layers:

| Layer                      | Prior signal                                      | Likely Superlogical responsibility                                                  |
| -------------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Terminal engine            | Ghostty and `libghostty`                          | correct terminal state, parser, renderer interfaces, and portability                |
| Session and company system | HashiCorp products and operations                 | durable service boundaries, APIs, remote hosts, policy, and production operation    |
| Native remote client       | Echo and Bonsplit                                 | macOS and iOS terminal interaction, panes, touch, keyboard, and mobile continuation |
| Developer experience       | Heroku, HashiCorp, Vercel, Poolside, and Clearbit | coherent product language, collaboration flows, and agent-aware interfaces          |

This is an unusually complete team for the announced wedge. The main unknown
is not whether the team can build a terminal client. The unknown is whether a
terminal session can expand into a general work protocol without becoming a
second CI system, remote IDE, production console, and incident system at once.

## 4. Likely first-generation architecture

This section is a forecast. No diagram below represents observed
Superlogical code.

### 4.1 Likely authority split

```text
                   Superlogical control plane
          identity | device enrollment | discovery
          sharing  | presence          | policy
                           |
                    encrypted transport
                           |
              session service on work host
      session identity | PTY owner | process lifecycle
      input ordering   | scrollback | snapshots | history
                           |
       shell | agent CLI | build | service | production tool

        macOS app | iOS app | browser | software client
             render projections and submit actions
```

The session service must stay alive after a client disconnects. Therefore, the
PTY and child process cannot belong only to the GUI process. A separate daemon,
host service, or managed runtime must own them. [forecast: high]

The web and iOS clients also require a network path to that owner. The product
can implement this path through direct connectivity, a relay, a hosted proxy,
or a centralized workload service. The announcement does not choose between
these designs. [forecast: medium]

### 4.2 Likely domain model

A minimal expandable model will probably need these identities:

```text
account
  device
  host
    session
      block
        process generation
        terminal stream
        state checkpoint
        artifact or structured item
      member attachment
      action
      history cursor
```

The first release might expose only sessions and terminal blocks. Stable host,
process-generation, member, action, and cursor identities become necessary as
soon as reconnection, sharing, replay, and software control coexist.
[forecast: high]

A window, pane, and terminal process should not share one identity. One session
can have different layouts on a phone and a desktop. A block can survive a
client layout change. A restarted process is not necessarily the same process
generation. [inferred]

### 4.3 Terminal state and history

The durable system has at least four different data forms:

1. raw PTY bytes.
2. parsed terminal state.
3. periodic state checkpoints.
4. user-visible history and structured metadata.

Raw bytes support exact reprocessing but can be expensive to replay. Parsed
state supports fast rendering but changes with parser semantics. Checkpoints
bound replay cost. User history needs search, retention, redaction, and export
rules that terminal state alone does not provide.

`libghostty-vt` is a strong candidate for parsing and state because the current
library supports C, Zig, Windows, and WebAssembly consumers. It does not provide
tabs, splits, session management, configuration, or a complete GUI. Those are
consumer responsibilities. [public] [forecast: high]

The browser could use a WebAssembly build of the same state engine. It could
also receive server-generated state deltas. The first option improves parser
consistency across clients. The second option keeps canonical parsing on the
session host. Both designs remain plausible. [forecast: medium]

### 4.4 Input and resize arbitration

Live sharing creates conflicts that single-user terminal multiplexers can
avoid. The system must define:

- who can write input.
- whether multiple writers can type concurrently.
- how paste and composition events stay atomic.
- which client controls PTY rows and columns.
- how a phone and a desktop receive different view sizes.
- how focus, mouse, secure input, and clipboard actions work.
- what a read-only member can observe.
- what happens during disconnect and reconnect.

A likely first design will use one active writer and explicit handoff. It will
also choose one authoritative PTY size while other clients use viewports.
[forecast: medium]

What would lower confidence:

- a CRDT-like multi-writer input model.
- one independent PTY per client with synchronized command history.
- shared sessions that are view-only in the first beta.

### 4.5 Local and remote placement

The announcement names local machines, remote hosts, sandboxes, services, and
production. A placement-neutral session protocol is therefore likely. The same
client should attach to a session without assuming that the PTY runs on the
client device. [announcement] [forecast: high]

The plausible placement set is:

| Placement   | Session owner                | Main benefit                       | Main risk                                            |
| ----------- | ---------------------------- | ---------------------------------- | ---------------------------------------------------- |
| Local       | daemon on the user's machine | low latency and existing tools     | sleep, network addressability, and host compromise   |
| Remote host | installed host service       | stable compute and SSH replacement | enrollment, upgrade, firewall, and credential scope  |
| Sandbox     | short-lived worker           | isolation and reproducibility      | lifecycle, transfer, cost, and cleanup               |
| Managed     | Superlogical service         | simple setup and team control      | transcript custody, cost, region, and lock-in        |
| Production  | enrolled operator endpoint   | direct operational work            | privileged access, audit, approval, and blast radius |

The first beta may support only local and remote hosts. The broader placement
set is a later vision. [forecast: medium]

### 4.6 Hosted control plane and user-controlled data plane

Pearkes has publicly favored user-controlled compute for sensitive and
unbounded agent data while hosting connectivity separately. HashiCorp products
also often separate a control plane from agents on workload infrastructure.
These are strong historical reasons to expect a similar split.
[founder-source] [forecast: medium]

A possible system would let the hosted service know:

- account and device identity.
- host reachability.
- session metadata.
- sharing membership.
- policy and audit metadata.

The host could retain:

- raw terminal bytes.
- process environment.
- filesystem context.
- secrets.
- detailed command output.

This split is only a forecast. Remote rendering, history search, mobile push,
and collaboration can move more data into the hosted service. Superlogical has
not published a privacy or data-flow design. [limitation]

### 4.7 Native clients

The macOS client will probably use Swift with AppKit or SwiftUI above a shared
terminal core. The iOS client will probably reuse patterns from Echo for
Keychain, Metal, touch controls, hardware keyboards, and network roaming. The
web client may use `libghostty-vt` through WebAssembly or a compatible state
projection. [forecast: high for Apple clients, medium for the web engine]

Layouts will probably be per-client projections. A desktop can show many
blocks. A phone can show one focused block plus a session navigator. Shared
session identity and block identity should remain stable across both layouts.
[forecast: high]

### 4.8 Agent and software clients

The announcement says software can drive a session while people retain
visibility and control. That requires more than synthetic keystrokes. A useful
software client needs structured operations such as:

- create a session.
- start a block.
- send input.
- resize or focus a block.
- observe output with a cursor.
- attach metadata and artifacts.
- request or record an approval.
- interrupt or terminate a process.
- subscribe to lifecycle changes.
- export history.

The second stage will probably expose an API or SDK around these actions.
[forecast: high]

An agent can still run inside a terminal block. That path gives immediate
compatibility with Claude Code, Codex, Amp, OpenCode, Pi, and other terminal
agents. A richer adapter can later map native agent events into structured
session items. [forecast: high]

The main design risk is a false equivalence. PTY output does not prove that a
tool succeeded, a file changed, a deployment completed, or an approval was
valid. A production session needs typed outcome sources outside terminal
rendering. [inferred]

## 5. What “make everything composable” probably means

The phrase can point to several layers. Confidence falls as the scope expands.

### 5.1 Composable blocks

Terminal blocks will probably gain stable identity, independent lifecycle, and
movable layout. A user or API could create, group, split, move, share, and
archive blocks. [forecast: high]

Later block kinds could include:

- terminal process.
- job log.
- file or diff.
- service status.
- browser or preview.
- agent conversation.
- approval request.
- metric or trace.
- production console. [forecast: medium]

The announcement does not promise these exact types. The rotating headline and
structured-data language make non-terminal blocks likely. [inferred]

### 5.2 Composable events and actions

A terminal byte stream is easy to display and difficult to compose. A general
session will probably add typed records for lifecycle, member, input, output,
artifact, approval, and effect events. Software can then subscribe to one
block, transform an event, or create another action. [forecast: high]

A likely event envelope would contain:

```text
session + block + generation + sequence + actor + time
event kind + payload version + visibility + source
```

This shape is illustrative only. [forecast]

### 5.3 Composable context

The company wants relevant context by default. A durable session can collect
working directory, repository, branch, process, command, host, collaborator,
and artifact context. Agents can consume that context without asking the user
to reconstruct the work. [announcement] [inferred]

Default context also creates a major privacy risk. A production design must
bind every context item to purpose, audience, origin, retention, and export or
deletion behavior. “Relevant” cannot mean that every agent or collaborator
receives all retained session data. [inferred]

### 5.4 Composable integrations

The HashiCorp pattern suggests providers, plugins, or adapters around a stable
core. The Vercel and Heroku backgrounds suggest careful developer APIs and
deploy integrations. The team could expose:

- a host daemon API.
- a client SDK.
- event subscriptions.
- workload adapters.
- authentication integrations.
- policy hooks.
- export formats.
- terminal application extensions. [forecast: medium]

The announcement does not use the words plugin, provider, SDK, API, or MCP.
The first composability surface could be much narrower. [limitation]

## 6. What “safe and operable in production” probably requires

This is the least specified stage and the hardest part of the vision.

### 6.1 Identity and access

A production session needs separate identities for the user, device, software
client, host, session, process generation, and target environment. Sharing a
link or terminal token cannot safely stand in for all these identities.

Likely controls include:

- organization single sign-on.
- device and host enrollment.
- role-based session access.
- read, write, control, and administration roles.
- short-lived attachment grants.
- session and production-target policy.
- immediate revocation. [forecast: medium]

### 6.2 Audit and history

The product thesis treats history as a core feature. Production history must
distinguish what a person saw, what they requested, what the system admitted,
what a process attempted, and what the target observed. A screen recording or
terminal log alone does not establish these facts.

The system will probably add immutable audit records, retention rules, export,
and searchable session history. [forecast: medium]

### 6.3 Approvals and policy

Interactive people, CI, background jobs, and agents have different authority.
A common session needs one policy model that does not silently give an agent
the rights of the attached human. Production actions will probably gain
approval steps and policy evaluation. [forecast: medium]

The main negative signal would be a design where the session only inherits the
host user's credentials and records terminal output after the fact.

### 6.4 Secrets and workload identity

Persistent terminals tend to accumulate shell credentials, environment
variables, agent tokens, SSH material, and production access. Multi-device
sharing increases the exposure. A production-grade system must avoid copying
long-lived secrets into every client.

Likely directions include host-local credentials, brokered short-lived
credentials, secure-input modes, redaction, and policy-bound environment
injection. [forecast: medium]

### 6.5 Reliability

A durable session must define:

- process ownership after every client disconnects.
- crash and host-restart recovery.
- checkpoint and replay limits.
- duplicate input prevention.
- network partition behavior.
- writer fencing.
- history durability.
- version compatibility.
- session archive and deletion.

The first terminal beta can provide best-effort recovery. The production claim
will require explicit guarantees and failure states. [inferred]

### 6.6 Production containment

A session boundary is not a sandbox. A permission prompt is not process
containment. A remote host agent can still act with the complete authority of
its operating-system user. Superlogical will need a separate story for
workload isolation, egress, filesystem scope, resource limits, and production
target policy if it runs untrusted agent work. [inferred]

## 7. Probable roadmap

The company gives an order, not dates. The table forecasts product increments
within that order.

| Phase | Announced outcome                 | Probable concrete product                                                                                       | Confidence                               |
| ----- | --------------------------------- | --------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| 1A    | high-quality terminal multiplexer | session daemon, multiple terminal blocks, reconnect, scrollback, native selection and scroll, local persistence | High                                     |
| 1B    | web and native Apple access       | macOS, iOS, and browser clients over one session transport                                                      | High                                     |
| 1C    | sharing from the start            | invitations, member presence, read or write roles, active-writer handoff                                        | High for sharing, medium for exact roles |
| 2A    | make parts composable             | stable block, action, event, and artifact identities with an API                                                | High                                     |
| 2B    | software drives the session       | agent and automation client, subscriptions, structured actions, and lifecycle callbacks                         | High                                     |
| 2C    | more than terminals               | jobs, agent items, files, diffs, services, or other block adapters                                              | Medium                                   |
| 3A    | safe in production                | organization identity, device and host enrollment, policy, approvals, and audit                                 | Medium                                   |
| 3B    | operable in production            | retention, search, recovery, observability, compatibility, administration, and support                          | Medium                                   |
| 3C    | unified production work           | deploy, debug, incident, and service integrations within sessions                                               | Low to medium                            |

### 7.1 Expected private-beta shape

The smallest credible private beta probably includes:

- a macOS session host or daemon.
- a native macOS application.
- several terminal blocks.
- session restoration after client restart.
- one remote access path.
- an iOS or web read and control client.
- invitations or direct sharing.
- Ghostty-level terminal rendering quality. [forecast: high]

It may omit organization administration, public APIs, Linux and Windows GUI
clients, managed compute, production policy, and general block types.
[forecast: medium]

### 7.2 Expected open-source boundary

The site promises “OSS releases along the way,” but it does not name a project,
license, or scope. [announcement]

Plausible open components include:

- terminal and session protocol types.
- a host daemon.
- client libraries.
- selected Ghostty improvements.
- example integrations.
- portable session formats. [forecast: medium]

Plausible hosted components include:

- account and organization service.
- discovery and relay.
- sharing and presence.
- policy and audit administration.
- managed session placement.
- support and enterprise controls. [forecast: medium]

The Ghostty license does not imply a Superlogical license. The company can use
or contribute to MIT-licensed Ghostty while keeping its session and hosted
systems under different terms. [public] [limitation]

## 8. Product positioning

### 8.1 The wedge

The wedge is a terminal multiplexer with three immediate differences from the
traditional category:

1. first-party native and web clients.
2. live human sharing.
3. product-quality selection, scrolling, and scrollback. [announcement]

The team has credible distribution into terminal and agent users. A terminal
also reaches local tools, remote hosts, databases, build systems, and
production consoles without waiting for application-specific integrations.
[inferred]

### 8.2 The expansion

The expansion turns the terminal session into the durable work record. If that
works, a user does not need separate concepts for a local shell, remote agent,
background job, shared debug session, and production action. Each becomes a
block or stream inside one session. [inferred]

The category claim is therefore closer to “operating layer for active work”
than “terminal application.” The terminal is the compatibility interface that
lets the company start before it has integrations for every tool.

### 8.3 Likely business model

The company is venture funded and lists investors with infrastructure,
developer-tool, collaboration, and product backgrounds. The site also commits
to private beta and some OSS releases. [announcement]

The likely model is:

- free or open local use to build adoption.
- paid hosted connectivity and sharing.
- organization plans for identity, policy, audit, retention, and support.
- possible managed compute or production access products later. [forecast:
  medium]

Other models remain possible. The team could charge for native clients, keep a
free self-hosted tier, or sell a complete managed service. No price or product
edition exists in the public corpus. [limitation]

### 8.4 Potential moat

The likely moat has four parts:

- terminal correctness and cross-client consistency.
- a durable session protocol used by people and software.
- high-quality native remote and collaborative interaction.
- production identity, policy, and operational history.

Terminal rendering alone is not a durable company moat because Ghostty is
open and other mature terminal engines exist. A useful session protocol and
trusted production control plane can be a stronger moat. [inferred]

## 9. Hard problems and failure modes

### 9.1 A terminal can become the wrong abstraction

Terminal compatibility is broad, but its semantics are weak. The system sees
bytes, escape sequences, and process state. It does not automatically know
that a deployment succeeded, a test result is authoritative, or a human
approved a production change.

If Superlogical treats parsed output as structured truth, the production layer
will be fragile. It needs native adapters and typed outcome sources beside the
terminal stream.

### 9.2 The durable session can become a privileged ambient authority

A long-lived session can hold shell access, environment secrets, agent state,
and production credentials. Sharing and remote access turn it into a high-value
security boundary.

Risks include:

- stolen attachment tokens.
- stale device access.
- host service compromise.
- malicious terminal escape sequences.
- clipboard and paste attacks.
- secret capture in history.
- cross-member data leakage.
- agent use of human credentials.
- replay of old input against a new process generation.

The production stage must address these as separate controls. A secure network
connection alone is insufficient.

### 9.3 Multi-client terminal geometry is difficult

A terminal application normally receives one row and column geometry. A phone,
browser, and desktop will not share one useful geometry. Reflow can change what
each user sees. Full-screen TUIs can depend on exact cursor positions and mouse
coordinates.

Possible answers include one canonical size, per-client viewports, detached
read-only history, or independent rendering checkpoints. Every answer has
interaction tradeoffs.

### 9.4 Agent output changes the scale

Agents create large output volumes, long sessions, many parallel processes,
and frequent file or tool actions. Scrollback designed for a person can become
an expensive unbounded event store. The system will need quotas, compaction,
search, archive, and exact truncation signals.

### 9.5 “Context by default” can violate least disclosure

A system that spans local work and production can accumulate sensitive source,
commands, output, credentials, customer data, and incident material. Context
must have audience and purpose boundaries. An agent, collaborator, or support
operator should not receive all session context because it is convenient.

### 9.6 The scope can expand too early

Remote development, CI, agents, sandboxes, production applications, debugging,
and incident response are each large markets. A general work platform can lose
focus before the terminal multiplexer becomes excellent.

The announcement directly addresses this risk. It states that the terminal
multiplexer must remain excellent at its narrow job while the larger system
grows. [announcement]

### 9.7 Cross-platform expectations can outrun the launch plan

The first named native platforms are macOS and iOS. Many terminal,
infrastructure, and production users work from Linux and Windows. Web access
can cover some demand, and `libghostty` can support these platforms, but native
client parity remains an open question. [announcement] [public]

### 9.8 Open-source and company boundaries can be unclear

Ghostty is a separate open-source project with its own contributors and goals.
Echo and Bonsplit are separate prior products. Superlogical will need clear
provenance, governance, license, contribution, and brand boundaries if it uses
these components. The current announcement makes no claim about reuse.
[limitation]

## 10. Forecast ledger

| Forecast                                                     | Confidence | Evidence                                                    | Disconfirming signal                                      |
| ------------------------------------------------------------ | ---------- | ----------------------------------------------------------- | --------------------------------------------------------- |
| A separate session service owns PTYs and child processes     | High       | reconnect after app closure and cross-device access         | GUI process owns sessions and closure stops work          |
| `libghostty` or a close derivative is the terminal core      | High       | founder, current library, Echo use, web portability         | published core uses another parser with no Ghostty layer  |
| macOS and iOS clients use native Swift UI                    | High       | explicit native claim, Ghostty architecture, Echo, Bonsplit | web-wrapper implementation or shared custom widget system |
| Browser uses the same terminal semantics                     | High       | web is a first-party client and `libghostty` supports WASM  | browser is a low-fidelity log viewer only                 |
| Hosted service handles identity, discovery, and sharing      | Medium     | live sharing and cross-device access                        | direct peer-only system with no hosted account service    |
| Workloads can stay on user-controlled hosts                  | Medium     | remote-host scope and Pearkes BYOC preference               | managed-only execution and storage                        |
| One active writer controls a shared PTY                      | Medium     | simplest safe collaborative terminal design                 | true concurrent writer model or view-only sharing         |
| Session history becomes searchable and exportable            | Medium     | durable history and production operation                    | scrollback remains only a bounded local buffer            |
| Stage two exposes a typed API and SDK                        | High       | software-driven sessions and structured actions             | automation only through CLI keystrokes and shell commands |
| New block kinds move beyond terminals                        | Medium     | “everything composable” and rotating scope list             | terminal blocks remain the only first-class object        |
| Enterprise policy, approval, and audit arrive in stage three | Medium     | safe and operable production goal and team history          | production means only reliable remote terminal access     |
| Local or protocol components become OSS                      | Medium     | explicit promise of OSS releases                            | only examples or unrelated Ghostty work are released      |
| Hosted collaboration and controls are paid                   | Medium     | venture-backed company and hosted needs                     | paid native client only or fully self-hosted model        |
| Superlogical becomes a CI replacement                        | Low        | automatic work and background jobs in vision                | jobs remain external streams attached to sessions         |
| Superlogical becomes an incident-management system           | Low        | incident response in rotating headline                      | production support remains terminal access and audit only |

## 11. Questions that public artifacts must answer

### 11.1 Session and process

1. Which process owns the PTY and child process?
2. Does the session survive client exit, host restart, and service restart?
3. What is the identity of a restarted process generation?
4. What data is canonical: raw bytes, terminal state, events, or all three?
5. How does the system bound scrollback and replay?
6. Can a session move between hosts, or only reconnect to the same host?

### 11.2 Clients and collaboration

1. Is the browser a full terminal client or a remote view?
2. Do Apple clients use native UI above `libghostty`?
3. How does the system arbitrate input from several clients?
4. Which client controls terminal geometry?
5. Are sharing roles read-only, writer, controller, and administrator?
6. Can an owner revoke one device or member immediately?

### 11.3 Hosting and privacy

1. Can users self-host the session service or control plane?
2. Which bytes pass through Superlogical services?
3. Which bytes are stored by Superlogical?
4. Is content encrypted end to end between enrolled clients and hosts?
5. What metadata remains visible to the hosted service?
6. What are retention, export, and verified-deletion behaviors?

### 11.4 Software and agents

1. Is there a versioned API, SDK, or protocol?
2. Can software subscribe with a stable cursor and resume after a gap?
3. Are agent events structured or only terminal bytes?
4. How are software clients distinguished from people?
5. Does an agent inherit the human writer's authority?
6. Can existing terminal agents attach without moving their credentials or
   session stores?

### 11.5 Production

1. What does “production” mean in the first concrete product?
2. How do identity, workload identity, policy, and approval interact?
3. What is recorded in the audit history?
4. What proves an effect beyond the terminal transcript?
5. What contains a process or agent?
6. How do upgrades preserve session and protocol compatibility?

### 11.6 Company and ecosystem

1. Which components will be open source?
2. What licenses and governance apply?
3. What is the relationship to Ghostty, Echo, and Bonsplit?
4. Which platforms and architectures will ship?
5. What is local, free, hosted, and paid?
6. Will third parties implement clients, hosts, blocks, or adapters?

## 12. OpenAgents comparison and disposition

### 12.1 Strategic overlap

Superlogical and OpenAgents share several product ideas:

- one durable object for related work.
- local and remote placement.
- desktop, web, and mobile control.
- human and agent participation.
- visible history.
- structured actions.
- long-lived work after a client disconnects.
- production safety and operation.

The important difference is the starting abstraction. Superlogical starts
with a terminal session and plans to expand upward. OpenAgents starts with
typed work, agent, authority, evidence, and verification contracts, and uses
Omega as the primary native IDE surface.

### 12.2 Lessons to track

OpenAgents should track these possible lessons:

1. **A session is a product object.** Users should attach from different
   clients without rebuilding context.
2. **Terminal fidelity matters.** Agent supervision still depends on correct
   PTY, scrollback, selection, resize, Unicode, and input behavior.
3. **Mobile is a real client.** A phone should supervise and steer the same
   work, not receive a separate summary-only chat.
4. **People and software use one visible system.** Automatic work should not
   disappear into a disconnected job log.
5. **Placement is part of session identity.** Local, remote, sandbox, managed,
   and production work need explicit host and generation facts.
6. **Composability needs structure.** Raw terminal streams are a compatibility
   plane, not the full product model.

### 12.3 Boundaries OpenAgents must retain

OpenAgents must not infer these equivalences:

- a terminal session is not a canonical work receipt.
- reconnect is not durable command admission.
- scrollback is not a complete event log.
- a shared terminal is not a collaboration authority model.
- a permission is not containment.
- a process exit is not an accepted deliverable.
- hosted reachability is not enrolled workload identity.
- visible history is not safe disclosure.
- software control is not authorization to act.

### 12.4 Current disposition

**Disposition: track, do not adopt.**

There is no public Superlogical component to evaluate, pin, license, test, or
integrate. The announcement does not justify a ProductSpec change, an
AssuranceSpec change, a roadmap change, or an implementation candidate. This
teardown records a high-relevance thesis and a falsifiable forecast only.

The next useful review point is the first of these events:

- a public beta artifact.
- an OSS repository or package.
- a protocol or SDK.
- a security or privacy design.
- a detailed product demonstration.
- a price and hosting model.

At that point, a new source audit should replace forecasts with exact artifact,
runtime, protocol, storage, authority, and failure evidence. Sections 1 through
11 should remain the launch-day forecast. The following section is a same-day
OpenAgents target-positioning addendum. It does not change a Superlogical
forecast or admit product work.

## 13. OpenAgents and Omega: the application for all work

This section is a target strategy, not a claim about current shipped behavior.
It uses the OpenAgents source at
`1281e6c7eea397830d73971f867f61fcf0bfddf7` and the current
[Omega and T3 Code gap analysis](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md).
That analysis pins the Omega source that supports each capability statement.
The [T3 Code server analysis](./2026-07-27-t3-code-server-projection-consistency-architecture.md)
provides the control-plane comparison.

### 13.1 The category claim

Superlogical proposes a multiplexer for all work. That phrase identifies the
missing coordination layer. It does not need to define the final user product.

The stronger OpenAgents and Omega position is:

> **OpenAgents and Omega can be the application for all work.**

Omega provides the native environment where a person can inspect and change
the work. OpenAgents provides the typed control, placement, identity, policy,
history, and evidence system behind that environment. Pylon and managed Agent
Computers provide execution without becoming separate products that each user
must learn.

The application is only one client of the system. The same system must also
have a versioned API and SDK. Automatic work, embedded tools, and third-party
clients must use the same work objects and controls as the application.

This product shape gives three related promises:

1. **One application.** A user sees local, remote, automatic, and production
   work in one workbench.
2. **One control model.** A person, agent, or program uses typed actions with
   explicit authority and durable outcomes.
3. **One ecosystem.** The application, API, SDK, agents, hosts, and extensions
   use the same contracts.

“IDE for all work” is a useful internal category description. The product does
not have to expose IDE complexity at first. Most people should receive a calm
application that already handles sessions, agents, machines, credentials,
history, and recovery. Advanced users and software authors can use the API and
SDK when they need more control.

### 13.2 The multiplexer is a layer, not the complete product

A terminal multiplexer combines byte streams and input streams. A work
application must also understand why an action occurred, who could approve it,
where it ran, what changed, and what proves the result.

The proposed OpenAgents model therefore sits above a terminal multiplexer:

| Superlogical launch wedge | OpenAgents and Omega product object |
| ------------------------- | ----------------------------------- |
| terminal block            | work block                          |
| PTY or process            | execution lane                      |
| attached client           | scoped participant                  |
| terminal input            | typed intent or bounded raw input   |
| scrollback                | event history plus explicit gaps    |
| remote host               | identified placement                |
| shared session            | governed work object                |
| process exit              | one fact in an outcome record       |

The terminal remains important. It is the universal compatibility block for
shells, agents, jobs, and infrastructure. It must not become the authority for
the complete work lifecycle. Editor state, plans, approvals, diffs, tests,
deployments, metrics, and receipts need native types beside terminal output.

This distinction creates a useful competitive frame:

- Superlogical begins with terminal bytes and plans to add structure above
  them.
- T3 Code begins with a provider-neutral agent control plane and projects it
  to desktop and mobile clients.
- OpenAgents and Omega can begin with typed work, deep native IDE capability,
  and explicit authority. They can integrate terminal compatibility below that
  layer and production operation above it.

This is not “Superlogical with an editor.” It is one durable work system where
the terminal is one composable block and the native IDE is the primary human
control surface.

### 13.3 Carry the T3 Code pattern forward

T3 Code is the clearest current proof that users benefit from one agent control
plane. Its environment server owns projects, threads, turns, messages,
activities, plans, approvals, sessions, checkpoints, worktrees, terminals, and
provider processes. Desktop and mobile clients send commands to that server and
read projections from it. This model makes parallel work understandable.

OpenAgents and Omega should carry that pattern forward, not stop at coding
agents:

| T3 Code pattern                               | OpenAgents and Omega extension                                                                                                |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| One coding environment owns work.             | One durable work object can represent a repository task, job, service change, incident, deployment, or other bounded outcome. |
| Provider adapters control coding agents.      | Lane adapters can control agents, people, jobs, services, and production operations.                                          |
| Desktop and mobile project one server model.  | Desktop, mobile, web, API, and SDK clients can project one typed work model.                                                  |
| The environment selects a process host.       | Placement can name a local host, Pylon, remote host, sandbox, managed Agent Computer, CI worker, or production target.        |
| Commands and read models describe agent work. | Intents, events, interactions, evidence, and receipts can describe the complete work lifecycle.                               |
| The workbench contains coding tools.          | Omega can keep native coding depth and add domain blocks without reducing every domain to chat.                               |

OpenAgents must also improve the control boundary. The T3 Code analysis finds
that its database state is strong, but provider work uses a best-effort live
reactor. Git and file effects also occur outside the database transaction.
OpenAgents already treats admission, effect, observation, verification, and
acceptance as different facts. That separation should become a visible product
advantage, not remain only an internal safety system.

The extension is therefore both broader and stricter. It covers more kinds of
work, hosts, and clients. It also preserves the exact authority and evidence
for each effect.

### 13.4 The parts that exist now

The [Omega and T3 Code gap analysis](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md)
reaches one central conclusion: Omega has strong parts, but it does not yet
compose them through one control projection. That gap is material, but it also
means OpenAgents does not need to invent the complete system from zero.

| Product layer        | Current building blocks                                                                                                             | Present composition gap                                                                       |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Native workbench     | Omega editor, language services, project model, Git, diff review, terminal, and agent timeline                                      | The default work surface does not present all of them as parts of one work object.            |
| Agent execution      | Native Omega agent, ACP agents, Codex app-server support, terminal agents, Full Auto, Pylon, and managed Agent Computer paths       | Each lane has different ownership, lifecycle, and projection rules.                           |
| Durable control      | Typed send, queue, steer, interrupt, approval, pause, resume, stop, retry, and handoff mechanisms                                   | No single command model spans every lane and client.                                          |
| Placement            | Local workspaces, managed worktrees, owner-local Pylons, remote-development primitives, and managed Agent Computer contracts        | No complete host catalog presents identity, health, generation, capability, and grant state.  |
| History and recovery | Native thread stores, queue journals, Full Auto records, Sync cursors, generation fences, and portable-session contracts            | A user cannot ask one surface what is active, blocked, lost, or recoverable across all lanes. |
| Safety               | Signed device grants, scoped identity, admission records, approvals, policy checks, brokered credentials, and containment contracts | The effective authority is not yet one clear user-facing summary for each work item.          |
| Evidence             | Command outcomes, run evidence, checkpoints, exact usage records, verification refs, and receipts                                   | Evidence remains split across subsystems instead of one outcome view.                         |
| Contract ecosystem   | Effect Schema protocols, Codex and ACP packages, Runtime Gateway contracts, Sync schemas, and Effect Native views                   | There is no single public Work API or SDK that exposes the complete model.                    |
| Client reach         | Native desktop depth, a bounded mobile mirror, web services, and shared projection code                                             | Mobile and web do not yet control the same complete workbench as desktop.                     |

The honest current statement is therefore:

> OpenAgents and Omega have much of the control and work substrate now. They do
> not yet have the one coherent application, projection, API, and SDK.

That is a composition problem before it is a new-infrastructure problem.

### 13.5 A common work model

OpenAgents needs a small product vocabulary that can include a terminal
session without reducing all work to a terminal session. The following names
are candidate names. They are not admitted schema names.

1. **Work** is the durable top-level object. It binds an objective, owner,
   context, participants, current state, placement, policy, history, and
   outcome refs.
2. **Session** is one live or resumable interaction with the work. A work
   object can contain a human session, agent turn, background run, or incident
   session.
3. **Block** is a composable view or control surface. Examples include a
   transcript, editor, terminal, plan, diff, review, preview, log, metric, and
   artifact.
4. **Host** identifies where work can run. It includes a local machine, remote
   host, sandbox, Pylon, Agent Computer, CI worker, or production target.
5. **Actor** identifies a person, agent, automation, or service. It does not
   imply authority.
6. **Intent** is a typed request to act. It binds the actor, target, generation,
   idempotency identity, requested effect, and required authority.
7. **Event** records an admitted lifecycle fact. A replay cursor and an
   explicit gap rule make history resumable.
8. **Receipt** binds an effect or observation to exact evidence. It does not by
   itself prove verification, acceptance, release, or a public claim.

These objects produce one useful shape:

```text
Omega desktop / OpenAgents mobile / web / API / SDK
                         |
            work inbox and work projection
                         |
        typed intents, events, blocks, receipts
                         |
       lane adapters and placement authority
                         |
 native agent / ACP / Codex / terminal / Full Auto
                         |
 local host / Pylon / sandbox / Agent Computer / production
```

Existing lanes can keep their own authoritative stores during the first
composition stage. A common read model can point to exact source refs instead
of moving all state into a new database. This approach reduces migration risk
and makes disagreement visible.

### 13.6 One application, API, and SDK

The native application should be the best complete client. It should not be a
privileged backdoor around the platform contract.

The application should let a user:

- start work from an objective, repository, incident, service, or existing
  session.
- choose or accept a disclosed placement and execution lane.
- watch people, agents, processes, and services in one timeline.
- open native editor, terminal, plan, diff, preview, and evidence blocks.
- answer questions and approvals without switching tools.
- disconnect and resume from desktop, mobile, or web.
- inspect authority, cost, changes, proof, and terminal outcome.

The API should expose the same model through stable queries, commands, and
subscriptions. A client must be able to list work, read a snapshot, resume from
a cursor, submit an intent, answer an interaction, and fetch a bounded outcome.
The API must preserve capability differences. It must not invent one weak
lowest-common-denominator agent interface.

The SDK should make three extension classes possible:

1. **Lane adapters** connect a new agent, job system, service, or automation
   engine.
2. **Host adapters** connect a new local, remote, sandbox, CI, or production
   placement.
3. **Block adapters** add a typed view and action set for a domain, such as
   deployment, data work, design review, or incident response.

An adapter must declare capabilities, events, actions, authority needs,
failure states, and evidence outputs. The application can then render only the
controls that the current adapter and grant support.

This ecosystem is the practical reason to own the application, API, and SDK
together. Most users get a complete product. Advanced users can automate it.
Partners can extend it without creating another disconnected control plane.

### 13.7 What OpenAgents can do now

The following sequence is a candidate execution path. It is not a roadmap
admission. Each step can reuse current OpenAgents or Omega behavior.

1. **Create a read-only work index.** Project native threads, ACP sessions,
   terminal sessions, Full Auto runs, Pylon assignments, and managed workrooms
   into one list. Preserve each source ref, state, generation, placement, and
   receipt ref.
2. **Make the index the Omega inbox.** Show what is active, waiting for a
   person, failed, completed, or stale. Opening an item should reveal its
   existing native editor, terminal, diff, timeline, or run view.
3. **Add one capability-aware control bar.** Map resume, steer, answer,
   approve, interrupt, pause, retry, handoff, and stop to lane-specific typed
   intents. Hide an action when the lane or current grant does not support it.
4. **Promote native tools to blocks.** Treat editor, terminal, plan, diff,
   preview, logs, metrics, artifacts, and receipts as views of the same work.
   Do not rebuild mature Omega tools in a generic web canvas.
5. **Add the host and placement catalog.** Show the local host, Pylon, remote
   host, sandbox, Agent Computer, and production target through stable
   identity, generation, health, capability, and grant facts.
6. **Use the internal contract as the first SDK.** Stabilize the work query,
   intent, event, interaction, block, and receipt schemas inside the product.
   Then publish a TypeScript client and generate other clients only after the
   behavior is stable.
7. **Project the same model to mobile and web.** Start with the attention inbox,
   work state, approvals, safe steering, diff review, and bounded task output.
   Add a full remote terminal only when its input and authority model is safe.

The first complete demonstration can be narrow:

1. A user starts a repository task in Omega.
2. OpenAgents creates one work object with a worktree, lane, host, and grant.
3. An agent works while terminal output, file changes, plans, and questions
   appear as blocks.
4. The user closes Omega. The work continues on the selected host.
5. The phone shows an approval request from the same work object.
6. The user approves or rejects the exact action.
7. The user resumes in Omega and reviews the native diff.
8. The final view links the run, changes, tests, verification, and receipt
   state without calling them equivalent.

This demonstration already covers the essential “all work” loop. The domain
is coding, but the object model can later support CI, operations, deployments,
incidents, data jobs, research, and other structured work.

### 13.8 Why this can be a stronger product

The strongest advantage is not the number of streams in one window. It is the
amount of difficult control work that disappears for the user.

OpenAgents and Omega can provide these combined advantages:

- native editor, terminal, language, Git, and review depth.
- provider and agent plurality behind capability-aware adapters.
- durable work across interactive and automatic modes.
- local, remote, sandbox, managed, and production placement.
- signed identity, explicit authority, approvals, and revocation.
- structured history, evidence, verification, and receipts.
- one desktop, mobile, web, API, and SDK contract family.

Other tools can implement individual parts. The defensible system is the
combination, plus the operational knowledge required to make the combination
reliable. The application makes that complexity usable. The API and SDK make
it extensible. The receipts and authority model make it suitable for work that
has consequences.

The terminal multiplexer can become an important interoperability peer or
component. It does not have to become the OpenAgents product center.

### 13.9 Product language and honest claim boundary

The positioning can use a ladder:

- **Near-term product:** “The application for work with agents.”
- **Category direction:** “The IDE for all work.”
- **Explanatory line:** “One application, API, and SDK for work across people,
  agents, and machines.”
- **Trust line:** “See what ran, control what can act, and verify what changed.”

“The application for all work” must remain a direction until the common work
projection and cross-client control have product proof. Current public claims
must name the exact supported lanes, clients, hosts, actions, and receipts.

The strategic conclusion is still immediate:

> OpenAgents and Omega do not need to wait for Superlogical to build this
> category. The required terminal, IDE, agent, control, placement, authority,
> and evidence parts already exist in useful forms. The next product act is to
> compose them into one work object and one excellent application, then expose
> that same system through the API and SDK.
