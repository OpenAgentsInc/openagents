# Character chat, side by side with the demo

Task `pt-chat-endpoint` (#b552) of the Verse project. Asked 2026-10-28 by claude-verse-3: the same five
questions, the same character payloads, to the live demo (`POST https://playtest.openagents.com/api/npc/ask`)
and to the new endpoint on staging (`POST https://staging.openagents.com/api/playtest/npc/ask`,
`coax/web/api/npc.c`).

**What the demo's players get.** The demo's endpoint asks Claude (`tools/npc_chat.py`, `claude-opus-5-5`)
only when `ANTHROPIC_API_KEY` is set. On the live service it is not: every question answered
`503 {"error": "no model configured"}`, so the page answered from its own lines
(`web/characters.js` `Characters.converse`, run here in Node from tag `demo/kiki-2026-10-28d`).
Those scripted lines are the demo column. The new endpoint sends the demo's prompt, memory and
taught facts to Gemini 3.8 Flash on Vertex AI; a 503 from it still leaves the page its own lines.

## Mortimer, a greeting

Character: Mortimer, the butler. World: Haunted House.

**Question:** Hello! Who are you?

| | Answer |
| --- | --- |
| Demo (endpoint: 503; the page's lines) | I'm Mortimer. |
| openagents.com (gemini-3.8-flash, 2.0 s) | Good evening. The master is... indisposed, so I am Mortimer, your humble butler in this delightfully drafty house. It is a genuine pleasure to welcome a fresh soul to our halls. |

## Mortimer, how to play

Character: Mortimer, the butler. World: Haunted House.

**Question:** How do I put a tombstone in the yard?

| | Answer |
| --- | --- |
| Demo (endpoint: 503; the page's lines) | I'm new here and nobody has taught me anything yet. To teach me, press B and type "teach Mortimer that" and what I should know. |
| openagents.com (gemini-3.8-flash, 2.4 s) | Press B to open edit mode, my friend. Then you may simply type or say "add a tombstone in front of me," or pick one from your hotbar with the number keys and click to place it. Do make yourself at home among the graves. |

## Mortimer, something he can't know

Character: Mortimer, the butler. World: Haunted House.

**Question:** What's the weather in Paris today?

| | Answer |
| --- | --- |
| Demo (endpoint: 503; the page's lines) | No idea, sorry. Ask me how to build, or teach me something. |
| openagents.com (gemini-3.8-flash, 2.4 s) | I'm afraid my duties keep me rather strictly confined to these drafty halls, so Paris remains quite a mystery to me today. I can assure you, however, that it is perpetually dreary and delightfully damp right here. |

## Agatha, what players taught her

Character: Agatha, the cook; taught: "The cellar key is under the kitchen rug. The ghost in the attic was once the gardener.". World: Haunted House.

**Question:** Where can I find the cellar key?

| | Answer |
| --- | --- |
| Demo (endpoint: 503; the page's lines) | The cellar key is under the kitchen rug. |
| openagents.com (gemini-3.8-flash, 2.2 s) | You'll find that old key tucked right under the kitchen rug, dear. Mind the dust when you lift it! |

## Agatha, with memory

Character: Agatha, the cook; taught: "The cellar key is under the kitchen rug. The ghost in the attic was once the gardener.". World: Haunted House.

Before: Player: Who haunts the attic? / Agatha: That would be old Silas, our gardener once.

**Question:** Why did he stay?

| | Answer |
| --- | --- |
| Demo (endpoint: 503; the page's lines) | Hmm, nobody has told me about that. To teach me, press B and type "teach Agatha that" and what I should know. |
| openagents.com (gemini-3.8-flash, 2.0 s) | Bless him, I couldn't rightly say why he lingers up there. Maybe he just misses his roses, or perhaps he's waiting for someone to finish what he started. |

## Reading it

- Taught facts come back in both (Agatha's cellar key); the model also uses the memory (who stayed
  in the attic) where the scripted lines fall back to "nobody has told me".
- Game how-tos: the prompt carries the demo's controls text, so "How do I put a tombstone in the
  yard?" gets the edit-mode answer; the scripted lines missed it (no small-talk match).
- Questions a character could not know are answered in character, as the prompt asks.
- Answers are one to three sentences, as the demo's prompt asks; about 2 s each.
