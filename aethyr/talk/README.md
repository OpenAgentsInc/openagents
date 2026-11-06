# Bounded character conversations

The native CPU teaching profile for [chapter 77](../book/ch077-people-who-talk-back.md).
No realm or production route consumes these modules yet.

| Module | Owns |
| --- | --- |
| `character.coax` | Authored character identity, UTF-8 field limits and required fallback |
| `ask.coax` | Request ownership, logical deadlines, bounded history and admitted replay |
| `wire.coax` | The existing NPC ask JSON and complete reply admission |

Call `talk_open`, then `talk_begin` on the realm's accepted tick. Only
`TalkStart.send` permits the adapter to send a request. Capture the complete
`TalkKey` with the SDK operation. Feed its accepted result to `talk_receive`
with that same key; call `talk_tick` every realm tick, including while waiting.
The adapter must not reinterpret answer words as commands.

`talk_replay` admits a complete typed record before replacing the current
conversation. Replayed questions return `replay_wait`; SDK callbacks are
refused. The caller must supply every recorded apply tick. The record holds
eight exchanges; history holds the last four pairs. `talk_line_text` views a
constructed or admitted line. External records require `talk_replay` admission
before their lines reach that accessor.

The request writer and reply decoder publish to the caller's buffer only
after complete success. `TalkReadStatus` carries a developer diagnostic;
ordinary model or transport failure uses the character's authored fallback.
The example transport is a deterministic fixture, with no network or model.
Local player IDs fence replies; they do not grant service permissions.

```sh
coax/bin/coax test aethyr/talk/wire.coax --library
coax/bin/coax mutate aethyr/talk/character.coax --library --root-only
coax/bin/coax mutate aethyr/talk/ask.coax --library --root-only
coax/bin/coax mutate aethyr/talk/wire.coax --library --root-only
coax/bin/coax run aethyr/examples/talk-and-replay.coax
```

[Qualification and remaining integrations](../../docs/verification/aethyr-talk-2026-11-06/README.md)
keep native component behavior separate from live model, host, voice and realm
adoption. The existing server's daily cap, facts and earlier-visit memory are
unchanged.
