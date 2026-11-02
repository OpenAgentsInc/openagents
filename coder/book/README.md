# Coder: Coding Agents From Scratch

Building OpenAgents' coding agent in [Coax](../../coax/book/README.md), and
learning from every coding agent we can read.

Coder takes one repository task from start to finish: it reads the code, runs
commands, edits, tests and opens the pull request. It is one kind of Worker
inside OpenAgents, the general agent that people reach from the web, the
terminal, the desktop and mobile apps ([Episode 289](../../docs/transcripts/289.md)).
OpenAgents speaks as "we", owns identity, the wallet, the knowledge base,
plugins and routing, and writes every reply in shared channels. Coder speaks
as "I" and is opinionated about code. The [plan](PLAN.md#openagents-and-coder)
sets out the split.

The book builds the harness one measured piece at a time: a simple loop, a log
that rebuilds every request, typed tools, a sandbox, condensing, programs as
tool calls (Part I). It then adds engines: Coder's own Microcoder loop, models
that [Psionic](../../psionic/book/README.md) serves, and the Claude Code or
Codex a person already uses (Part II). Parts III and IV turn one session into
one agent across many tasks and channels that stays online, following the
DeepSeek Harness team's Super-agent design. Part V measures each addition
against the simplest loop, and Part VI collects everything into the
`openagents coder` command line. Part VII reads other coding agents' source,
one agent per chapter, and records what Coder takes from each.

Every chapter ends at an `openagents coder` verb that does what the chapter
built. The [plan](PLAN.md#the-cli-is-the-spine) lists them.

## Status

Written 2026-11-01. Of the 18 chapters that live here, 6 are design drafts
and 12 are planned. None is implemented yet. Chapters 1–8 and 14–22 are moving
from Part XXI of the Psionic book, where eight of them are partly implemented;
until the move they live there, and this book's
[registry](chapters.json) lists only the chapters in this directory.

| Status | Meaning |
| --- | --- |
| implemented | The chapter's examples run and its done criteria pass on a named host. |
| partly implemented | Some examples run; the chapter says what remains. |
| design draft | The chapter says what will be built and how it will be checked. Code it names may already exist. |
| planned | A title and a scope only. |

## Contents

See [SUMMARY.md](SUMMARY.md) for the chapter list and [PLAN.md](PLAN.md) for
what each chapter builds and checks.
