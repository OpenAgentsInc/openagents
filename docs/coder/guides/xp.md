# Referee quests and read the XP ledger

This guide covers `microcoder xp`: publishing a quest, accepting a
completion as an award, revoking an award, and reading the XP ledger from a
relay. [NIP-XP](../../../nips/openagents/NIP-XP.md) specifies the events,
and the [knowledge-base guide](knowledge-base.md) covers the entries and
evidence a knowledge quest is completed with.

XP is evidence of an accepted outcome. It's awarded once per quest version,
never per run, commit, token, or hour, and it can't be spent, transferred,
or converted. Sats for beating a quest are a separate payment contract that
doesn't exist yet; nothing here moves money.

`microcoder xp --help` prints the command list.

## How a knowledge quest works

1. A **referee** publishes a quest: for example, "beat Fable 5.1 low's
   cheapest winning run on `fix-git`". The quest is frozen: its task, bar,
   season, and award never change.
2. An **author** writes and publishes a knowledge entry with
   `microcoder kb publish`.
3. A **runner**, someone other than the author, runs paired Microcoder runs
   with and without the entry and publishes the evidence with
   `microcoder kb publish-evidence`.
4. The referee checks the evidence against the quest's rule and, when it
   passes, publishes an award that credits the author and the runner.
5. Each **reader** derives XP from the awards of the referees it trusts,
   re-checking every award against the signed entry and evidence.

The rule, `kb-transfer`, accepts a completion only when all of these hold:
the runner isn't the author; the entry wasn't written from the quest's
task; the report's verdict is `pass`; the report pairs runs on the quest's
task; and on that task the with-entry runs pass at the quest's pass rate
and cost less per run than the quest's bar. The first accepted completion
of a quest version earns its award, once.

## Before you begin

- The referee key is `~/.openagents/nostr/referee-key`. The first `xp quest`,
  `xp award`, or `xp revoke` creates it with mode `0600`. Pass `--key PATH`
  to use another. The key is never printed; the commands print its `npub`.
- Keep the referee key apart from your knowledge key
  (`~/.openagents/nostr/knowledge-key`). A referee can't award XP to a
  completion whose runner is the entry's author, and separate keys keep your
  roles legible to readers.
- Every command needs `--relay URL`. There's no default relay, because
  these commands publish to other people or read what they published.

## Publish a quest

Write the quest to a JSON file. Every field is required, except
`completions`, which defaults to `first`:

```json
{
  "id": "tb4.fix-git.beat-fable-low",
  "version": 1,
  "season": {"id": "2026-q4", "opens_at": 1790000000, "closes_at": 1798000000},
  "title": "Beat Fable 5.1 low's cheapest winning run on fix-git",
  "objective": "Publish an entry that makes a paired Microcoder run pass fix-git for less than Fable 5.1 low's cheapest winning run.",
  "acceptance": {
    "rule": "kb-transfer",
    "task": "fix-git",
    "min_pass_rate": 1.0,
    "max_usd_per_run": 0.21
  },
  "reference": {"label": "Fable 5.1 low, cheapest winning run", "usd": 0.21, "seconds": 312, "source": null},
  "award": {"author": 6, "runner": 4}
}
```

- `acceptance.task` is the task name as the evidence report records it.
- `acceptance.max_usd_per_run` is the bar the with-entry runs must beat, in
  dollars per run; `null` removes the cost bar.
- `award` is XP per role. The quest's award is the sum, from 1 to 1,000.
  The author and the runner split it; they don't each get the whole.
- `reference` is display and provenance only. Put the reference run's
  record in `source`, such as its result file in the Gym.

Then publish it:

```sh
microcoder xp quest quest.json --relay wss://relay.openagents.com
```

Publishing the same version again is a no-op. Publishing the same `id` and
`version` with different content is refused: to change a quest, raise its
`version` and publish again. The old version and its awards stay as they
were.

## Award a completion

When a runner has published evidence for an entry, award the quest version
by naming it and the evidence event:

```sh
microcoder xp award --relay wss://relay.openagents.com \
  --quest tb4.fix-git.beat-fable-low@1 \
  --evidence <kind-3189 event ID> \
  --label beat-reference
```

The command fetches the evidence and the entry it's about, reads the tasks
the entry was written from, and runs the quest's rule. When the rule
refuses, it prints why and publishes nothing. When the quest version
already has a live award from you, it refuses: a quest version pays once.

Each `--label VALUE` also publishes a NIP-32 achievement label, in the
`openagents.xp` namespace, that points at the award. Readers show a label
only while its award counts, and a label never carries XP.

Before you accept, check that you can trust the evidence. The protocol can
refuse self-evidence, but it can't tell whether two keys belong to one
person. Accept evidence you can re-derive from retained runs, or evidence
from runners you know.

## Revoke an award

```sh
microcoder xp revoke <kind-3193 award ID> --relay wss://relay.openagents.com \
  --reason "The runs were graded against the wrong verifier version."
```

A revocation is permanent, and readers see its reason. Once an award is
revoked, you can award the quest version to a correct completion with
`xp award`.

## Read the ledger

```sh
microcoder xp ledger --relay wss://relay.openagents.com --referee npub1...
```

The ledger counts only the awards of referees you trust. It trusts:

- the referees in `~/.openagents/knowledge/xp-trust.json`;
- the referees named with `--referee`, which can repeat;
- your own referee key, when it exists.

The trust file looks like this:

```json
{
  "referees": ["npub1..."],
  "runners": ["npub1..."]
}
```

When `runners` lists keys, or you pass `--runner`, only evidence from those
runners counts. Use it when you don't want to rely on a referee's judgment
about who ran the evidence.

The ledger re-checks every award against the quest, entry, and evidence it
names, then prints XP per public key, the awards behind it, and anything it
refused:

```text
connected to wss://relay.openagents.com; trusting 1 referee
1 quests; 1 awards counted, 0 revoked, 0 refused, 0 conflicts, 0 from untrusted referees
     6 XP  npub1...
     4 XP  npub1...
- tb4.fix-git.beat-fable-low@1 "Beat Fable 5.1 low's cheapest winning run on fix-git" (2026-q4), award 35dbfac640e7: author npub1... 6, runner npub1... 4
```

`--json` prints the ledger as JSON for another program, such as a display.
The command exits with `1` when an award was refused or a conflict was
found, so a script can notice. A conflict means a referee has two live
awards for one quest version, or published one quest version twice with
different content; neither counts until the referee fixes it.

Levels, titles, and stat points are a client's reading of these totals, not
part of the ledger. The Verse display of quests, awards, and levels is
planned in issue #9685.
