# Publish and share

Publishing makes a plugin's test result public, so other people can
rerun it, build on it, and use the plugin.

## Publish a result

```sh
openagents plugin test publish evals/results/TIMESTAMP/report.json
```

On the phone, it's **Add to the Gym**. Publishing puts two signed records
on Nostr, the open protocol OpenAgents runs on: your test set, released
under your key, and your result. A result is public whatever it says,
**Worse** included.

There is no review queue. Nobody approves a plugin, a test set, or a
result before it's published; anyone with a key can publish, and what
decides whether a plugin is used is what other people's runs show.

Today the `openagents` command publishes your test set and your result,
not the plugin's own release. Our hosted runner releases the catalog
plugins it tests, which is how the three example plugins were released.
Publishing a plugin's release from your own computer is designed but not
built yet.

## Checks

Anyone else can rerun your tests on your plugin:
`openagents plugin test check RESULT_ID` (in the app, **Check a result**).
A check publishes its own result that confirms yours when the verdicts
match and disputes it when they don't. Both stay visible.

## Validation

A check reruns *your* tests. A validation answers whether your plugin
only does well on tests you wrote: someone other than you writes a
second test set, releases it after your plugin's release
(`openagents plugin test release`), runs it, and publishes the result
with `--validates RESULT_ID`. It counts when it is **Better**.

## Adoption into Coder

A plugin becomes a candidate for Coder's defaults, the plugins Coder uses
for everyone, when its result is **Better**, at least three checks by
different people confirmed it, and at least one validation is **Better**.
Adoption is then a decision an OpenAgents operator signs, never
automatic. Computers pick up the defaults with
`openagents plugin defaults sync`, and our hosted runner reads them on
each run.

This has happened once, on 2026-09-29. Every key in that record came
from one machine and one operator, which the
[record](https://github.com/OpenAgentsInc/openagents/blob/main/docs/extensions/measurements/2026-09-29-first-adoption.md)
says.

## Credit

Credit is XP and your name, not money. It comes from two events only,
and each quest sets the amounts; in the current quests:

- **Someone reruns your result to protocol.** The checker earns 50 XP,
  the original tester 25, and the test set's author 25, whether the
  check confirms or disputes.
- **Your plugin is adopted.** Its author earns 200 XP, the author of the
  tests that showed it 100, and each tester whose result was confirmed 50.

Running, publishing, viewing, or downloading earns nothing. XP can't be
spent, transferred, or converted. Your profile in the app shows what you
made and the XP it earned.

## On the map

In OpenAgents for Mac, the **Map** page shows each plugin and the step it
has reached: Not packaged, Candidate, a verdict, Reproduced, Validated,
or Adopted, with the next step for each. See [Decks and the Map](/docs/decks).

The specifications are on GitHub:
[testing and publishing](https://github.com/OpenAgentsInc/openagents/blob/main/docs/extensions/evaluation.md),
and the protocols for
[plugins](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-EXT.md),
[results](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-EVAL.md),
[workflows](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-PRG.md),
[knowledge](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-KB.md),
and [XP](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-XP.md).

Next: [The Gym and XP](/docs/gym-and-xp).
