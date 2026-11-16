# 2. State and actions

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 1's field holds text, and nothing happens when you press Send. This
chapter gives the app its model: one state record, actions that change it,
dirty marks that say which surface to draw again, and one frame that draws
it. Send becomes an action that moves the draft into the state as your
first message.

## Draft

An app on Coax Native is its state and its actions
([Coax Native, "Application state and dispatch"](../../coax/native.md#application-state-and-dispatch)).
The state is an explicit record the app owns. For now it holds three
things: the draft, the messages you sent, and whether Send can be pressed.
An action is a named, typed change to that record. This chapter has two:
`composer.edit`, which sets the draft, and `composer.send`, which moves the
draft into the messages and clears it.

Every input reaches the app as an action. A tap, a key, an edit from the
platform editor, an accessibility action and a driver operation all enter
one serialized dispatcher. A host never changes state on its own, and it
never calls back into Coax from the middle of a platform call. It queues an
event and wakes the dispatcher.

An action is all or nothing. It checks its inputs, works out the whole new
state, and then commits it. If it refuses, you see no half-changed screen.
`composer.send` with an empty draft refuses, and Send is drawn disabled so
you can see that before you press it. Use the existing
[checked state](../../coax/checked-state.md) mechanism for this.

An action marks what it changed. The dirty mark is per surface: a change to
the draft marks the composer, not the whole screen. A frame runs only when a
surface is dirty or a motion is due. The frame lays out, paints and records
rows for the dirty surface and commits it
([GPU components, "The frame"](../../components/gpu-components.md#the-frame)):

```text
input, timer or completion
        -> action (validates, changes state, marks surfaces dirty)
        -> frame: components lay out, paint and record rows
        -> commit: display list + hit regions + semantic rows, together
        -> gpu_draw -> present
```

Views derive from state. A view reads committed state and calls
components. It never starts a request, reads a clock or changes state. A
forgotten dirty mark shows stale UI, so state fields change only inside
actions, and a check compares an incremental frame with a full rebuild.

Stable IDs start here. Every piece of state that you can see or press has
an ID made from its role and its domain key, such as `composer.send` or
`message.1`. An ID never comes from a list index or a pixel position.

There are no effects yet. Send adds your message to the state and nothing
leaves the phone. Chapter 4 sends it.

## What we build

A plan for what this chapter ships once the draft is approved:

- The chat page's state record: the draft, the sent messages, Send's state.
- The actions `composer.edit` and `composer.send`, with the refusal for an
  empty draft.
- Per-surface dirty marks and the frame that commits only dirty surfaces.
- Send as a frame-context button, `composer.send`, beside the field.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/checked-state.md`](../../coax/checked-state.md)
- [`components/gpu-components.md`](../../components/gpu-components.md#state-and-invalidation)
- [`components/kit/controls.coax`](../../components/kit/controls.coax) (`ctx_button`)
- [`components/paint/surface.coax`](../../components/paint/surface.coax)

### The chapter's check

- **Source:** the chat page's state and actions in the lean app, with their
  checks, and `openagents/examples/cases/send.ops`.
- **Trace:** `type composer.input hello`, `tap composer.send`, `state
  messages` prints `you: hello`, `state draft` prints an empty line, and
  `tree` shows `message.1`.
- **Refusal:** `tap composer.send` with an empty draft answers
  `composer.send can't be pressed now` and leaves the state unchanged.
- **Independent check:** a `#forall` check that any sequence of refused
  actions leaves the state equal to its start, and a check that an
  incremental frame and a full rebuild commit the same rows.

Planned paths: `openagents/examples/cases/send.ops`. Generic dispatch code
goes in the proposed `coax/lib/app/` that
[Coax Native](../../coax/native.md#architecture) describes, not in this
book.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): one product behind every surface
- **Also:**
  - [Coax Native](../../coax/native.md) and the [Exact research](../../docs/research/exact/README.md) it draws on
  - [Exact: what to adapt](../../docs/research/exact/03-what-to-adapt.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
