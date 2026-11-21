# 2. State and actions

> **Status: partly implemented.** The shared compiled Coax state record,
> serialized action dispatcher (`composer.edit`, `composer.send`), all-or-nothing
> refusal on empty draft, per-surface dirty marks (`transcript` and `composer`),
> frame redraw gating, `composer.send` button (`ctx_button`), driver ops
> (`openagents/examples/cases/send.ops`), and unit/`#forall` checks pass on
> `cpu` across [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
> [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
> and [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax).
> Native desktop/iOS host wiring and accessibility UI tests remain pending.

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
leaves the machine. Chapter 4 sends it.

## What we build

Implemented on `cpu` in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax) and consumed by [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax) (primary) and [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax) (mobile):

- The chat page's state record (`ChatState`): the draft (`EditBuf`), the sent
  messages (`ChatMessage` with stable domain keys `message.<id>`), and
  `send_enabled`.
- Serialized action dispatch (`cs_dispatch` / `chat_state_apply`) for
  `composer.edit` and `composer.send`, with all-or-nothing refusal on an empty
  draft (and on full message buffer or re-entrant dispatch).
- Per-surface dirty marks (`dirty_composer`, `dirty_transcript`) across two
  `UiSurface`s (`surface` and `transcript`) and frame redraw only when a
  surface is dirty or motion is due (`composer_needs_redraw`).
- Send as a frame-context button (`ctx_button`), `composer.send`, beside
  `composer.input`, disabled when the draft is empty.
- Driver case [`openagents/examples/cases/send.ops`](../examples/cases/send.ops)
  and expected trace [`openagents/examples/cases/send.trace`](../examples/cases/send.trace).

It builds on:

- [`coax/native.md`](../../coax/native.md)
- [`coax/checked-state.md`](../../coax/checked-state.md)
- [`components/gpu-components.md`](../../components/gpu-components.md#state-and-invalidation)
- [`components/kit/controls.coax`](../../components/kit/controls.coax) (`ctx_button`)
- [`components/paint/surface.coax`](../../components/paint/surface.coax)

### The chapter's check

- **Source:** [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
  [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
  [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax),
  and [`openagents/examples/cases/send.ops`](../examples/cases/send.ops).
- **Trace:** `type composer.input hello`, `tap composer.send`, `state
  messages` prints `you: hello`, `state draft` prints an empty line, and
  `tree` shows `message.1`.
- **Refusal:** `tap composer.send` with an empty draft answers
  `composer.send can't be pressed now` and leaves the state unchanged.
- **Independent check:** a `#forall` check that any sequence of refused
  actions leaves the state equal to its start, and a check that an
  incremental frame and a full rebuild commit the same rows.
- **Commands:** `coax/bin/coax test apps/shared/chat/screen.coax`,
  `coax/bin/coax test apps/openagents-desktop/chat_screen.coax`, and
  `coax/bin/coax test apps/openagents-ios/composer_screen.coax`.

Generic dispatch code goes in the proposed `coax/lib/app/` that
[Coax Native](../../coax/native.md#architecture) describes, not in this book.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): one product behind every surface
- **Also:**
  - [Coax Native](../../coax/native.md) and the [Exact research](../../docs/research/exact/README.md) it draws on
  - [Exact: what to adapt](../../docs/research/exact/03-what-to-adapt.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Source listings & runnable example

Explicit state, action, and dispatch result types in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax):

<!-- source: apps/shared/chat/screen.coax state-types -->
```jai
ChatState :: struct {
  draft: [1] EditBuf;
  messages: [CS_MAX_MESSAGES] ChatMessage;
  message_count: int;
  next_message_id: int;
  send_enabled: bool;
  // Chapter 3: transcript scroll & pin-to-bottom state.
  // Zero-initialized state starts pinned to bottom (!unpinned).
  scroll_y: f32;
  content_h: f32;
  viewport_h: f32;
  unpinned: bool;
  unread_below: int;
}

ChatAction :: struct {
  kind: int;
  role: int;
  text: string;
  delta_y: f32;
}

ChatDispatchResult :: struct {
  accepted: bool;
  reason: int;
  dirty_composer: bool;
  dirty_transcript: bool;
  seq: int;
}
```

Serialized, re-entrancy-refusing dispatch:

<!-- source: apps/shared/chat/screen.coax dispatch -->
```jai
// Serialized action dispatch on the screen.
cs_dispatch :: (s: [] ComposerScreen, act: ChatAction) -> ChatDispatchResult {
  if s.count < 1 { return ChatDispatchResult.{accepted = false, reason = CS_DISPATCH_REFUSED_UNKNOWN}; }
  if s[0].dispatching {
    s[0].actions_refused += 1;
    return ChatDispatchResult.{accepted = false, reason = CS_DISPATCH_REFUSED_REENTRANT, seq = s[0].action_seq};
  }
  s[0].dispatching = true;
  res := chat_state_apply(s[0].state[..], act);
  s[0].dispatching = false;
  if !res.accepted {
    s[0].actions_refused += 1;
    res.seq = s[0].action_seq;
    return res;
  }
  s[0].action_seq += 1;
  res.seq = s[0].action_seq;
  if res.dirty_composer { s[0].dirty_composer = true; }
  if res.dirty_transcript { s[0].dirty_transcript = true; }
  return res;
}
```

Run [`openagents/examples/chat-screen-flow.coax`](../examples/chat-screen-flow.coax) (committed transcript [`openagents/examples/chat-screen-flow.out`](../examples/chat-screen-flow.out)):

```shell
coax/bin/coax run openagents/examples/chat-screen-flow.coax
```

Expected output:

```text
empty_send accepted=false reason=1 count=0
edit accepted=true send_enabled=true dirty_composer=true dirty_transcript=false
send accepted=true count=1 pinned=true draft_len=0
reply accepted=true count=2 pinned=false unread=1
jump pinned=true unread=0
```

Planned paths: none — extends existing `apps/shared/chat/screen.coax`, `apps/openagents-desktop/page.coax`, and `apps/openagents-ios/composer_screen.coax`.
