# 3. A transcript

> **Status: partly implemented.** The shared transcript surface, Markdown
> parsing and rendering (`md_parse`, `markdown_height`, `markdown_block`),
> scroll state and end-follow rule (`scroll_clamp`, `scroll_at_end`,
> `transcript.scroll`, `transcript.jump_bottom`), per-surface dirty isolation,
> and driver operations (`openagents/examples/cases/transcript.ops`) pass on
> `cpu` in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
> [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
> and [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax).
> Remaining: wiring into the native desktop/iOS host scroll gestures and
> backing the transcript directly with the persisted `session.coax` record.

## Premise

Chapter 2 keeps your messages in the state and shows only the last one.
This chapter draws all of them on the primary desktop app (and mobile via
the same shared module): a transcript above the composer, each message a
keyed component, its text as Markdown, scrolling under the wheel or touch,
and the view following the end while you are at the end.

## Draft

The transcript is the session record in
[`coax/lib/agent/session/`](../../coax/lib/agent/session): one conversation
as its parts, the same for every engine and every surface. A part is your
message, the reply's text, thinking, or a tool call with its result. This
chapter has only your messages; chapter 4 adds replies. The screen reads the
record and draws what it finds. It never writes to it.

Each message is a component with a stable ID, `message.N`, where N is the
part's number in the record. Its state that outlives a frame, such as its
entry motion, belongs to that ID in the keyed store. Adding a message never
moves state from one message to another, and a message that leaves the
record retires its keyed state.

Text in a message is Markdown, parsed by the one Markdown parser in
[`components/core/markdown.coax`](../../components/core/markdown.coax) and
painted by the kit's Markdown component. No screen writes a second parser.
Display text uses the glyph atlas. A glyph the atlas lacks falls back to the
platform's text services; a missing glyph is a regression.

Coax owns the scroll offset, as keyed state on the transcript's ID. The
host drives that offset from the platform's own scroll physics, so momentum
and wheel or touch scrolling feel native on desktop and mobile alike. One
gesture has one owner. The end-follow rule is in
[`components/core/scroll.coax`](../../components/core/scroll.coax): while
you are at the end, a new message keeps the end in view; once you scroll up
to read, a new message does not move you and a jump-to-bottom control
(`transcript.jump_bottom`) lets you return to the end.

A long conversation stays cheap. The transcript realizes only the rows on
screen, and its cost follows those rows, not the length of the
conversation. A clean surface redraws from its cached display list.

VoiceOver reads the transcript in order. Each message's row has the role of
a text block and its whole text as its label. A virtualized row that
VoiceOver asks for is realized.

## What we build

Implemented on `cpu` in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), grounded in the primary desktop app ([`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax)) and consumed by mobile ([`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax)):

- The transcript `UiSurface` above the composer, independent of the composer
  surface's dirty marks.
- `message.N` as a keyed semantic row with Markdown parsing and painting via
  `md_parse`, `markdown_height`, and `markdown_block`.
- Scroll state (`scroll_y`, `content_h`, `viewport_h`, `unpinned`,
  `unread_below`) driven by `transcript.scroll` and `transcript.jump_bottom`
  actions using `scroll_clamp` and `scroll_at_end`.
- Visible-row culling in `cs_layout_transcript`, skipping paint for messages
  outside the viewport while tracking unread messages below when unpinned.
- Driver case [`openagents/examples/cases/transcript.ops`](../examples/cases/transcript.ops)
  and expected trace [`openagents/examples/cases/transcript.trace`](../examples/cases/transcript.trace).

It builds on what exists today:

- [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax)
- [`components/core/markdown.coax`](../../components/core/markdown.coax)
- [`components/kit/markdown.coax`](../../components/kit/markdown.coax)
- [`components/zeron/transcript.coax`](../../components/kit/markdown.coax)
- [`components/core/scroll.coax`](../../components/core/scroll.coax)
- [`components/core/keyed.coax`](../../components/core/keyed.coax)

### The chapter's check

- **Source:** [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
  [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
  [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax),
  and [`openagents/examples/cases/transcript.ops`](../examples/cases/transcript.ops).
- **Trace:** send three messages, one with `**bold**`; `tree` lists
  `message.1` to `message.3` in order; `state scroll` prints `at end`; a
  scroll up prints `not at end`, a fourth send leaves the offset where it
  was and reveals `transcript.jump_bottom`, and tapping `transcript.jump_bottom`
  returns to `at end`.
- **Refusal:** a frame that gives two messages the same ID refuses that
  component's keyed state and counts it; `counts` prints
  `refused_duplicate 1`, and neither message takes the other's state.
- **Independent check:** the session record's own checks
  (`coax/bin/coax test coax/lib/agent/session/session.coax`), the Markdown
  parser's checks (`coax/bin/coax test components/core/markdown.coax`), and
  the scroll helper checks (`coax/bin/coax test components/core/scroll.coax`).
- **Commands:** `coax/bin/coax test apps/shared/chat/screen.coax`,
  `coax/bin/coax test apps/openagents-desktop/chat_screen.coax`, and
  `coax/bin/coax test apps/openagents-ios/composer_screen.coax`.

Implemented paths: [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax), [`apps/openagents-ios/composer_screen.coax`](../../apps/openagents-ios/composer_screen.coax), [`openagents/examples/cases/transcript.ops`](../examples/cases/transcript.ops), [`openagents/examples/cases/transcript.trace`](../examples/cases/transcript.trace).

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across devices
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, chat with the same thing everywhere
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`
- **Also:**
  - [GPU components, "Scrolling"](../../components/gpu-components.md#scrolling)
  - [The chat zero-base audit](../../docs/audits/2026-11-11-chat-zero-base-audit.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Source listing

Pin-to-bottom invariant in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), exercised by [`openagents/examples/chat-screen-flow.coax`](../examples/chat-screen-flow.coax):

<!-- source: apps/shared/chat/screen.coax transcript-pin -->
```jai
chat_state_pinned :: (st: [] ChatState) -> bool {
  if st.count < 1 { return true; }
  return !st[0].unpinned;
}
```

Planned paths: none — extends existing `apps/shared/chat/screen.coax`, `apps/openagents-desktop/page.coax`, and `apps/openagents-ios/composer_screen.coax`.
