# Choose who answers

Coder is one chat, whoever answers it. Each chat runs on one of these:

| Who | What it needs on this computer |
| --- | --- |
| **Claude Code** | Claude Code installed and signed in to your Claude account (`claude`, then sign in). |
| **Codex** | Codex installed and signed in with ChatGPT (`codex`, then sign in). |
| **OpenCode** | OpenCode installed ([opencode.ai](https://opencode.ai)). It can use its free model or the models you set up in it. |
| **Coder's own loop** | Your openagents.com sign-in (`coder login`), or an OpenRouter key. |
| **Other agents** | Devin, Cursor, Grok Build, Amp, Goose, and others, each installed and signed in with its own program. |

Claude Code and Codex run on your own plans, so they cost you nothing
through OpenAgents.

## Auto

A new chat starts on **Auto**, shown at the input's top right. Coder looks
at your message and picks who answers among the ones that are on and
signed in here. With no other reason to choose, it tries Claude Code,
then OpenCode, then its own loop. Follow-ups stay with the same agent.

Once Auto has picked, the label says so, with the model:
`Auto · Claude (Haiku 5.5)` or `Auto · Coder (auto)`.

You can also just ask in your message: "use Codex for this".

## Pick with Shift+Tab

`Shift+Tab` steps through who can answer here, in this order:

**Auto → Claude Code → Codex → OpenCode → the other agents found on this
computer** (such as Devin, Cursor, or Grok Build), then back to Auto.

Only the ones that can run here are offered. Your pick holds for this
chat until you press `Shift+Tab` again; your draft stays as it is. It is
a choice inside the same chat, not a separate mode.

## Plan windows

Beside **Claude Code** and **Codex**, the label shows how much of your
plan's usage windows you've used, such as `5h 12% · week 15%`: 12% of the
five-hour window and 15% of the weekly one. Auto skips an agent whose window
has run out, and leans away from Claude Code when its five-hour window is
nearly used up.

## Pick a model

`/models` opens the models of whoever answers this chat. Type to search,
use Up and Down to choose, and `Enter` to go on; `Esc` steps back.

| Who answers | Models in `/models` |
| --- | --- |
| Claude Code | Haiku 5.5 (the default), Sonnet 5.5, Opus 5.5, and Fable 5.1, each with a reasoning level from low to max. |
| Codex | GPT-6.1-Sol (the default), GPT-6-Astra, GPT-6-Sol, and GPT-6-Luna, each with a reasoning level. |
| OpenCode | The models your OpenCode can reach. |
| Coder's own loop | **Auto** (OpenAgents picks the model), the OpenAgents models your account can use, and, with the OpenRouter plugin on, OpenRouter models on your key. |

The new model answers from your next message, in the same chat. If
`/models` says to sign in, run `/login`, or turn on OpenRouter BYOK in
`/plugins` ([how](/docs/coder-plugins)).

## Preferences and pins

`/preferences` sets how much Auto should favor **Cost efficiency**,
**Speed**, and **Intelligence**, each from 1 to 5. Up and Down move,
Left and Right (or `-` and `+`) change a value, and each change is saved
at once.

To make a standing choice, tell Coder in plain words:

- "Always use Claude Opus for code in this repo."
- "Use Codex less."
- "Never use Devin."

Coder saves it as a pin, for one project, one kind of work, or
everywhere. `/preferences` lists your pins in plain words, and the file
it names holds every change with your notes. An agent you said never to
use is never started.

## Hand work to another agent

Any chat can hand a piece of work to another agent and keep going. Ask
for it: "have Codex review this change", "ask Claude Code to write the
tests". The agent works in its own conversation, and a row for it
appears under the input with its task, time, and tokens.

- `Down` from the input selects the first row, and shows that agent's
  conversation.
- `Up` and `Down` move between agents; `Up` from the first one, or `Esc`,
  returns to your chat.

When it's done, its short report comes back to your chat.

From a script or another agent:

```sh
openagents coder delegate --to codex --task "Review the last commit for bugs" --read-only
```

`--to` takes `claude-code`, `codex`, `opencode`, `coder`, an agent ID
from `openagents coder agents list`, or `auto`.

## Background agents

A background agent works at the same time as your chat, in its own copy
of your project (a Git worktree on its own branch), with no time limit.
Your chat keeps going, and the agent's report joins the chat when it
ends.

Start one yourself:

```text
/agent codex fix the flaky login test
```

Type `/agent` alone to list the agents you can start here (the ones on in
**ACP Subagents**, and `microcoder` for Coder's own loop). The chat can
also start them itself when work splits up.

`/agents` lists them with their status, time, tokens, and cost:

| Key | Does |
| --- | --- |
| Up, Down | Choose an agent. |
| Enter | Open its conversation. |
| s | Stop it. |
| m | Send it a message. |
| r | Resume one that stopped. |
| Esc | Close the list. |

The agents a chat starts can also talk to each other while they work
(the **Agent channel** plugin): each says what it means to change, and
they hear when their changes would overlap.

Next: [Coder's plugins](/docs/coder-plugins).
