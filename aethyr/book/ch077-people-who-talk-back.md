# 77. People who talk back

> **Status: unimplemented — design draft.** Today characters answer through
> `POST /api/playtest/npc/ask` in
> [`coax/web/api/npc.c`](../../coax/web/api/npc.c): the page sends the
> character, the world, the recent history and the question, the server asks
> Gemini on Vertex AI, and a failure leaves the page its own scripted lines.
> Prompts, memory and the daily cap live in SQL. Each realm wires this
> itself. This chapter plans one character-conversation module for every
> realm. Nothing in `aethyr/` runs yet.

You can walk up to a character in a realm and ask it something. It answers
in character, remembers what you told it on earlier visits, and still says
something useful when the model is unreachable. This chapter builds that
path as an engine feature, with the same bounds everywhere.

## What this chapter teaches

- How a character record (name, role, knowledge, greeting) becomes a
  bounded request.
- How memory and taught facts reach the prompt, and who may add them.
- How to answer from the realm's own lines when the model fails, so play
  never waits.
- How to keep the answer text separate from game state: talk never moves a
  character, gives an item or changes a rule.
- How to route what a player means through a typed selector, not keyword
  matching.

## Outline

1. **The request.** The body is `{npc: {name, role, knowledge, greet},
   world, history: [{from, text}], question, controls?, visit?}`, with the
   optional `X-Verse-Player` key. A request without a question is a 400.
   Aethyr defines this as a protocol record, with size limits on every
   field.
2. **The model and its limits.** The server asks `OA_NPC_MODEL` (default
   `gemini-3.8-flash`) with the service's own token. `OA_NPC_DAILY_CAP`
   (2,000) answers a day apply across the server. A child process makes the
   call so the server keeps answering. Answers are one to three sentences.
3. **Memory and taught facts.** With a player key, what that player said on
   earlier visits goes into the prompt (migration `0031_npc_memory.sql`).
   Players teach facts with "teach NAME that ..." in edit mode. The chapter
   states who may teach a character in each realm and how a fact is removed.
4. **Falling back.** A 503 leaves the page its own lines, as the demo's
   `Characters.converse` did. Aethyr makes the fallback a required part of
   every character: a realm without scripted lines fails its check.
5. **Typed intent.** When talk should change the world (a set piece, an
   edit), the server's selector picks one entry from a catalogue under a
   response schema, and the game checks it again
   ([chapter 79](ch079-a-piece-from-a-sentence.md)). No route matches words
   in the sentence. A character's free-text answer never reaches the
   simulation.
6. **Voice.** Spoken questions arrive as text from the voice host, and
   answers can be spoken back. Placement of a character's voice in the
   world uses [chapter 68](ch068-sound-in-the-world.md).
7. **Recording and replay.** A session records the answer text it received,
   not the request to the model, so a replay shows the same conversation
   without calling the model.
8. **Measurement.** The side-by-side comparison in
   [npc-chat-side-by-side.md](../../docs/verse/npc-chat-side-by-side.md)
   (five questions, about 2 s an answer) becomes a repeatable evaluation with
   fixed characters and questions.

## What exists today

- [coax/web/api/npc.c](../../coax/web/api/npc.c): the ask route, the set
  piece route and the spoken realm edit route.
- [coax/web/api/migrations/0005_verse_npc.sql](../../coax/web/api/migrations/0005_verse_npc.sql):
  the prompt, every question and answer, and the daily cap.
- [coax/web/api/migrations/0031_npc_memory.sql](../../coax/web/api/migrations/0031_npc_memory.sql):
  per-player memory.
- [docs/verse/npc-chat-side-by-side.md](../../docs/verse/npc-chat-side-by-side.md):
  the comparison with the earlier demo's scripted answers.
- Callers: [apps/openagents-web/realm.coax](../../apps/openagents-web/realm.coax)
  and [myrealm.coax](../../apps/openagents-web/myrealm.coax).

## What it builds

- `aethyr/talk/character.coax`: the character record, its scripted lines
  and its limits.
- `aethyr/talk/ask.coax`: the request and answer records, the fallback and
  the replay record.
- A shared server route that every realm uses, replacing per-realm wiring.

## Acceptance checks and oracles

The chapter is done when:

- Every realm with characters uses `aethyr/talk/`, and each character has
  scripted fallback lines.
- With the model unreachable, every character answers from its lines within
  one frame of the failure.
- A request over a field limit is refused with a 400 and counted.
- A replayed session shows the same answers and makes no model call.
- No answer text changes game state, checked by a fixture that returns
  instructions in the answer.
- The fixed evaluation set keeps taught facts (the cellar key under the
  kitchen rug) in every answer that needs them.

## Dependencies

Chapters 5 (replay), 6 (stable entity IDs for characters), 15 (the UI that
shows answers) and 68 (spoken answers in the world). Coax
[chapter 18](../../coax/book/ch18-protocol.md) for the protocol boundary.
