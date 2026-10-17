# Foreman

Foreman is an agent that keeps the whole picture of OpenAgents work: every
open issue in `OpenAgentsInc/openagents` and every agent working on it. Ask it
what nobody is doing, who is on an issue, which agents are colliding, what is
stuck, or where the boards disagree with reality. It reads by default and
changes things (board status, a message to a chat, a new agent chat) only
when you ask, or on its own check-ins once you give it standing authority. Tracked in [#11926](https://github.com/OpenAgentsInc/openagents/issues/11926).

## Get Foreman

On the phone or at [openagents.com/labs/agent](https://openagents.com/labs/agent):

1. Open **Agent Lab** (on the phone, from the agent picker).
2. Choose **Start from Foreman**.
3. Choose the computer it runs on and a project, then **Save**.
4. Choose **Use agent**. Foreman now appears in the agent picker. Pick it and ask.

## Ask Foreman from the terminal

```sh
openagents agents ask Foreman "what isn't being worked on?"
```

Foreman runs on a computer of yours that's online in the OpenAgents app with
Claude Code. Inside a Git checkout, it prefers the project with the same name
as the repository; `--on COMPUTER` and `--project PROJECT` choose explicitly.
The command waits up to ten minutes (`--timeout SECONDS`) and prints Foreman's
answer. Its conversation stays in the app, and `openagents agents status`
lists recent questions. `openagents agents list` shows your saved agents.

Chats can ask too. In a website, phone, or project chat
(`openagents chat send "Ask Foreman …" --project NAME`), the chat hands the
question to Foreman the same way and replies with its answer
([#11934](https://github.com/OpenAgentsInc/openagents/issues/11934)).

To make Foreman from code instead of the app, run the [SDK example](../../crates/openagents-sdk/examples/foreman.rs).
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

## Let Foreman run on its own

In the desktop app, open **Settings → General → Foreman** on the computer that runs Foreman's chat ([#11965](https://github.com/OpenAgentsInc/openagents/issues/11965)):

1. **Foreman's chat**: pick the chat you started with **Use agent**.
2. **Check-ins**: choose how often (every 10 minutes to 4 hours) and turn them on. The schedule is saved to your account and also appears at [openagents.com/settings/schedules](https://openagents.com/settings/schedules), where you can pause or delete it. The computer runs it, not a server.
3. **Standing authority**: turn it on to let Foreman message, start, steer and stop chats on your computers without asking during check-ins. Turn it off at any time. Money, keys, secrets, deploys and closing other agents' issues still need your approval. Turns you start yourself work as before.

Foreman also checks in, at most once every few minutes and never over a turn it's already running, when:

- a chat it started, messaged or stopped finishes, errors or waits for an answer;
- an issue in `OpenAgentsInc/openagents` opens or closes;
- a claim has had no new comment for six hours.

The computer's `gh` reports the issue and claim events. Each check-in asks Foreman to call `record_decision` with what it saw, decided and did. The **Decision log** in the same card lists check-ins, those records, how each check-in ended, and authority changes. The log is a file in the app's data folder (`foreman/decisions.jsonl`), kept across restarts, and the next check-in includes the latest entries.

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
