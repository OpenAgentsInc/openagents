# OpenAgents and Omega: The App for All Work

Status: OpenAgents product strategy reference

This document is the canonical statement of the App for All Work thesis. It is
a target strategy, not an admitted ProductSpec, roadmap, schema, or claim about
current shipped behavior. The [teardown catalog](../teardowns/README.md) defers
to this document for the OpenAgents and Omega counter-position.

The thesis is distilled from the complete teardown program: roughly fifty
point-in-time studies of terminals, IDEs, assistants, agent engines, control
planes, review surfaces, and compute fabrics, synthesized in the
[full catalog synthesis](../teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md).
The capability baseline for Omega is the
[Omega and T3 Code gap analysis](../teardowns/2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md),
which pins the Omega source behind each capability statement. The
[T3 Code server analysis](../teardowns/2026-07-27-t3-code-server-projection-consistency-architecture.md)
provides the control-plane comparison. Section 12 lists all sources.

## 1. The category claim

Software work is fragmented. It is split across local machines, remote hosts,
sandboxes, services, and production systems, and across interactive human
work, background jobs, and parallel agent work. Current tools separate the
interactive interfaces, the automatic jobs and their logs, and the production
controls. Agents raise the cost of this split — one person now supervises many
concurrent workers across many hosts — but the split existed first.

A new class of entrants answers with a durable-session or multiplexer layer:
one long-lived session object that spans applications and environments,
survives disconnection, and admits both people and software. That framing
correctly identifies the missing coordination layer. It does not need to
define the final user product, because a coordination layer under fragmented
tools still leaves the person assembling the work from parts.

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

## 2. What “all work” means

The claim needs a scope, or it is a slogan. The work the system must hold, in
adoption order:

1. **Interactive coding.** A person and an agent working a repository task,
   with native editing, terminal, search, review, and Git depth.
2. **Delegated agent work.** Turns and whole tasks handed to agents that run
   while the person does something else, on this machine or another one.
3. **Parallel and background work.** Many isolated runs at once — worktree
   fan-out, best-of-N attempts, scheduled and unattended runs — with explicit
   admission, leases, and recovery rather than fire-and-forget.
4. **Remote supervision.** The same work followed, answered, approved,
   steered, and stopped from a phone or the web, with no separate
   summary-only companion app.
5. **Operational work.** Deployments, service changes, incidents, and
   production actions, where approval, audit, and blast radius are the
   product, not an afterthought.
6. **Other structured domains.** CI, data jobs, research, design review,
   and eventually non-software work that still needs sessions, evidence,
   and authority.

Coding is the first domain because it has the best-developed agents and the
most acute fragmentation today. The object model must not assume it is the
last domain.

## 3. The competitive field

Every serious product class is converging on the same move: an agent-session
plane pushed above the product's original center of gravity.

| Class | Representatives | Center of gravity | What it proves | Structural limit |
| --- | --- | --- | --- | --- |
| Terminal-first | durable-session multiplexers | the PTY and the long-lived session | work should survive the window and admit software | terminal semantics are weak; bytes prove neither effects nor approvals |
| IDE-first | VS Code, Cursor, Zed | the editor and the project graph | the incumbent editors are themselves pivoting to agent sessions | the session plane inherits the editor's identity model and authority gravity |
| Assistant-first | Claude Desktop, ChatGPT/Codex desktop | the conversation | a chat surface is an acceptable front door to real work | no project graph and no work model; the engine is a closed sidecar |
| Engine-first | Factory, T3 Code | one engine or server behind many clients | one control plane can serve desktop, mobile, chat, and CI | weak authority laws under the architecture; default-open execution |
| Projection-first | Pierre | review-grade rendering | review is a first-class surface worth dedicated engineering | owns no truth: no filesystem, sessions, authority, or receipts |
| Local-compute-first | Local Studio, cluster fabrics | owned inference and hardware | local inference is a placement with lifecycle, not a provider string | trust laws below the consequence level; port possession becomes authority |

Two conclusions follow from the catalog.

First, the **supervision architecture is converging and commoditizing**: a
real engine outside the renderer, a versioned command/query/event seam,
worktree-isolated parallel agents, desktop-mobile continuity, a workbench that
grows out of chat, and collaboration separated from execution. Convergence
this broad is validation, not differentiation.

Second, **none of these products relocated authority to match the new
plane**. The agent sessions inherit the old identity model — a repository, a
conversation, a port, a process tree — so nobody owns one durable, portable,
receipted unit of work across surfaces, hosts, and providers. The unclaimed
territory is the trust half: authority manifests, execution receipts, delivery
receipts that distinguish `completed` from `merged` and `accepted`,
host-portable sessions, release provenance, exact usage and model-identity
truth, and economic participation. The synthesis states the verdict directly:
the supervision half is becoming table stakes, and OpenAgents wins on the
trust half or not at all.

The most instructive single case is the leading agent-IDE fork: it
independently validated most of the original OpenAgents demand list — desktop
over TUI, mobile carrying the same work, overnight runs, subagents,
discoverable history — while accumulating trust failures in exactly the
unclaimed territory: unpredictable startup authority, undisclosed model
identity, illegible billing, and a closed engine, cloud, and marketplace. In
2025 the differentiation was a feature list. In 2026 it is a trust and
openness list, because the features are now table stakes.

## 4. The multiplexer is a layer, not the complete product

A terminal multiplexer combines byte streams and input streams. A work
application must also understand why an action occurred, who could approve it,
where it ran, what changed, and what proves the result.

Three levels of multiplexing make the distinction precise:

| Level | Independent streams | Common interface |
| --- | --- | --- |
| Terminal multiplexing | PTYs and terminal blocks | panes, tabs, input, terminal state |
| Work multiplexing | human turns, agent turns, jobs, logs, artifacts, actions | durable session and structured history |
| Production multiplexing | deployments, live services, incidents, approvals, operator actions | policy-controlled operational session |

The first level can ship alone. The second and third are where the product
value and the product risk live, and a terminal-first entrant must climb into
them after the fact. OpenAgents starts from typed work and integrates the
terminal below it:

| Terminal-session wedge | OpenAgents and Omega product object |
| ---------------------- | ----------------------------------- |
| terminal block         | work block                          |
| PTY or process         | execution lane                      |
| attached client        | scoped participant                  |
| terminal input         | typed intent or bounded raw input   |
| scrollback             | event history plus explicit gaps    |
| remote host            | identified placement                |
| shared session         | governed work object                |
| process exit           | one fact in an outcome record       |

The terminal remains important. It is the universal compatibility block for
shells, agents, jobs, and infrastructure, and an agent running in a terminal
block is immediately compatible with every terminal agent that exists. It must
not become the authority for the work lifecycle, because its semantics cannot
carry one. The boundaries below are the reason, and each one is a place a
session-first product can silently overclaim:

- a terminal session is not a canonical work receipt.
- a reconnect is not durable command admission.
- scrollback is not a complete event log.
- a shared terminal is not a collaboration authority model.
- a permission prompt is not containment.
- a process exit is not an accepted deliverable.
- hosted reachability is not enrolled workload identity.
- visible history is not safe disclosure.
- software control is not authorization to act.

These boundaries are the same distinctions the
[All Work model](./model.md) encodes as separate objects, which is not a
coincidence: the model exists so the boundaries have names.

## 5. Carry the control-plane pattern forward

T3 Code is the clearest current proof that users benefit from one agent
control plane. Its environment server owns projects, threads, turns, messages,
activities, plans, approvals, sessions, checkpoints, worktrees, terminals, and
provider processes. Desktop and mobile clients send commands to that server
and read projections from it. This model makes parallel work understandable,
and its strongest idea is not event sourcing — it is **one environment-owned
model for every client**.

OpenAgents and Omega should carry that pattern forward, not stop at coding
agents:

| Control-plane pattern                          | OpenAgents and Omega extension                                                                                                 |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| One coding environment owns work.              | One durable work object can represent a repository task, job, service change, incident, deployment, or other bounded outcome.  |
| Provider adapters control coding agents.       | Lane adapters can control agents, people, jobs, services, and production operations.                                           |
| Desktop and mobile project one server model.   | Desktop, mobile, web, API, and SDK clients can project one typed work model.                                                   |
| The environment selects a process host.        | Placement can name a local host, Pylon, remote host, sandbox, managed Agent Computer, CI worker, or production target.         |
| Commands and read models describe agent work.  | Intents, events, interactions, evidence, and receipts can describe the complete work lifecycle.                                |
| The workbench contains coding tools.           | Omega can keep native coding depth and add domain blocks without reducing every domain to chat.                                |

The pattern also carries laws worth adopting exactly: the environment owns
durable state and clients send typed commands; one transaction commits events
and their core projections together; a snapshot and its sequence number are
read in one transaction; live delivery attaches before snapshot work so the
handoff can lose nothing; replay may overlap live delivery and the client
drops duplicates by sequence; reconnect is repair, not replay, and an
unreconstructable gap is marked honestly rather than papered over.

OpenAgents must also improve the control boundary, because the reference
implementation is weakest exactly where OpenAgents is strongest. Its provider
work rides a best-effort live reactor with no durable cursor, so a crash
between intent commit and provider start strands the committed intent. Its Git
and file effects occur outside the database transaction. Its receipts prove
database admission and nothing further. OpenAgents already treats admission,
effect, observation, verification, and acceptance as different facts, and the
control plane needs that vocabulary end to end: `command admitted`,
`command committed`, `effect claimed`, `effect completed`, `turn quiesced`,
`work verified`. One sequence number must not claim all of those meanings.

The extension is therefore both broader and stricter. It covers more kinds of
work, hosts, and clients. It also preserves the exact authority and evidence
for each effect.

## 6. Omega is the application

The owner selected Omega — the tracked Zed fork — as the primary OpenAgents
desktop and IDE destination. The thesis is not waiting for a new client to be
invented. The application exists, and its recent history is the thesis in
miniature.

### 6.1 Zero Base is the inversion, already shipped

Omega's default surface is Zero Base: one agent thread, a composer, a
persistent sidebar, and a workbench — with the legacy editor demoted to a
flag and scheduled for removal. The
[zero-base design](../omega/2026-07-26-omega-zero-base-mode.md), the
[zero-base audit](../omega/2026-07-29-omega-zero-base-mode-audit.md), and the
[single-experience plan](../omega/2026-07-29-omega-zero-base-single-experience-plan.md)
record the arc. This is the same inversion the incumbents are attempting from
the other side. They push an agent plane above an editor whose gravity they
cannot escape. Omega made the agent surface the application and kept the
editor's depth one reveal away — the composer is a real editor buffer, a
transcript file link opens a real editable pane, and the workbench surfaces
are the real project, search, review, Git, and terminal entities rather than
web approximations.

### 6.2 The block model is already rendered

The Zero Base workbench shell mounts six work surfaces on an activity rail —
Files, Search, Review, Git, Terminal, Plan — each a typed host around a native
entity, each with capability state, each scoped to the thread's repository and
worktree. That is the Block concept of the [All Work model](./model.md) in
embryo, with the two properties that matter: blocks are views of one work
object, and a block does not own authority because it is visible. The
single-experience plan's repair law — a drawn control's action must be
admitted, its dependencies must exist, and its result must be visible — is
the block contract stated as a UI invariant.

### 6.3 The trust primitives already render

Omega already draws what competitors keep internal, and mechanically guards
what it draws:

- **Executor disclosure.** A thread names the executor that did its work, on
  the surface, from a typed record. Model-identity truth is a rendered
  product feature, not a policy document.
- **The delta registry.** More than one hundred and fifty recorded product
  deltas, each with a mechanical check, so a rebase cannot silently revert a
  policy. The registry is release provenance applied to product behavior.
- **Identity first.** A fresh profile creates a signed identity before the
  front door opens. Mobile access rides signed device grants with short-lived
  one-use pairing secrets and bounded projections.
- **Capability-truthful controls.** The mode's law — if the gate refuses an
  action, its control must not be drawn — is the adapter rule from section 9
  enforced in pixels: the application renders only the controls the current
  lane and grant support.
- **Run evidence.** Full Auto runs carry admission, leases, health,
  generations, handoff, and receipt references through a supervising daemon,
  with a bounded device mirror showing up to sixty-four runs.

### 6.4 The lanes exist; the composition does not

Omega today runs seven execution planes: the native agent loop, external ACP
executors that keep their own homes and credentials, terminal threads, Full
Auto unattended runs, workroom paths, the device bridge, and signed remote
commands. The gap analysis is blunt about the consequence: the main gap is
composition. No single projection describes every thread, tool call, decision,
artifact, and run, and a user cannot ask one surface what is active, blocked,
lost, or recoverable across all lanes.

That verdict sets the Omega work queue for this thesis:

1. **The portable control projection.** One aggregate work projection over
   the existing lanes, consumed by desktop, mobile, and web alike. It does
   not replace the lanes' stores; it makes their state, authority, requests,
   and receipts legible on every authorized client.
2. **The effective-authority record.** One user-facing answer per work item:
   host and workspace, lane, model class, tool profile, file and network
   scope, approval policy, device grants, generation, and revocation refs —
   projected from enforced state, never descriptive text.
3. **The single experience.** Finishing the legacy-editor removal, so the
   application has one init path and one render path, and the block contract
   is structural rather than case-by-case.
4. **Mobile as controller.** The bounded mirror grows into the attention
   loop — find work, read enough to decide, answer, approve, steer, stop —
   before it grows into a phone IDE.

Two distinctions from the lane teardowns govern the composition. Provider
identity and executor identity are different columns and must not collapse: a
compute mesh or hosted model says where inference ran, never which agent loop
and authority executed the work. And external agent kits federate at the
protocol edge as named executors that keep their own homes; they never become
a second authority plane inside the product.

## 7. The parts that exist now

The gap analysis reaches one central conclusion: Omega has strong parts, but
it does not yet compose them through one control projection. That gap is
material, but it also means OpenAgents does not need to invent the complete
system from zero.

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

## 8. A common work model

The reusable [All Work model](./model.md) defines the common product
vocabulary for Work, Session, Block, Host, Intent, Event, and Receipt.

In summary: Work is the container. Sessions perform it. Blocks display and
control it. Hosts run it. Intents request actions. Events record facts.
Receipts bind facts to evidence.

Existing lanes can keep their own authoritative stores during the first
composition stage. A common read model can point to exact source refs instead
of moving all state into a new database. This approach reduces migration risk
and makes disagreement visible.

Placement deserves one elaboration, because it is where “all work” meets the
real world. A host catalog spans at least: the local machine, an enrolled
remote host, a short-lived sandbox, an owner-local Pylon, a managed Agent
Computer, a CI worker, and an enrolled production target. Each placement has a
different owner, benefit, and risk profile, and each needs stable identity,
generation, health, capability, and grant facts. Host reachability is never an
execution grant, and a managed placement is an explicit authority change the
product must show before work is created there.

## 9. One application, API, and SDK

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
subscriptions. A client must be able to list work, read a snapshot, resume
from a cursor, submit an intent, answer an interaction, and fetch a bounded
outcome. A software client needs the full verb set: create a session, start a
block, send input, observe output with a cursor, attach artifacts, request or
record an approval, interrupt a process, subscribe to lifecycle changes, and
export history. The API must preserve capability differences. It must not
invent one weak lowest-common-denominator agent interface.

The SDK should make three extension classes possible:

1. **Lane adapters** connect a new agent, job system, service, or automation
   engine.
2. **Host adapters** connect a new local, remote, sandbox, CI, or production
   placement.
3. **Block adapters** add a typed view and action set for a domain, such as
   deployment, data work, design review, or incident response.

An adapter must declare capabilities, events, actions, authority needs,
failure states, and evidence outputs. The application can then render only the
controls that the current adapter and grant support — the same
capability-truthful rule Omega already enforces on its own surface.

This ecosystem is the practical reason to own the application, API, and SDK
together. Most users get a complete product. Advanced users can automate it.
Partners can extend it without creating another disconnected control plane.

## 10. What OpenAgents can do now

The following sequence is a candidate execution path. It is not a roadmap
admission. Each step can reuse current OpenAgents or Omega behavior.

1. **Create a read-only work index.** Project native threads, ACP sessions,
   terminal sessions, Full Auto runs, Pylon assignments, and managed workrooms
   into one list. Preserve each source ref, state, generation, placement, and
   receipt ref.
2. **Make the index the Omega inbox.** The Zero Base threads sidebar is the
   seed. Show what is active, waiting for a person, failed, completed, or
   stale. Opening an item should reveal its existing native editor, terminal,
   diff, timeline, or run view.
3. **Add one capability-aware control bar.** Map resume, steer, answer,
   approve, interrupt, pause, retry, handoff, and stop to lane-specific typed
   intents. Hide an action when the lane or current grant does not support it.
4. **Promote native tools to blocks.** Treat editor, terminal, plan, diff,
   preview, logs, metrics, artifacts, and receipts as views of the same work.
   The workbench shell already hosts six of them. Do not rebuild mature Omega
   tools in a generic web canvas.
5. **Add the host and placement catalog.** Show the local host, Pylon, remote
   host, sandbox, Agent Computer, and production target through stable
   identity, generation, health, capability, and grant facts.
6. **Use the internal contract as the first SDK.** Stabilize the work query,
   intent, event, interaction, block, and receipt schemas inside the product.
   Then publish a TypeScript client and generate other clients only after the
   behavior is stable.
7. **Project the same model to mobile and web.** Start with the attention
   inbox, work state, approvals, safe steering, diff review, and bounded task
   output. Add a full remote terminal only when its input and authority model
   is safe.

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

## 11. Why this can be a stronger product

The strongest advantage is not the number of streams in one window. It is the
amount of difficult control work that disappears for the user, and the amount
of trust the system can prove rather than assert.

OpenAgents and Omega can provide these combined advantages:

- native editor, terminal, language, Git, and review depth.
- provider and agent plurality behind capability-aware adapters.
- durable work across interactive and automatic modes.
- local, remote, sandbox, managed, and production placement.
- signed identity, explicit authority, approvals, and revocation.
- structured history, evidence, verification, and receipts.
- exact usage, cost, and model-identity truth as a product surface.
- one desktop, mobile, web, API, and SDK contract family.

Other tools can implement individual parts. The market now supplies strong
references for every individual layer and none for the composition plus the
trust layer. The defensible system is the combination, plus the operational
knowledge required to make the combination reliable. The application makes
that complexity usable. The API and SDK make it extensible. The receipts and
authority model make it suitable for work that has consequences — with the
standing rule that receipts are countersigned or they are not verification,
and an agent never accepts its own work.

The terminal multiplexer can become an important interoperability peer or
component. It does not have to become the OpenAgents product center.

## 12. Product language and honest claim boundary

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

> OpenAgents and Omega do not need to wait for any entrant to build this
> category. The required terminal, IDE, agent, control, placement, authority,
> and evidence parts already exist in useful forms, and the application that
> composes them is already the shipped Omega direction. The next product act
> is to compose those parts into one work object and one excellent
> application, then expose that same system through the API and SDK.

## Sources

- [Full catalog synthesis](../teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md)
  — the convergence facts, the trust-half thesis, and the incorporation and
  refusal lists.
- [Omega and T3 Code gap analysis](../teardowns/2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md)
  — the Omega capability baseline, the composition-gap verdict, and the
  parity definition.
- [T3 Code server analysis](../teardowns/2026-07-27-t3-code-server-projection-consistency-architecture.md)
  — the environment-server pattern, its transaction and handoff laws, and its
  durable-effect hole.
- [Superlogical teardown](../teardowns/2026-07-29-superlogical-teardown.md)
  — the terminal-first durable-session entrant this thesis was first drafted
  against; retained as external evidence and forecasts, tracked as a possible
  future interoperability peer.
- The teardown catalog ([index](../teardowns/README.md)) — the per-product
  evidence behind section 3, including the IDE, assistant, engine, review,
  and local-compute references.
- Omega zero-base chain:
  [design](../omega/2026-07-26-omega-zero-base-mode.md),
  [audit](../omega/2026-07-29-omega-zero-base-mode-audit.md),
  [single-experience plan](../omega/2026-07-29-omega-zero-base-single-experience-plan.md).
- [All Work model](./model.md) — the domain vocabulary.
