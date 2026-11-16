# 3. A transcript

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 2 keeps your messages in the state and shows only the last one.
This chapter draws all of them: a transcript above the composer, each
message a keyed component, its text as Markdown, scrolling under your
finger, and the view following the end while you are at the end.

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
and overscroll feel like iOS. One gesture has one owner. The end-follow rule
is in [`components/core/scroll.coax`](../../components/core/scroll.coax):
while you are at the end, a new message keeps the end in view; once you
scroll up to read, a new message does not move you.

A long conversation stays cheap. The transcript realizes only the rows on
screen, and its cost follows those rows, not the length of the
conversation. A clean surface redraws from its cached display list.

VoiceOver reads the transcript in order. Each message's row has the role of
a text block and its whole text as its label. A virtualized row that
VoiceOver asks for is realized.

## What we build

A plan for what this chapter ships once the draft is approved:

- The transcript surface above the composer, over the session record.
- `message.N` as a keyed component with Markdown text.
- The scroll offset as keyed state, driven by the platform's scroll
  physics, with the end-follow rule.
- Row realization for visible rows only.

It builds on what exists today:

- [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax)
- [`components/core/markdown.coax`](../../components/core/markdown.coax)
- [`components/kit/markdown.coax`](../../components/kit/markdown.coax)
- [`components/zeron/transcript.coax`](../../components/zeron/transcript.coax)
- [`components/core/scroll.coax`](../../components/core/scroll.coax)
- [`components/core/keyed.coax`](../../components/core/keyed.coax)

### The chapter's check

- **Source:** the transcript surface in the lean app, with its checks, and
  `openagents/examples/cases/transcript.ops`.
- **Trace:** send three messages, one with `**bold**`; `tree` lists
  `message.1` to `message.3` in order; `state scroll` prints `at end`; a
  drag up prints `not at end`, and a fourth send leaves the offset where it
  was.
- **Refusal:** a frame that gives two messages the same ID refuses that
  component's keyed state and counts it; `counts` prints
  `refused_duplicate 1`, and neither message takes the other's state.
- **Independent check:** the session record's own checks
  (`coax/bin/coax test coax/lib/agent/session/session.coax`) and the
  Markdown parser's checks, both written before this chapter.

Planned paths: `openagents/examples/cases/transcript.ops`. The transcript
draws the existing session record from
[`coax/lib/agent/session/`](../../coax/lib/agent/session).

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across devices
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, chat with the same thing everywhere
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`
- **Also:**
  - [GPU components, "Scrolling"](../../components/gpu-components.md#scrolling)
  - [The chat zero-base audit](../../docs/audits/2026-11-11-chat-zero-base-audit.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
