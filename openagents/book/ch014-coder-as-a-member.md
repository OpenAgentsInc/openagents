# 14. Coder as a member

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The agent can talk and answer. This chapter gives it its first member that
does work: when you ask about your repositories, the front hands the request
to Coder.

## Draft

[Episode 289](../../docs/transcripts/289.md): ask it to interact with your
repos, and it loads the Coder agent. This is promise P3, and Coder is the
first specialist member.

The split is the one the Coder book's plan sets out. OpenAgents is the general
agent. It speaks as "we", owns identity, the wallet, knowledge and routing,
and writes every reply in shared channels. Coder takes one repository task
from start to finish: it reads the code, runs commands, edits, tests and opens
the pull request. Coder speaks as "I" and has opinions about code.

Coding is the first member for a reason. It is the job this project has
returned to since Faerie read its first issue in [Episode
20](../../docs/transcripts/020.md). Code is also a general way to act on
digital information, and coding agents check their own work with tests. That
makes Coder the strongest executor we have, and the general agent's answer to
the first pressure, execution.

The interface is small. A task goes in with a grant for one repository. A
result comes out: a change, the checks that ran, and whether they passed. A
red result pushes nothing and says why.

This book does not rebuild Coder. The Coder book does that, in 38 chapters.
This chapter builds the adapter that makes Coder a member, and the route that
sends it work.

## What we build

A plan for what this chapter ships once the draft is approved:

- A member adapter over Coder: task in, result and checks out, under a
  repository grant.
- The issue flow as a member call: its own worktree, the repository's checks,
  and a push only when they pass.
- Labeled rows that separate "work on my repository" from a question about
  code.

It builds on what exists today:

- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/lib/microcoder`](../../coax/lib/microcoder)
- [`coax/lib/agent`](../../coax/lib/agent)
- [`coder/book/README.md`](../../coder/book/README.md)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A task and its result | `openagents.exec.v1` `ExecRequest`, `ExecResult` ([spec 17.2](../../docs/protocol/openagents-protocol-spec.md#172-openagentsexecv1)) | Nostr kinds 25920 and 26920, encrypted (NIP-CJ); HTTP `POST /v1/jobs` | Spec sketch; no `.proto` file |
| Coder's session log and stream records | Planned `openagents.session.log.v1alpha1`; `coder.observability`, `coder.reply-stream`, `coder.route` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C1 to C3) | Files and a line stream | Hand-written JSON today. The compliance plan moves each to one codec |

Planned paths: `openagents/agent/members/coder.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, loading Coder
  - [Episode 20, *Planning a GitHub Agent*](../../docs/transcripts/020.md): planning the first GitHub agent
  - [Episode 30, *Faerie Debugs Failing Tests*](../../docs/transcripts/030.md): the agent debugs failing tests
  - [Episode 123, *GitHub Issues to Pull Requests*](../../docs/transcripts/123.md): issues to pull requests
  - [Episode 161, *Basic Coding Loop*](../../docs/transcripts/161.md): a basic coding loop
  - [Episode 275, *Coder*](../../docs/transcripts/275.md)
  - [Episode 277, *Coder Terminal*](../../docs/transcripts/277.md): the Coder terminal
- **Old code at `coder-v1.0.0`, crates:** `coder`, `coder-one`, `microcoder`, `microcoder-loop`, `briefed-agent`, `supervise`, `coder-project`
- **NIPs:** [NIP-WORK](../../nostr/nips/openagents/NIP-WORK.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md)
- **Also:**
  - `programs/solve-issue.json` at `coder-v1.0.0`
  - [Coder book, plan: OpenAgents and Coder](../../coder/book/PLAN.md#openagents-and-coder)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3
