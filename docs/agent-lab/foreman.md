# Foreman

Foreman is an agent that keeps the whole picture of OpenAgents work: every
open issue in `OpenAgentsInc/openagents` and every agent working on it. Ask it
what nobody is doing, who is on an issue, which agents are colliding, what is
stuck, or where the boards disagree with reality. It reads by default and
changes things (board status, a message to a chat, a new agent chat) only
when you ask. Tracked in [#11926](https://github.com/OpenAgentsInc/openagents/issues/11926).

## Get Foreman

On the phone or at [openagents.com/labs/agent](https://openagents.com/labs/agent):

1. Open **Agent Lab** (on the phone, from the agent picker).
2. Choose **Start from Foreman**.
3. Choose the computer it runs on and a project, then **Save**.
4. Choose **Use agent**. Foreman now appears in the agent picker. Pick it and ask.

To do the same from code, run the [SDK example](../../crates/openagents-sdk/examples/foreman.rs).
It saves the experiment, runs its tests on a computer, prints the report, and with
`--promote` saves the agent:

```sh
mkdir -m 700 /tmp/foreman-requests
OPENAGENTS_TOKEN=… OPENAGENTS_LAB_REQUEST_DIR=/tmp/foreman-requests \
  cargo run -p openagents-sdk --example foreman -- --promote
```

The computer needs the updated desktop app, Claude Code signed in, and the GitHub
CLI (`gh`) signed in with access to the repository and its projects. **Run tests**
copies the chosen project into a trial folder, so tests need a small project; a
full `openagents` checkout is over the copy limit
([#11929](https://github.com/OpenAgentsInc/openagents/issues/11929)).
**Use agent** doesn't run tests and works with any project.

## What it is

The template is [`agent_lab::templates::foreman`](../../crates/agent-lab/src/templates.rs):
Claude Code on Opus 5.5 with the host's own access, and the instructions in
[`foreman.md`](../../crates/agent-lab/src/templates/foreman.md). It reads two sources:

| Source | What Foreman reads |
| --- | --- |
| GitHub, through the computer's `gh` | Open and recently closed issues, claim markers, native blockers, project 19 and 22 status, and recent commits that name issues |
| The app's agent tools | Every chat on every device, with harness, model, live status, last activity, and its harness session id |

The harness session id is the join between the two
([#11927](https://github.com/OpenAgentsInc/openagents/issues/11927)): a claim
marker `session=claude-code:ID` belongs to the chat whose `harnessSessionId` is
`ID`. A claim that matches no chat comes from an agent outside the app, such as a
terminal session, and Foreman says so.

## Its tests

| Test | Check |
| --- | --- |
| Reaches GitHub issues | Finds #11840 by its title |
| Reaches the app's agent tools | Names its own harness, `claude-code` |
| Reads claim markers | Reads the session in #11859's claim marker |
| Holistic status | Reviewed: evidence-backed, split into can-start, waiting and parked |
| Finds collisions and drift | Reviewed: names both sides and the contrary evidence |

The first three pass or fail on their own; the last two wait for your review in
the run report. A passing run shows that Foreman reached its sources on that
computer. It is not a benchmark of its judgment.
