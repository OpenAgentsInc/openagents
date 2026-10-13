# Choose who answers

Coder is one chat, whoever answers it. Each chat runs on one of these:

| Who | What it needs on this computer |
| --- | --- |
| **Claude Code** | Claude Code installed and signed in to your Claude account (`claude`, then sign in). |
| **Codex** | Codex installed and signed in with ChatGPT (`codex`, then sign in). |
| **OpenCode** | OpenCode installed ([opencode.ai](https://opencode.ai)). It can use its free model or the models you set up in it. |
| **Coder's own loop** | Your openagents.com sign-in (`coder login`), or an OpenRouter key. |
| **Other agents** | Devin CLI, Cursor, Grok Build, Amp, Goose, and others, each installed and signed in with its own program. |

Claude Code and Codex run on your own plans, so they cost you nothing
through OpenAgents.

## Auto

A new chat starts on **Auto**, shown at the input's top right. Coder looks
at your message and picks who answers among the ones that are on and
signed in here. With no other reason to choose, it tries Claude Code,
then OpenCode, then its own loop. Once it has answered, the whole chat
stays with that agent and model; a new chat (`Ctrl+N`) can pick again.

Once Auto has picked, the label says so, with the model once it is
known: `Auto → Claude (Haiku 5.5)` or `Auto → Coder (Haiku 5.5)`. An agent
you pinned with Shift+Tab shows alone, without Auto: `Claude (Haiku 5.5)`.
A short question Auto sends to Coder's own loop runs on a cheap model:
OpenAgents' quick model on your free daily allowance when you're signed
in (`coder login`), else the cheapest model on your login (Haiku on the
Claude plan), never Opus or Sonnet unless you chose them.

You can also just ask in your message: "use Codex for this".

## Pick with Shift+Tab

`Shift+Tab` steps through who can answer here, in this order:

**Auto → Claude Code → Codex → OpenCode → the other agents found on this
computer** (such as Devin CLI, Cursor, or Grok Build), then back to Auto.

`Shift+Tab` works in a new chat, before you send its first message. Only
the ones that can run here are offered, and your draft stays as it is.
Once the chat has a reply, it keeps its agent: `Shift+Tab` says
"Start a new chat (Ctrl+N) to switch agents" instead.

An agent that doesn't ask before every step, such as Cursor, says so
once on the line under the input when you pick it.

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
| OpenCode | OpenCode's own model list: free models first (newest first, then Big Pickle), then your favorites and recent models from OpenCode, then each provider's models. |
| Devin CLI | Devin's own models by family, SWE-2 first. A chat runs SWE-2 Medium unless you pick another. |
| Grok Build | The models `grok models` lists, with Grok's default marked. |
| Coder's own loop | **Auto** (OpenAgents picks the model), the OpenAgents models your account can use, and, with the OpenRouter plugin on, OpenRouter models on your key. |
| Auto, before it has answered | Coder's own loop's models: Auto picks the agent for each message, and these are the models it runs on Coder's own loop. |

`/models` always lists the models of the agent shown at the input's top
right, including one you just picked with `Shift+Tab`. Pick a model before
the chat's first message; the chat then keeps it. After that, `/models`
shows the chat's model, and another pick says "Start a new chat (Ctrl+N)
to use another model". Each new chat starts on its agent's default
model. A chat you reopen with `/resume`, or continue from your phone or
the web, goes on with the agent, model, and payment it ran on. If that
agent isn't on this computer any more, the line under the input says so
and the chat doesn't move to another one. If
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
appears under the input, below a `main` row for your chat, with what it
is doing right now, its time, and its tokens.

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

## Several things at once

Send a list of separate jobs and Coder works out which ones need each
other. The ones that don't start at the same time, each with its own
agent in its own worktree; one that needs another's result waits for it
and starts from its changes. Each part gets a row under the input while
it works.

In a chat you pinned with Shift+Tab, the list still splits, and every
part runs on the harness or agent you picked.

When they finish, their changes come back to your folder together, the
checks they ran are run once on the result, and one report lists how
each part went. Coder never overwrites anything: if two parts (or you)
changed the same lines, that part's changes stay on its own branch and
the report names the branch and the files.

Two to four parts run at once, depending on how many cores your
computer has. To set your own number,
add `"parallel_parts": 2` to `~/.openagents/coder/preferences.json`, or
start Coder with it for one session:

```sh
CODER_PARALLEL_PARTS=2 openagents coder
```

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
