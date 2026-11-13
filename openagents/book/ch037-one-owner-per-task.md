# 37. One owner per task

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A conversation now follows you between devices, and a turn can outlive the
screen that started it. This chapter gives every agent task exactly one
place that runs it, its execution owner, and makes every other client a
viewer that can't run it again.

## Draft

An agent task runs in one of three places: on the device, on a computer you
paired, or on our hosted service (chapter 29). The task record names which
one. That place is the execution owner, and only it calls models, runs tools
and asks for payments for that task
([Coax Native](../../coax/native.md#layers-and-execution-ownership)).

Every other connected client presents the task. It reads the task record,
draws the reply as it grows, and sends input, approvals and Stop to the
owner. It does not call the model itself or repeat a tool call. Two phones
open on one conversation show the same turn; they don't run it twice.

A change of owner is an explicit handoff. You might start a task on your
phone and move it to your computer. The old owner stops at a recorded point,
the record names the new owner, and the new one continues from there. A
handoff that is not acknowledged leaves the old owner in place.

Reconnects and sync can't create a second run. A client that reconnects
reads the record; it never decides a task is lost and starts it again. If an
owner goes away mid-task, the record says so, and the task waits for you to
resume it somewhere, with the owner named.

This rule keeps three things apart that are easy to blur. The app's code is
shared on every device (chapter 1). Your conversations are shared data
(chapter 35). Running a task happens in one place. The session record and
journal in [`coax/lib/agent/`](../../coax/lib/agent) already write what a
run did; this chapter adds who runs it.

## What we build

A plan for what this chapter ships once the draft is approved:

- An owner field on the task record, and a handoff with an acknowledgement.
- Client behavior: present, send input and Stop to the owner, never execute.
- A refusal at the service for a second execution of one task.

It builds on what exists today:

- [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax)
- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`coax/lib/agent/engine/engine.coax`](../../coax/lib/agent/engine/engine.coax)
- [`docs/sync/README.md`](../../docs/sync/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A task's owner and its handoff | Planned, in `openagents.sync.v1alpha1` beside the conversation log ([chapter 35](ch035-sync-across-devices.md)) | Binary frames on one WebSocket | Planned |

### The chapter's check

- **Source:** the owner rule in the shared app and its policy file on the
  service.
- **Trace:** two clients open one conversation; client A sends; both traces
  show one model request, made by the owner, and the same parts.
- **Refusal:** client B asks to execute the running task and gets
  `not the owner`, with the owner named; nothing runs.
- **Independent check:** the service's request log for the task has exactly
  one execution after both clients reconnect three times.

Planned paths: `coax/web/api/task-owner-policy.coax`.

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across terminal, Android and iOS
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): one suite across computers, cloud and mobile
- **Also:**
  - [Coax Native, "Layers and execution ownership"](../../coax/native.md#layers-and-execution-ownership)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P3, P13
