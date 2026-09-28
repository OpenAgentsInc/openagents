# Tutorial quests: reproduce a published pass

> **Status: Published 2026-09-28.** Six `reproduce` quests from the
> OpenAgents referee are open on `wss://relay.openagents.com` in season
> `tb21-tutorial-s1`, which closes 2026-12-25. They are the first quests a
> person outside OpenAgents can complete. No award has been granted yet.
> XP is a record, not money: nothing here pays.

A tutorial quest asks you to rerun one of Microcoder's retained
Terminal-Bench 2.1 passes from its pinned recipe, on your own machine, and
pass the task's tests. The OpenAgents referee checks your run and signs an
award of 50 XP to your key. Your level then shows over your head in the
OpenAgents app's Grid, such as `650a2a22 · lv 2`, and on your trainer card
under **Account > Trainer**.

The rule is NIP-XP's [`reproduce`](../../nips/openagents/NIP-XP.md#reproduce):
the quest pins a **claim** (run evidence of the published pass) and the
digest of its **recipe**; you publish a **reproduction** that cites the
claim; the referee accepts it only when your run followed the recipe,
passed, has a run record of its own, and your key isn't the claimant's. Each
quest pays once, to the first accepted reproduction.

## The quests

Each is worth 50 XP to the reproducer and 0 to the claimant (OpenAgents
doesn't earn XP from its own referee). The claims are signed by the
OpenAgents knowledge key and made from the retained run records named
here.

| Quest | Task | Claimed run | Microcoder's passes |
| --- | --- | --- | --- |
| `tb21.prove-plus-comm.reproduce@1` | `prove-plus-comm` | `prove-plus-comm-1790468849833` | 3 of 3 |
| `tb21.fix-git.reproduce@1` | `fix-git` | `fix-git-1790462479328` | 3 of 3 |
| `tb21.openssl-selfsigned-cert.reproduce@1` | `openssl-selfsigned-cert` | `openssl-selfsigned-cert-1790465139801` | 3 of 3 |
| `tb21.regex-log.reproduce@1` | `regex-log` | `regex-log-1790469395553` | 3 of 3 |
| `tb21.sqlite-db-truncate.reproduce@1` | `sqlite-db-truncate` | `sqlite-db-truncate-1790470547404` | 3 of 3 |
| `tb21.build-pmars.reproduce@1` | `build-pmars` | `build-pmars-1790459651308` | 3 of 3 |

The run records are in
`bench/terminal-bench/microcoder-runs/coderos-4080-tb21/`. Every recipe is
Microcoder with GPT-6 Luna at medium effort, knowledge off, on the task's
`alexgshaw/<task>` image. The quest specs, with each claim's event ID and
recipe digest, are in `knowledge/quests/tb21.*.reproduce@1.json`.

## Before you begin

You need:

- Docker, to run the task's container.
- Microcoder: `cargo install --path crates/microcoder --locked` from a
  checkout of this repository.
- A Codex login (`codex login`), for GPT-6 Luna, and a Jev key
  (`TYPESAFE_API_KEY` or `~/.openagents/jev.json`). A tutorial run costs
  about a cent at list price.
- The Terminal-Bench 2.1 tasks, installed as the
  [TB2.1 dev set](../terminal-bench/tb21-dev-set.md#install) describes.
- Your trainer key. In the OpenAgents app, it's the key over your head in
  the Grid: open **Account > Trainer**, tap **Reveal nsec**, and save the
  nsec in a file with mode `0600` on your computer. `--key` reads an nsec
  or 64 hex characters. Any Nostr key works, but only an award to your
  trainer key shows in the app.

## Complete a quest

1. Run the task with the recipe's settings:

   ```sh
   export MICROCODER_TASKS=~/.openagents/terminal-bench/upstream/terminal-bench-2.1/tasks
   microcoder prove-plus-comm --model gpt-6-luna --effort medium --kb off
   ```

   Microcoder prints `Record: <directory>` at the end. The run must pass
   (reward 1). If it fails, run it again; a failed attempt costs no XP.

2. Publish your reproduction, signed with your trainer key:

   ```sh
   microcoder xp reproduce --relay wss://relay.openagents.com \
     --quest tb21.prove-plus-comm.reproduce@1 \
     --referee npub1v59z5gklyzc4v7c8klhqd8nuffl426d3s7zyluu5suxyyjn4khrsrusf6k \
     --record <the run's directory> --key <your trainer key file>
   ```

   It checks your run against the quest first and publishes nothing when
   the run failed, didn't follow the recipe, or is the claimant's.

3. Open a GitHub issue titled `Reproduction: tb21.prove-plus-comm.reproduce@1`
   with the reproduction's event ID, and attach the run's `summary.json`.
   Read it before you share it: it holds your run's test output and costs.

4. The referee checks the file against the digest your reproduction names
   and runs the rule. When it passes, it publishes the award, and your
   level updates in the app within seconds of the next read.

## What a reader checks

Anyone can recompute your level: fetch the OpenAgents referee's quests and
awards from the relay, then the claims and reproductions the awards name,
and run the rule over them. `microcoder xp ledger --relay
wss://relay.openagents.com --referee npub1v59z5…` and `openagents xp
--pubkey <your npub>` do this; the app does it on the phone. Levels use
`trainer-curve-v1`: level 2 at 100 XP, level 3 at 283, level 4 at 520. The
six tutorial quests together are worth 300 XP, level 3.

The referee's one check a reader can't repeat is the run record file
itself; a reader that wants more than the referee's word can list the
reproducers it trusts. See [agent trainer leveling](agent-trainer-leveling.md)
for the whole design and what comes after the tutorials.
