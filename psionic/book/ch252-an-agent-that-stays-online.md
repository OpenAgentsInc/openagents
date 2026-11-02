# 252. An agent that stays online

> **Status: unimplemented — design draft.** Nothing here runs yet.

Chapter 251's agent is only useful if it keeps working while a laptop sleeps,
answers while a long build runs, talks the way each channel expects and
recovers from failures without a person watching. This chapter covers the
asynchronous loop, channel rules, the split between compute, state and
execution environments, and a watchdog. It follows the second half of the
DeepSeek Harness team's Super-agent article
([notes](../docs/harness.md#one-agent-across-many-tasks)).

## What this chapter covers

- **Full duplex.** Long tools become `start` and `stop`. `start` returns a
  `started` notice with a job ID at once, and the loop goes on; the result
  arrives later as a new event in the log (chapter 208's background jobs). The
  model can `stop` a job that runs too long. New input is handled when the
  current inference step ends, not when the tools finish.
- **Channel rules.** The Router writes every reply, so verbose reasoning and
  tool logs stay inside Workers. Each channel has written rules:
  - Chat: short, interrupt only for a decision or a sign-off, ask direct
    questions, give conclusions. A hard cap of 100 words per reply.
  - Issues, pull requests, mail and documents: structured and complete, such
    as background, change and verification for a pull request, following the
    project's conventions.
- **Compute apart from state.** The loop runs in stateless containers that can
  restart, scale out or move. Sessions, Threads, subscriptions and jobs live
  in a durable store as chapter 201's log, and a replacement container
  resumes from the log.
- **Compute apart from execution environments.** Repositories with
  uncommitted work, local credentials, intranet services and specific
  hardware stay where they are. Each machine runs a small daemon that holds no
  session state; it runs commands and returns results under chapter 204's
  sandbox:
  - The person's own machine, a cloud machine or a sandbox made on demand all
    attach through the same interface.
  - A task that needs an offline machine waits, and starts when that machine
    reconnects.
- **A watchdog.** A separate session, outside the main system and woken on a
  schedule:
  - Handles infrastructure faults: API errors, failed state recovery, stuck
    jobs. It rechecks transient external failures until they clear.
  - Judges with a model whether each Worker is making real progress or has
    drifted from its goal (chapter 208's goal check), and stops or redirects
    it.
  - Answers maintainers' questions about failures and can change the main
    system's prompts and trigger rules while it runs. Every change it makes is
    a logged, revertible event.
- **Trust boundaries.** The loop takes in untrusted events, and the machines it
  reaches hold private code and credentials:
  - A daemon admits only commands within its machine's grant, so a
    compromised loop cannot take every attached machine.
  - Credentials are used through capability handles the daemon resolves. The
    secret never enters model context or the log.
  - No session content leaves the person's machines without chapter 213's
    opt-in.
- **Open problems kept open.** When to shard the Router and how finely, when
  Workers should talk to each other directly, memory over months (chapter
  214), how tasks appear to a person once a conversation is no longer one
  session, and agent-to-agent work between instances that share no context
  and hold different permissions. The chapter records these as unanswered
  rather than guessing.

## Dependencies

- [Chapter 201](ch201-model-visible-means-logged.md): the durable log the stateless loop resumes from.
- [Chapter 204](ch204-a-sandbox-and-approvals.md): grants and confinement on each daemon.
- [Chapter 205](ch205-a-document-tries-to-become-the-system-prompt.md): hostile events and tool results.
- [Chapter 208](ch208-forks-subagents-and-jobs.md): background jobs and goals.
- [Chapter 213](ch213-consent-before-sharing.md) and [Chapter 214](ch214-remember-a-fact-then-revise-it.md): consent and long-term memory.
- [Chapter 251](ch251-one-agent-many-tasks.md): the Router and Workers this chapter keeps online.

## Done means

- **Duplex.** With a ten-minute job running, a new message gets a reply within
  one inference step, and the job's result arrives later as an event. A
  stopped job reports as stopped, and an expired one as expired.
- **Channels.** Every chat reply in a replayed stream is 100 words or fewer,
  and every pull request body has its required sections.
- **Restart.** Killing the loop's container mid-task and starting a new one
  resumes every open task from the log with no lost or repeated tool effects.
- **Intermittent machines.** A task bound to a machine that goes offline waits,
  then completes when the machine reconnects.
- **Watchdog.** In fault-injection runs (API errors, a Worker looping on one
  failing call, a Worker drifting off its goal) the watchdog detects each
  fault and the time to detection is reported.
- **Containment.** In chapter 205's lab, extended to Triggers, a hostile event
  produces no admitted command outside a daemon's grant and no secret in
  model context, the log or outbound messages.
- Each mechanism passes chapter 211 against the simpler design on tasks it was
  not built from, or it is removed.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
- DeepSeek Harness team, "Exploring Super-agent Design: Towards a More
  Autonomous Agent System" (Yujia Qiao et al.).
- Anthropic, "Decoupling the brain from the hands", on a stateless harness
  with external session logs.
