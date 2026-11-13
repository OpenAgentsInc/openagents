# Outcome replay

Issue: [#13438](https://github.com/OpenAgentsInc/openagents/issues/13438).
Source: [docs/research/summer-what-to-adapt.md](../research/summer-what-to-adapt.md), S5.
Code: `metaverse/splice-bots/replay.coax` (Splice, the first suite),
`metaverse/splice-bots/replay-checks.coax`, `metaverse/splice-bots/tools/replay.coax`.

A recorded run of operations is replayed into a fresh World. Four outcomes are
checked apart, because each can be wrong while the others are right.

| Check | What it holds | A way it goes wrong alone |
| --- | --- | --- |
| live | The World a page holds after taking each saved edit as it came | A page missed an event |
| saved | The document the server saved, opened in a fresh game and written again | The save is a step behind |
| errors | Every batch of the run was taken | A batch was refused |
| probes | What the run was for | A thing is in the state and has no collider |

What a run should leave is worked out apart from the realm: each batch as it
was taken is folded into a plain scene by the wire's own rules
(`we_scene_apply`). The page's layer and the saved document's are each held to
that, not to each other.

## Probes

| Probe | Holds when |
| --- | --- |
| stands N K | Thing N is of content ID K, in the layer, in the World, with a collider |
| blocks X Z | A body can't stand at X Z (the game's own collision) |
| free X Z | A body can stand there |
| count N | The layer holds N things |
| walks | The run's whole way can be walked: the game's own step and collision ran |

A failed probe names the link that broke, by the why chain's codes
([diagnostics](diagnostics.md), #13429): `not-in-state`, `despawned`,
`collider-missing`.

## Every check has a broken variant

A check that can't fail proves nothing. Each of the four is also run against a
replay broken on purpose, and must fail it, alone:

| Variant | The check that must fail |
| --- | --- |
| The page misses the run's last edit | live |
| The saved document is the one before the last batch | saved |
| A batch the world refuses is slipped in | errors |
| A placed thing's collider is taken out of the World | probes |

## Run it

```sh
coax/bin/coax test metaverse/splice-bots/splice-game.coax --only "outcome replay"   # the recorded run and its broken variants
coax/web/tool.sh metaverse/splice-bots/tools/replay.coax apps/openagents-web/build/.worlds/splice   # a run coax serve kept
```

The second command replays the steps the dev server kept for the world (each
edit an agent or a person made there). A run recorded without probes earns its
own: every thing it leaves stands and stops a body, and the run walks.

To add a run to the suite, copy its batches into `SPR_RUN`'s form in
`replay-checks.coax` with the probes that say what it was for.

## Not built

- A page's runtime errors (a trap in a frame) are not read: the replay runs
  the game's code natively, where a trap fails the check itself.
- Everglade's suite. Its world is not a world document yet (E13, #13422).
