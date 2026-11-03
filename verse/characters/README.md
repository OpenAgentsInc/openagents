# Experimental Verse character profiles

[Veyr v2](veyr-v2.coax) is a dark-wizard dialogue prototype: the Ash Regent,
a former royal astronomer who binds the dead against the Long Winter.
His politeness, bargains and defensible obsession make him an antagonist
rather than a stream of stock threats. No character has been placed in a
realm or activated on the live NPC service.

The preferred experiment configuration uses the existing authenticated
`claude-opus-5-5` text client, low effort, with the character card, four style
examples and explicit encounter state. Generate a fresh single-turn line:

```sh
coax/web/tool.sh psionic/tools/villain-text.coax ask "My king promised me a place in history. What are you offering?"
```

This command makes an external model call through the installed client. It
prints only the reply; this wrapper saves no transcript by default. A third
argument explicitly records the provider response at a new file.
It is a prototype writer, not a deployed local Psionic model
or a persistent conversation server. The experiment also tests original
GPT-OSS-20B on Metal, including permanent weight edits and natural-channel
planning; their outputs and failures remain available separately.

`veyr-v2.json` has the existing Verse NPC brief's fields: name, role,
knowledge, greet. The role stays short; knowledge embeds the voice and
examples. It fits the current `verse.npc_request` limits of 60/120/4000/300
characters without truncation. Regenerate it with:

```sh
coax/web/tool.sh psionic/tools/villain-text.coax preset > verse/characters/veyr-v2.json
```

The exact tested writer renderer is `vt_prompt`; an existing Verse cloud
route renders the brief differently and uses a different model; its shared
warm/playful instruction also needs evaluation for an antagonist. The brief
is content for integration, not evidence of live-route quality or activation.

Keep permanent lore in the card. Pass the current quest state, inventory,
magic limitations and conversation separately on every turn. The final card
contains no fixed wounded companion: Mira appears only in scenes that need
her. Dialogue proposes or threatens; the game alone changes actual state.
The greeting is an authored, state-independent fallback, not a model sample.

`veyr.coax` preserves the first experimental card. Do not quietly replace it:
the original captures and bias candidates depend on its precise text.
Full protocol, raw outputs, development reviews and numerical evidence are
under [the experiment artifacts](../../psionic/fixtures/verse-villain/README.md).
