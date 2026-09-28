# Cheapest verified passes, and a network that gets smarter

An essay, written September 27, 2026. Every number here is copied from a
retained record and linked to it. The
[showcase](beat-fable-showcase.md) has the full tables and labels.

## Two claims, and only two

"World's best coding agent" is a claim every lab makes and no reader can
check. OpenAgents makes two narrower claims instead, and each one comes with
the records you need to check it yourself:

1. **Cheapest verified passes.** When Coder passes a task, it passes for a
   small fraction of what a frontier agent spends, and a grader, not the
   model, decides what counts as a pass.
2. **A network that gets smarter.** Coder improves when people teach it,
   not only when a lab ships a new model. What one person teaches, every
   agent on the network can use, and the teacher gets the credit.

The first claim has evidence today. The second has a mechanism and early
signs, but no proof yet. The rest of this essay says exactly where each one
stands.

## Claim one: cheapest verified passes

On Terminal-Bench 2.1, Microcoder ran 65 tasks it was never tuned on, using
GPT-6 Luna with the knowledge base turned off. The task list and its digest
were committed before the first run
([pre-registration](../terminal-bench/2026-09-26-tb21-oos-study.md),
[results](../terminal-bench/2026-09-26-tb21-oos-results.md)):

- **30 confirmed wins.** On 30 of 65 tasks, at least 2 of 3 runs passed
  for less than Fable 5 xhigh's cost per trial on the same task.
- **The median pass cost 2.9% of Fable's cost per trial:** $0.0075 at list
  price, in 1 minute 49 seconds.
- **The whole round of 127 graded runs, failures included, cost at most
  $4.45.**

That's the headline. Here is the part a headline usually leaves out:

- **Reliability is the gap.** Microcoder's first run passed 31 of the 65
  tasks (48%). Fable 5 xhigh passed 92% of its trials. Microcoder is far
  cheaper where it succeeds. It isn't a drop-in replacement.
- **TB2.1 is the older, easier benchmark.** On held-out Terminal-Bench 4
  tasks, Microcoder hasn't passed yet: 0 of 24.
- **"Cost" is list price** applied to reported tokens on a subscription
  login, not a bill.

Why lead with cost instead of pass rate? A pass that costs under a cent can
be retried, checked, and combined with other passes. A $1 attempt can't be
retried as freely. Once a cheap agent passes, reliability becomes a
question of how many attempts you can afford and how well you can verify
them. That's an engineering problem, not a question of how big the model
is.

"Verified" carries as much weight as "cheapest". A model saying it
finished counts for nothing. A pass is what the benchmark's grader
accepts, and every run leaves a record with its steps, cost, time, and
outcome. Every claim prints its own labels ("in-sample",
"knowledge-assisted", "list price") in the claim text itself, so a
screenshot can't separate the number from its caveats.

## Claim two: a network that gets smarter

On Terminal-Bench 4, the same cheap loop failed tasks until it was given
one precise, cited fact from the shared knowledge base. With that fact, it
passed three TB4 tasks for between 1/45 and 1/2 of what Fable 5.1 low's
cheapest winning run cost
([showcase](beat-fable-showcase.md#the-tb4-result-in-sample)):

| Task | Before the fact | With the fact |
| --- | --- | --- |
| `gsea-proteomics` | 0 of 10 | 4 of 4, at $0.05–0.07 against Fable's cheapest $0.69 |
| `fin-saccr-rwa` | 0 of 6 | 4 of 4, at $0.04–0.08 against Fable's cheapest $1.23 |
| `embedding-drift-monitor` | — | 8 of 9, every pass under Fable's cheapest $0.74 |

The facts were delivered over a Nostr relay as signed entries
([NIP-KB](../../nips/openagents/NIP-KB.md)). The agent didn't need them
baked into its weights, didn't need a new model, and didn't need to trust
the author's word, because each entry cites its source.

The honest label is **in-sample**. Each deciding entry was written from the
task it helped. That shows the mechanism works: a cheap model plus the
right fact beats an expensive model without it. It doesn't show the
knowledge **transfers** to work its author never saw. When a delegate was
briefed from the same knowledge base on 14 more tasks, it beat Fable's bar
on only 4 of 28 attempts, and each of those used knowledge written from
the same task ([reproduction](../terminal-bench/2026-09-27-fable-delegate-repro.md)).
It doesn't reproduce reliably yet.

The knowledge base on `relay.openagents.com` holds 154 entries, and one
key wrote all of them. One operator writing lessons on the tasks they
measure is fitting to those tasks. Many people writing lessons, and
measuring each other's lessons on tasks the author never saw, is a network.
Only the second can prove claim two, and we can't build it alone.

## Why this matters to developers

A frontier agent's knowledge is frozen at training time and lives inside
someone else's model. Every hard-won lesson about your build system, your
flaky test, or your cloud provider's error message disappears when the
session ends.

A knowledge entry is different. It's small, cited, signed by a key, and
public. It can be retrieved by any agent on any model. When it helps, the
run record says which entry helped. When it's wrong, the regression checks
show it and the entry can be retired. The unit of improvement becomes
something you can write in five minutes and verify yourself.

## Why this matters to gamers

Games already know how to make hard, verifiable work fun: quests with clear
objectives, leaderboards you can't fake, and replays that show exactly what
happened.

The [quest board](../terminal-bench/quest-board.md) posts real benchmark
tasks as quests. XP comes only from a verified, accepted outcome
([NIP-XP](../../nips/openagents/NIP-XP.md)). Each quest version pays once,
on its first accepted completion. The award splits between contributors,
such as the author of a deciding entry and the runner who measured it. It
never multiplies with replays, so grinding earns nothing. Your client
re-checks every award against referees it trusts, so there's no central
score to hack.

In [Verse](../verse/README.md), the 3D world where Coder lives, a replay
shows your agent's run as a walk through the world: the workbench for
model steps, the oracle for questions, the library for knowledge lookups,
and the proving ground for tests. It races beside a ghost of Fable's
cheapest winning run on the same task. It's a speedrun against the
frontier, where the timer and the price tag are both real.

## Why this matters to bitcoiners

If knowledge is worth something, the person who wrote it should be able to
get paid for it, without an account, a platform cut, or anyone's
permission. That calls for a native internet currency with small payments
and identities that are only keys.

The pieces are Nostr keys for identity, signed events for knowledge and
XP, and Lightning for payment. The `openagents wallet` Lightning node and
pay-per-call x402 flows over HTTP, MCP, and the relay itself landed in
code this week ([NIP-X402](../../nips/openagents/NIP-X402.md)). The same
keys are meant to carry compute, knowledge, and labor markets
([market infrastructure](../agents/market-infrastructure.md)).

Here is exactly where that stands: no paid round trip has been recorded
yet, and nothing in the repository pays an author today. XP is never
money and never converts into sats. When sats move, they'll move for real
work a buyer accepted, and every payment will have a receipt you can open.

## What would prove claim two

A pre-registered test on held-out tasks, with the knowledge base frozen
before the first run. It compares the same model with knowledge on and
knowledge off, and uses only entries written by keys other than ours, on
tasks those authors never saw. If shared knowledge raises the pass rate on
that test, the network effect is real and measured. If it doesn't, we
publish that too.

That test needs authors. The fastest way to make a cheap coding agent
better than an expensive one is for more people to teach it, check each
other's lessons, and get the credit when those lessons work. Cheapest
verified passes are what we can show today. A network that gets smarter is
what we're asking you to help prove.
