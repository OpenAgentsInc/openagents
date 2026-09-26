# Contribute knowledge and take a quest

Microcoder, a cheap GPT-6 Luna loop, beats Fable 5.1 low's cheapest winning
run on three Terminal-Bench 4 tasks (in-sample and knowledge-assisted; see
the [results](../../terminal-bench/tb4-results.md#microcoder-development-runs-in-sample)).
Each time, the difference was one precise, cited fact from the shared
knowledge base, written from that same task. On the other 11 tasks it has
tried, it fails, and the base holds nothing about their domains. The
base is only as wide as the people who write it.

This page is for anyone who wants to widen it: write an entry, publish it
under your own key, and let other people's runs decide whether it counts.
The [plan](../beat-fable-together.md) explains why, and the
[quest board](../../terminal-bench/quest-board.md) lists the open quests.

There are two roles, and one person can't play both for the same entry:

- An **author** writes and publishes an entry. You need this repository and
  a Nostr key; you don't need to run anything.
- A **runner** runs paired Microcoder runs, with and without someone
  else's entry, and publishes the evidence. You need Docker, the
  Terminal-Bench 4 tasks, a Codex login, and a Jev key.

## Which tasks are open

**While the [out-of-sample study](../../terminal-bench/2026-09-26-out-of-sample-study.md)
runs, quests name only the 14 tasks Microcoder has already run.** The study
measures Microcoder on 49 tasks that nobody has written knowledge about;
entries about those tasks would spoil it. More tasks open when the study
closes.

Until then:

- Take quests only from the [quest board](../../terminal-bench/quest-board.md).
- Don't read anything about a task outside the 14, including its
  instruction, its files, Microcoder runs on it, and Fable's public
  trajectories on it, to write an entry. The study's pre-registration lists
  those tasks.
- Write entries from the 14 tasks, from Terminal-Bench 2.1, or from general
  references such as papers, standards, and manuals.

## Set up Microcoder

Runners need this; authors need it only for `kb` commands.

1. Build and install it from this repository:

   ```sh
   cargo install --path crates/microcoder --locked
   ```

2. Install the pinned Terminal-Bench 4 tasks with
   `uv run tbench tasks checkout --catalog tb4` from `bench/terminal-bench`
   (see the [runbook](../../terminal-bench/runbook.md)). They land under
   `~/.openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks/`.
3. Sign in to Codex with `codex login`, and put a Jev key in
   `TYPESAFE_API_KEY` or `~/.openagents/jev.json`.
4. Check that a task starts and grades, at no model cost. This runs the
   task's reference solution for you; it doesn't show it:

   ```sh
   microcoder shadow-relay --check-grading
   ```

The [Microcoder guide](microcoder.md) explains the loop and its options.

## Find the missing detail

An entry is one general, checkable fact a cheaper agent would miss: a
formula, a convention, an edge case, or an environment trap. The winning
entries so far were the unbiased MMD estimator, the log2 scale for omics
tests, and the SA-CCR commodity hedging sets.

**A quest can't be completed by an entry written from its own task.** The
rule reads the entry's `provenance.written_from`, and a task listed there
never counts as evidence for that entry. So the useful move is transfer:
learn a detail from one source, and show that it helps on a different task.

Two ways to find one:

- **Contrast a failure with a win.** Pick one of the 14 tasks, take a
  failed Microcoder run on it and a Fable 5.1 winning trajectory on it, and
  ask for the decisions that explain the failure:

  ```sh
  microcoder kb harvest-contrast <run> <trajectory.json> --task <task>
  ```

  Fable's public trajectories are listed in
  `bench/terminal-bench/reference/fable-5.1-replays.json`;
  `uv run python -m tbench.public_replays` from `bench/terminal-bench`
  downloads them to `~/.openagents/terminal-bench/public-replays/`. The
  command writes `<task>` into each entry's `written_from`, so the entry can
  complete quests on every task except that one.
- **Write it by hand** from a reference:

  ```sh
  microcoder kb add numerics.kahan-summation --kind method \
    --title "Compensated (Kahan) summation" --author "Your Name"
  ```

  Cite your sources under `provenance.cites`. List every task and run you
  read to write it under `provenance.written_from`, or `reference` if you
  read none.

Then check it:

```sh
microcoder kb lint
```

The lint refuses an entry that cites nothing, still has `TODO` text, names
a benchmark task, or shares 40 or more characters with a task's tests. Read
every harvested entry before you publish it; the
[knowledge-base guide](knowledge-base.md) covers every `kb` command.

## Publish under your key

```sh
microcoder kb publish --relay wss://relay.openagents.com <entry-id>
```

The first `kb publish` creates your key at
`~/.openagents/nostr/knowledge-key` and prints its `npub`. Share the `npub`
so runners can trust your entries. The relay needs no sign-up or
authentication. A published version is immutable: to change an entry, raise
its `version` and publish again.

## How measurement decides

Nothing counts because an author says so. An entry counts when someone
else's runs show it helped:

1. A runner syncs your entries and trusts your key:

   ```sh
   microcoder kb sync --relay wss://relay.openagents.com --author <npub>
   ```

2. The runner runs the quest's task with the base on and off, the same
   number of times, with the same model and caps:

   ```sh
   microcoder <task> --kb candidates --kb-trust all --max-usd <bar>
   microcoder <task> --kb off --max-usd <bar>
   ```

3. The runner measures the entry and publishes a NIP-EVAL report signed
   with their own key: the runs that showed the entry against the runs that
   didn't, on tasks the entry wasn't written from.

**Known gap:** `kb evidence` and `kb publish-evidence` measure and cite
only entries in your own local directory, signed by your own key, so a
runner can't yet publish evidence about another author's entry. Issue
[#9687](https://github.com/OpenAgentsInc/openagents/issues/9687) tracks
the fix. Until it lands, no quest can be completed.

## How XP is awarded

Quests and XP follow [NIP-XP](../../../nips/openagents/NIP-XP.md), and the
[XP guide](xp.md) covers the referee's commands. The OpenAgents referee
awards a quest's `kb-transfer` completion when all of these hold:

- The runner isn't the author.
- The quest's task isn't in the entry's `written_from`.
- The report's verdict is `pass`, and it pairs runs on the quest's task.
- On that task, the runs with the entry pass at the quest's rate (0.66,
  so 2 of 3) and cost less per run than the quest's bar, which is below
  Fable 5.1 low's cheapest winning run.
- The evidence was published inside the season (`tb4-s1`, 2026-09-26 to
  2026-12-25 UTC).

Each quest version pays once, to its first accepted completion: 6 XP to the
author and 4 XP to the runner. More runs, more evidence, or republishing
never add to it. XP is a record of accepted work; it can't be spent,
transferred, or converted into sats, credits, or permissions. Any reader
can recheck an award from the signed events with
`microcoder xp ledger --relay wss://relay.openagents.com --referee <npub>`.

## What never counts

- **Reading a task's reference solution or verifier** to write an entry.
  Harvesting from public winning trajectories is allowed and recorded in
  `written_from`.
- **Task-specific wording.** Entries are general facts with citations, not
  answers to one task: no task names, file paths, expected values, or
  quotes from its tests. Nothing enters Microcoder's instructions per task.
- **In-sample evidence.** Runs on a task the entry was written from.
- **Self-evidence.** Runs by the entry's author, or by a second key the
  author controls. The protocol can't tell two keys apart, so the referee
  accepts evidence it can rederive from retained runs or from runners it
  knows.
- **Cherry-picked runs.** Every graded run of both arms on the task counts.
- **Entries about the study's tasks**, until the study closes.

## Withdraw an entry

If an entry is wrong, withdraw it and publish the withdrawal:

```sh
microcoder kb withdraw <entry-id> --reason "the formula has the wrong sign"
microcoder kb publish --relay wss://relay.openagents.com
```

Runs never show a withdrawn entry again, and readers see the reason. A
withdrawal doesn't revoke XP already awarded; only the referee can revoke
an award, with a signed reason. Removal from a relay isn't erasure: signed
events may remain on other relays.
