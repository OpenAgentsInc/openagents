# All Work Model

Status: OpenAgents product strategy reference with a landed read-only boundary

This document defines a common model for interactive, automatic, remote, and
production work. It is not an admitted ProductSpec, schema, roadmap item, or
public capability claim.

The model gives OpenAgents and Omega one vocabulary for the application, API,
SDK, agents, hosts, and extensions. A terminal session can fit inside this
model, but it does not define the complete work lifecycle.

## Summary

**Work is the container. Sessions perform it. Blocks display and control it.
Hosts run it. Intents request actions. Events record facts. Receipts bind facts
to evidence.**

## Core objects

| Object      | Purpose                                                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------------------- |
| **Work**    | The durable top-level object for an objective and its complete lifecycle.                                  |
| **Session** | One live or resumable interaction within the work.                                                         |
| **Block**   | A composable view or control surface, such as chat, editor, terminal, diff, plan, preview, log, or metric. |
| **Host**    | The identified placement where work can run.                                                               |
| **Intent**  | A typed request to perform an action with explicit authority.                                              |
| **Event**   | A durable lifecycle fact with stable order and resume rules.                                               |
| **Receipt** | Evidence that an effect or observation occurred.                                                           |

### Work

Work binds the objective, owner, context, participants, state, placement,
policy, history, and outcome refs. One work object can represent a repository
task, CI job, deployment, incident, data job, research task, or another bounded
outcome.

Work can continue when a client disconnects. It can contain many sessions and
blocks. It does not require one process, one agent, or one host for its complete
lifetime.

### Session

A session is one live or resumable interaction with the work. Examples include
a human review, an agent turn, a terminal process, a background run, and an
incident response period.

A session is not the complete work object. A restarted process can create a
new session generation without creating new work.

### Block

A block is a typed view or control surface inside the work. Blocks can include
a transcript, editor, terminal, plan, diff, review, preview, log, metric, and
artifact.

A block can show state or submit an intent. It does not own authority because
it is visible or interactive.

### Host

A host identifies where a session or effect can run. Examples include a local
machine, remote host, sandbox, Pylon, Agent Computer, CI worker, and production
target.

Host identity includes the placement generation and relevant capability facts.
Reachability does not grant execution authority.

### Intent

An intent is a typed request to act. It binds the actor, target, generation,
idempotency identity, requested effect, and required authority.

An admitted intent is not proof that the effect occurred. A rejected, expired,
or interrupted intent remains a useful lifecycle fact.

### Event

An event records an admitted fact about the work lifecycle. Stable sequence and
cursor rules let clients resume after a gap or disconnection.

An event is not necessarily an effect receipt. A transcript or terminal stream
is also not automatically a complete event history.

### Receipt

A receipt binds an effect or observation to exact evidence. It can identify the
intent, host, generation, result, artifact, or observation that produced it.

A receipt does not by itself prove verification, acceptance, release, or a
public claim. Those states need their own authority and evidence.

## Relationships

```text
Work
├── Session  ── runs on ──> Host
├── Block    ── displays or controls part of the work
└── history
      Actor ── submits ──> Intent
                              |
                         admission
                              |
                           effect
                          /      \
                      Event    Receipt

Verification, acceptance, release, and public claims remain separate.
```

The same relationships support a native application, mobile application, web
client, API client, SDK integration, agent, or automation. No client receives
more authority only because it uses a different interface.

## Example

For a repository task:

1. Omega creates or opens the Work object.
2. OpenAgents binds a worktree, execution lane, Host, and grant.
3. The agent and user interact through separate Sessions.
4. Editor, terminal, plan, diff, and test results appear as Blocks.
5. User and agent actions enter as typed Intents.
6. Durable Events record lifecycle changes.
7. Receipts bind changes, tests, and observations to evidence.

The user can close one client and resume from another client. The Work identity
and its history do not depend on one window or one terminal process.

## Required distinctions

The model preserves these boundaries:

- a Session is not the complete Work object.
- a Block is not an authority boundary.
- Host reachability is not an execution grant.
- an Intent is not an effect.
- an Event is not necessarily complete evidence.
- a Receipt is not verification or acceptance.
- verification is not release or a public claim.

## Adoption boundary

These names are product-strategy vocabulary. The first structural boundary is
now current in `@openagentsinc/all-work-contract`: Work summary and snapshot,
same-identity Issue projection, assignment and delegation refs, source
authority, freshness, completeness, revisions, cursors, and the
`omega-effectd.v2` Work read methods. That package owns encoded shape only. It
does not make this strategy document a lifecycle or storage authority.

The first composition stage uses a read-only Work projection with exact source
refs. Existing lanes retain their authoritative stores. A later schema proposal
must reconcile those sources before it changes ownership.

## Origin

This model is the domain-model companion to the
[App for All Work thesis](./README.md), which lists its sources. The model's
distinctions answer the failure modes recorded across the
[teardown catalog](../teardowns/README.md): products that treat a terminal
transcript as a receipt, a reconnect as admission, a permission prompt as
containment, or a process exit as an accepted deliverable.
