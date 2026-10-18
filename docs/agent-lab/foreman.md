# Foreman

Foreman is an agent that keeps the whole picture of OpenAgents work: every
open issue in `OpenAgentsInc/openagents` and every agent working on it. Ask it
what nobody is doing, who is on an issue, which agents are colliding, what is
stuck, or where the boards disagree with reality. It reads by default and
changes things (board status, a message to a chat, a new agent chat) only
when you ask, or on its own check-ins once you give it standing authority. Tracked in [#11926](https://github.com/OpenAgentsInc/openagents/issues/11926).

For the review of Foreman's first two days, lessons learned, and the roadmap, see [`docs/foreman/`](../foreman/README.md).

## Get Foreman

On the phone or at [openagents.com/labs/agent](https://openagents.com/labs/agent):

1. Open **Agent Lab** (on the phone, from the agent picker).
2. Choose **Fork Foreman**. It saves a copy of Foreman as your own agent and opens it. (If the app can't reach the published Foreman, the button reads **Start from Foreman** and starts from the copy in the app.)
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
It starts from the published Foreman, saves the experiment, runs its tests on a
computer, prints the report, and with `--promote` saves the agent:

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

**On by default** ([#12030](https://github.com/OpenAgentsInc/openagents/issues/12030)): Foreman's definition asks for check-ins every 10 minutes and standing authority. When a computer has exactly one chat of a saved agent whose definition asks for this, and no chat is chosen yet, it picks that chat, turns both on, logs the grant, and says so in that chat. The agent's name doesn't matter, only its definition: a fork that keeps the setting gets the same, and an agent named Foreman without it doesn't. Agent Lab trial chats run a draft and never count, and a trial chat the computer picked by itself loses check-ins and authority at its next pass ([#12109](https://github.com/OpenAgentsInc/openagents/issues/12109)). With two or more such chats, the computer picks none and the card asks you to choose. If check-ins are off for an agent that asks for them, every answer it gives starts with "Check-ins are off on this computer, so I only wake when you ask."

**Any agent, not just Foreman** ([#12044](https://github.com/OpenAgentsInc/openagents/issues/12044)): check-ins, the stuck-turn watchdog, the stall ladder, grants and the decision log work for every saved agent, keyed by its definition ID. A fork of Foreman under another name gets the same, and its definition's `autonomy.wakes` chooses what it wakes on. Grants belong to your account (`GET`/`POST /v1/agent-definitions/{id}/grants`, `DELETE …/grants/{grant}`, or the SDK's `agents.grant`), so a fork asks you for its own; money, keys, secrets, deploys and closing other agents' issues can't be granted to any agent.

In the desktop app, open **Settings → General → Check-ins** on the computer that runs the agent's chat ([#11965](https://github.com/OpenAgentsInc/openagents/issues/11965)) to change any of this. Each agent with check-ins has its own rows:

1. **Chat**: pick the chat you started with **Use agent**. **Another agent** sets up check-ins for a saved agent that has none yet.
2. **Check-ins**: choose how often (every 10 minutes by default, up to 4 hours) and turn them on. Every check-in posts a short status in Foreman's chat, even when nothing changed. The schedule is saved to your account and also appears at [openagents.com/settings/schedules](https://openagents.com/settings/schedules), where you can pause or delete it. The computer runs it, not a server.
3. **Standing authority**: turn it on to grant the agent, through your account, every chat power (message, start, steer and stop chats on your computers) without asking during check-ins. Turn it off at any time. Money, keys, secrets, deploys and closing other agents' issues still need your approval. Turns you start yourself work as before.

Foreman also checks in, at most once every few minutes and never over a turn it's already running, when:

- a chat it started, messaged or stopped finishes, errors or waits for an answer;
- an issue in `OpenAgentsInc/openagents` opens or closes;
- a claim has had no new comment for six hours.

**If its own turn gets stuck** ([#12029](https://github.com/OpenAgentsInc/openagents/issues/12029)): when Foreman's chat has been working for 10 minutes with no output (no text, thinking, or tool call or result), the computer stops that turn once and starts a check-in that names the call it was stuck in and for how long, for example "Your last turn was stuck in `Bash` (`curl …`) for 12 minutes, so the host stopped it." A turn that keeps printing, such as a long wait with progress lines, is left alone. The Decision log shows the stopped turn as stuck. Separately, Foreman's definition caps every shell command at 2 minutes (`autonomy.shell_timeout_secs`), and any saved agent can set the same cap.

**Stalls get an action, not an offer** ([#12032](https://github.com/OpenAgentsInc/openagents/issues/12032)). The computer raises, once per stall, a chat Foreman manages that has been working with no output for 20 minutes, and a claimed issue with no commit naming it, no push to its linked branch, no comment, no output from the chat that holds it, and no build running in a worktree named for it, for 20 minutes. Every check-in lists open stalls with the rung that is due, and reaching the next rung starts a check-in of its own: message it at 20 minutes, interrupt it and restart it with a brief at 30, and tell you, first in its status, at 40. Foreman records each rung in the Decision log. Any progress ends the stall. The window is the definition's `autonomy.stall_minutes` (a fork may want 10).

**Bind an existing chat** ([#12057](https://github.com/OpenAgentsInc/openagents/issues/12057)): a chat you started before saving the agent can run it from its next turn, by the agent's definition ID, never its name:

```sh
openagents agents bind AGENT_ID --chat CHAT_ID --on COMPUTER --project PROJECT
```

It creates an assignment that names the chat (the SDK's `assignment_request_for_chat`). The computer adopts the chat only when it's a plain chat in that project on that computer, never another agent's chat, and says so in the chat. From then on every turn runs the agent's current revision, and the computer acts on what the definition declares: check-ins, standing authority, the decision log.

**Publish a new revision only if it passes** ([#12057](https://github.com/OpenAgentsInc/openagents/issues/12057)):

```sh
openagents agents revise AGENT_ID --template foreman --on COMPUTER --project PROJECT --promote
```

It drafts the revision from the template (keeping the agent's name) or, without `--template`, from the agent's current settings, with `--instructions FILE` if given. It runs the [2026-10-17 replay suite](README.md) (#12034) on that computer, prints each case, and promotes only when every case passed. Anything failed, unfinished or waiting for review leaves the agent where it was.

**Working state across restarts** ([#12057](https://github.com/OpenAgentsInc/openagents/issues/12057)): `record_decision` takes an optional `working` list, the agent's whole working state: each issue it tracks, the chat that owns it, and its status (`working`, `waiting`, `handoff`, `stalled`, `watching`) with a note. The computer saves it next to the decision log (`foreman/working.json`), keyed by the agent's definition ID, stamps each item with when it first saw it in that status, and puts the list with those times in every check-in. The first check-in after a restart, an app upgrade or a context reset resumes from it.

The computer's `gh` reports the issue and claim events. Each check-in asks Foreman to call `record_decision` with what it saw, decided and did. The **Decision log** in the same card lists check-ins, those records, how each check-in ended, and authority changes. The log is a file in the app's data folder (`foreman/decisions.jsonl`), kept across restarts, and the next check-in includes the latest entries.

## What it is

Foreman is a published agent: anyone can start from it, and when it changes,
the next **Fork Foreman** gets the new version without an app update
([#12048](https://github.com/OpenAgentsInc/openagents/issues/12048)). It is
Claude Code on Opus 5.5 with the host's own access, short instructions, and
its playbook as eight skills: where it looks, how it reasons, what it may
change, dispatching issue work, upgrading itself, staying responsive,
check-ins, and stalls. The copy in
this repository is [`agent_lab::templates::foreman`](../../crates/agent-lab/src/templates.rs)
with its text in [`templates/foreman/`](../../crates/agent-lab/src/templates/foreman/);
the app falls back to it when it can't reach the published one. It reads two sources:

| Source | What Foreman reads |
| --- | --- |
| GitHub, through the computer's `gh` | Open and recently closed issues, claim markers, native blockers, project 19 and 22 status, and recent commits that name issues |
| The app's agent tools | Every chat on every device, with harness, model, live status, last activity, and its harness session id |

The harness session id is the join between the two
([#11927](https://github.com/OpenAgentsInc/openagents/issues/11927)): a claim
marker `session=claude-code:ID` belongs to the chat whose `harnessSessionId` is
`ID`. A claim that matches no chat comes from an agent outside the app, such as a
terminal session, and Foreman says so.

## Publish your own

To share your version, change its skills or instructions in
`crates/agent-lab/src/templates/foreman/` and run the SDK example above with
`--built-in --promote --publish`. It prints the published ID; anyone can read
it at `https://openagents.com/v1/published-agents/ID`, signed in or not. For a
new revision, run it again with `OPENAGENTS_PUBLISHED_ID=ID`. Only the account
that published it can add revisions.

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
