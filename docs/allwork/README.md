# OpenAgents and Omega: The App for All Work

Status: OpenAgents product strategy reference

This document is the canonical statement of the App for All Work thesis. It is
a target strategy, not an admitted ProductSpec, roadmap, schema, or claim about
current shipped behavior.

The thesis was first developed as the OpenAgents positioning addendum to the
[Superlogical teardown](../teardowns/2026-07-29-superlogical-teardown.md).
That teardown retains the external product analysis, launch-day evidence, and
founder-background forecasts. This document owns the OpenAgents and Omega
counter-position and execution direction.

The capability baseline is the OpenAgents source at
`1281e6c7eea397830d73971f867f61fcf0bfddf7` and the
[Omega and T3 Code gap analysis](../teardowns/2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md).
That analysis pins the Omega source behind each capability statement. The
[T3 Code server analysis](../teardowns/2026-07-27-t3-code-server-projection-consistency-architecture.md)
provides the control-plane comparison.

## The category claim

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

## The multiplexer is a layer, not the complete product

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

## Carry the T3 Code pattern forward

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

## The parts that exist now

The [Omega and T3 Code gap analysis](../teardowns/2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md)
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

## A common work model

The reusable [All Work model](./model.md) defines the common product
vocabulary for Work, Session, Block, Host, Intent, Event, and Receipt.

In summary: Work is the container. Sessions perform it. Blocks display and
control it. Hosts run it. Intents request actions. Events record facts.
Receipts bind facts to evidence.

Existing lanes can keep their own authoritative stores during the first
composition stage. A common read model can point to exact source refs instead
of moving all state into a new database. This approach reduces migration risk
and makes disagreement visible.

## One application, API, and SDK

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

## What OpenAgents can do now

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

## Why this can be a stronger product

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

## Product language and honest claim boundary

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
