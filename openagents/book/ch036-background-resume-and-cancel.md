# 36. Background, resume and cancel

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A phone app is put away mid-sentence. This chapter says what happens to
each piece of work when you leave the app, come back, close a tab or cancel,
so that nothing runs twice and nothing is lost.

## Draft

Every operation declares its lifetime when it starts
([Coax Native, "Time and lifecycle"](../../coax/native.md#time-and-lifecycle)).
Some end with the screen, such as a search as you type. Some end with the
session, such as a pending reply. Some outlast the app, such as a long task
that runs on a computer or our service. The host reports that the app went
to the background or came back; shared rules decide what each operation does
about it.

Going to the background releases what the device must not hold: the
microphone, the camera and input in progress. The app saves its unsent work
(chapter 34). An operation that ends with the screen is cancelled.

Coming back starts fresh. The app does not replay the time it was away by
running a backlog of frames or timers. It asks the service what changed, the
same way a reconnect does, and draws the result.

Work that runs elsewhere keeps running. A turn whose execution owner is our
service or your computer does not stop because your phone slept. When you
come back, the app shows its record up to where it got. Only an explicit
Stop cancels it, and Stop goes to the owner, which ends the turn and records
that it was cancelled.

Cancellation has one rule everywhere: a cancelled operation can't change the
screen, and its late answer is dropped (chapter 5). A payment is the strict
case. Once a payment is sent, cancelling the screen can't take it back; the
app shows its real outcome, including "unknown", and never sends again on
its own.

## What we build

A plan for what this chapter ships once the draft is approved:

- Declared lifetimes for the app's operations, and the background and resume
  actions that act on them.
- A resume that catches up from the service instead of replaying time.
- Stop for a remote turn, sent to its execution owner.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`metaverse-engine/agent/pin.coax`](../../metaverse-engine/agent/pin.coax)
- [`pay/wallet.coax`](../../pay/wallet.coax)

### The chapter's check

- **Source:** the lifecycle actions in the shared app.
- **Trace:** start a fixture turn, send `app.background`, advance the clock
  ten minutes, send `app.resume`, and print the transcript; the parts that
  arrived while away appear once, in order.
- **Refusal:** a payment whose outcome is unknown at `app.background` is
  still unknown at `app.resume`, and no second send is requested.
- **Independent check:** on a device, a recorded run backgrounds the app
  during a turn and resumes it (recorded in `NEEDS_OWNER.md` when it needs
  the owner's phone).

Planned paths: none new. The actions go in [`apps/shared`](../../apps/shared).

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): work that continues while the phone is away
- **Also:**
  - [Coax Native](../../coax/native.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
