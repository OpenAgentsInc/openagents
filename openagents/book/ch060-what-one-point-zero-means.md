# 60. What 1.0 means

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

This last chapter builds nothing. It states what has to be true before we call
any of this version 1.0, and it lists what we will still not have shown.

## Draft

A version number is a promise, and this project has made promises it did not
keep on time. [Episode 234](../../docs/transcripts/234.md) admits the gap
between what was said and what was running, and starts a registry that marks
each claim as live, gated or withdrawn. Version 1.0 uses the same discipline.

The test for 1.0 is the list this book began from. Episode 289 makes sixteen
promises, and the audit numbers them P1 to P16. Each is covered by chapters
here. 1.0 is the point at which, for every one of the sixteen, the chapters
that cover it run on the platforms they name, and a person outside the company
can do the thing promised.

Some promises can be checked by a program: the same runtime on every surface,
a payout that matches its receipts. Some need a person: a stranger publishes a
plugin and is paid. The list says which is which.

The general-agent essay ends by separating what was shown from what was not,
and that section stays true until we change it. We have not shown independent
contributors at scale. We have not shown that the network compounds, that
adding people makes the agent measurably better. And we have not shown the
composition working outside software. 1.0 does not need all three. It needs
each to be stated plainly next to the version number, with the measurement
that would settle it.

The essay also says what would show the thesis is wrong. If members that pass
every check do not make the agent better on new work, the method has failed,
and we should say so.

This book gives no dates. The order of work is in the plan, and each step is
done when its chapters run.

## What we build

A plan for what this chapter ships once the draft is approved:

- A 1.0 checklist: each of P1 to P16, the chapters that cover it, the
  platforms, and whether a program or a person checks it.
- A public list of what is live, gated or not yet built, kept next to the
  download page.
- A page of what we have not shown, each item with the measurement that would
  settle it.

It builds on what exists today:

- [`docs/audits/2026-11-10-episode-289-port-audit.md`](../../docs/audits/2026-11-10-episode-289-port-audit.md)
- [`docs/transcripts/README.md`](../../docs/transcripts/README.md)
- [`README.md`](../../README.md)

Planned paths: `openagents/book/ONE-POINT-ZERO.md`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, version 1.0
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): 2:15, three years into one product
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md)
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): an earlier 1.0
  - [Episode 248, *Predictable Software*](../../docs/transcripts/248.md)
  - [Episode 259, *Verifiable Software*](../../docs/transcripts/259.md)
  - [Episode 138, *Year One Recap*](../../docs/transcripts/138.md): a year-one recap
  - [Episode 100, *Looking Ahead*](../../docs/transcripts/100.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), Part V and "What would show this is wrong"
  - [The port audit](../../docs/audits/2026-11-10-episode-289-port-audit.md), sections 1 and 4
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P16

## Open questions for the owner

1. Is 1.0 one release of everything, or does each surface reach 1.0 when its
   own chapters run?
2. Which of the sixteen promises may ship marked "gated" at 1.0, if any?
