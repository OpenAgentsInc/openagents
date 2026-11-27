# 3. A transcript

> **Status: partly implemented.** The shared transcript surface, Markdown
> parsing and rendering (`md_parse`, `markdown_height`, `markdown_block`),
> scroll state and end-follow rule (`scroll_clamp`, `scroll_at_end`,
> `transcript.scroll`, `transcript.jump_bottom`), per-surface dirty isolation,
> and driver operations (`openagents/examples/cases/transcript.ops`) pass on
> `cpu` in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
> [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
> and [`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax).
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

Implemented on `cpu` in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), grounded in the primary desktop app ([`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax)) and consumed by mobile ([`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax)):

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
- [`apps/openagents-desktop/page/transcript.coax`](../../apps/openagents-desktop/page/transcript.coax)
- [`components/core/scroll.coax`](../../components/core/scroll.coax)
- [`components/core/keyed.coax`](../../components/core/keyed.coax)

### The chapter's check

- **Source:** [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
  [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
  [`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax),
  [`openagents/examples/transcript-scroll.coax`](../examples/transcript-scroll.coax),
  and [`openagents/examples/cases/transcript.ops`](../examples/cases/transcript.ops).
- **Trace:** committed debug/release stdout in [`openagents/examples/transcript-scroll.out`](../examples/transcript-scroll.out)
  and [`openagents/examples/cases/transcript.trace`](../examples/cases/transcript.trace):
  editing dirties only the composer (`composer_redraws=2 transcript_redraws=1`);
  sending commits `transcript.list` and `message.1`; appending six Markdown replies
  overflows the viewport while staying pinned (`has_jump=false`); scrolling up
  (`scroll -120`) unpins, exposes `transcript.jump_bottom`, and keeps `scroll_y`
  stationary while incrementing `unread=1` on a new reply; tapping `transcript.jump_bottom`
  re-pins and clears `unread=0`.
- **Refusal:** tapping `transcript.jump_bottom` while already pinned answers
  `no such control`, and appending an empty message is refused (`accepted=false reason=1`)
  leaving `message_count` unchanged.
- **Independent check:** the session record's own checks
  (`coax/bin/coax test coax/lib/agent/session/session.coax`), the Markdown
  parser's checks (`coax/bin/coax test components/core/markdown.coax`), and
  the scroll helper checks (`coax/bin/coax test components/core/scroll.coax`).
- **Commands:** `coax/bin/coax run openagents/examples/transcript-scroll.coax`,
  `coax/bin/coax test openagents/examples/transcript-scroll.coax`, and
  `coax/bin/coax test apps/shared/chat/screen.coax`.

Implemented paths: [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax), [`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax), [`openagents/examples/cases/transcript.ops`](../examples/cases/transcript.ops), [`openagents/examples/cases/transcript.trace`](../examples/cases/transcript.trace).

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

Pin-to-bottom invariant in [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax), exercised by [`openagents/examples/transcript-scroll.coax`](../examples/transcript-scroll.coax) (committed transcript [`openagents/examples/transcript-scroll.out`](../examples/transcript-scroll.out)):

<!-- source: apps/shared/chat/screen.coax transcript-pin -->
```jai
chat_state_pinned :: (st: [] ChatState) -> bool {
  if st.count < 1 { return true; }
  return !st[0].unpinned;
}
```

<!-- source: openagents/examples/transcript-scroll.coax transcript-scroll-flow -->
```jai
main :: () {
  s: [1] ComposerScreen;
  o: [1] CsOut;
  buf: [4096] u8;
  cs_test_atlas(s[0].atlas[..]);
  s[0].form_factor = CHAT_FORM_DESKTOP;
  _ := cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);

  // Typing dirties only the composer surface, never the transcript surface.
  _ = cs_dispatch(s[..], ChatAction.{kind = CS_ACT_EDIT, text = "hello"});
  _ = cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);
  print("after_edit composer_redraws=% transcript_redraws=%\n", s[0].composer_redraws, s[0].transcript_redraws);

  // Sending commits message.1 and dirties both surfaces.
  _ = cs_dispatch(s[..], ChatAction.{kind = CS_ACT_SEND});
  _ = cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);
  print("after_send has_list=% has_msg1=% pinned=%\n",
    cs_find_transcript(s[..], "transcript.list") >= 0,
    cs_find_transcript(s[..], "message.1") >= 0,
    chat_state_pinned(s[0].state[..]));

  // Append 6 Markdown assistant replies so content overflows the 420px viewport.
  for i: 0..<6 {
    _ = cs_dispatch(s[..], ChatAction.{
      kind = CS_ACT_APPEND,
      role = CHAT_ROLE_ASSISTANT,
      text = "# Heading\n\nBody with **bold** and `code`.\n\n- item one\n- item two"
    });
  }
  _ = cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);
  print("overflow count=% overflows=% pinned=% has_jump=%\n",
    s[0].state[0].message_count,
    s[0].state[0].content_h > s[0].state[0].viewport_h,
    chat_state_pinned(s[0].state[..]),
    cs_find_transcript(s[..], "transcript.jump_bottom") >= 0);

  // Refusal 1: tapping jump_bottom while already pinned is refused (control not present).
  print("refuse_jump_when_pinned=%\n", ts_drive_line(s[..], "tap transcript.jump_bottom", buf[..]));

  // Refusal 2: appending an empty message is refused and leaves message_count unchanged.
  rempty := cs_dispatch(s[..], ChatAction.{kind = CS_ACT_APPEND, role = CHAT_ROLE_ASSISTANT, text = ""});
  print("refuse_empty_append accepted=% reason=% count=%\n", rempty.accepted, rempty.reason, s[0].state[0].message_count);

  // Scroll up unpins and exposes transcript.jump_bottom; appending increments unread_below without moving scroll_y.
  _ = ts_drive_line(s[..], "scroll -120", buf[..]);
  _ = cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);
  prev_y := s[0].state[0].scroll_y;
  _ = cs_dispatch(s[..], ChatAction.{kind = CS_ACT_APPEND, role = CHAT_ROLE_ASSISTANT, text = "New reply while reading above"});
  _ = cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);
  print("unpinned_append pinned=% same_scroll=% unread=% has_jump=%\n",
    chat_state_pinned(s[0].state[..]),
    s[0].state[0].scroll_y == prev_y,
    s[0].state[0].unread_below,
    cs_find_transcript(s[..], "transcript.jump_bottom") >= 0);

  // Jump to bottom re-pins, clears unread_below, and hides transcript.jump_bottom.
  print("jump_tap=%\n", ts_drive_line(s[..], "tap transcript.jump_bottom", buf[..]));
  _ = cs_render(s[..], o[0].v[..], 1280, 420, 1, 0.016, false);
  print("after_jump pinned=% unread=% has_jump=%\n",
    chat_state_pinned(s[0].state[..]),
    s[0].state[0].unread_below,
    cs_find_transcript(s[..], "transcript.jump_bottom") >= 0);
}
```

```shell
coax/bin/coax run openagents/examples/transcript-scroll.coax
```

Expected output:

```text
after_edit composer_redraws=2 transcript_redraws=1
after_send has_list=true has_msg1=true pinned=true
overflow count=7 overflows=true pinned=true has_jump=false
refuse_jump_when_pinned=no such control
refuse_empty_append accepted=false reason=1 count=7
unpinned_append pinned=false same_scroll=true unread=1 has_jump=true
jump_tap=tapped transcript.jump_bottom
after_jump pinned=true unread=0 has_jump=false
```

Planned paths: none — extends existing `apps/shared/chat/screen.coax`, `apps/openagents-desktop/page.coax`, and `apps/openagents-ios/gpu-page.coax`.
