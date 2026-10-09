# Coding agents

Coder doesn't bring its own model account. It runs the coding agents you
already use, on your computer, under your own sign-in with each one.

| Agent | Order | Model when you don't name one |
| --- | --- | --- |
| Codex | First | `gpt-6.1-sol`, medium reasoning |
| Claude Code | Second | `claude-opus-5-5` |
| Grok Build | Third | Grok Build's own default |
| Devin | Fourth | Devin's own default |
| OpenCode | Fifth | The model OpenCode's own configuration names |

Every agent signed in on this computer is on, with nothing to enable.
Coder tries them in order and uses the first one that is signed in and has
room in its usage window. Devin bills its own paid API for each run; turn
it off if you don't want Coder to use it.

## Sign in

Each agent is signed in with its own program, on the same computer and as
the same user that runs Coder:

1. Install the agent's command-line program. Codex:
   `npm install -g @openai/codex`. Claude Code:
   `npm install -g @anthropic-ai/claude-code`. OpenCode: see
   [opencode.ai](https://opencode.ai). Grok Build and Devin install from
   their makers' own instructions.
2. Run it once in a terminal (for example `codex`, `claude`, or
   `opencode`) and sign in.
3. Check: the Mac app's sidebar and **Settings → Coder** say **Signed in**
   for it, and OpenAgents Terminal lists it under **Agents**.

Coder only checks whether each agent is signed in. It never reads or sends
your sign-in itself.

## Ask for one

Name the agent in your message: "run this with Claude Code", "do it with
Codex". Coder puts that agent first for the run, unless you turned it off
on that computer. If it can't use it, because it isn't signed in, is
turned off, or has no room right now, the start says why and names the one
running instead. This works from the phone too.

There's no model picker in the apps yet. To choose a model, name it in the
settings (`codex:MODEL`, `opencode:PROVIDER/MODEL`); see the
[settings reference](/docs/settings).

## Failover

If an agent turns Coder away during a run, Coder records it and switches
to the next agent, and the chat says it switched. An agent turned
away earlier is skipped until it has room again.

## Turn an agent off, or change the order

Every signed-in agent is used unless you turn it off.

- **Mac app:** **Settings → Coder → Agents Coder may run.** Turn one off to
  keep Coder from using it. At least one stays on.
- **OpenAgents Terminal:** `/settings`, the same switches.
- **Anywhere:** `openagents settings disable devin` (and `enable devin`).
  `openagents settings set coder.providers claude,codex` puts Claude Code
  first; agents you leave out still follow.

Delegated sessions show inside their Coder chat: when a task hands work to
OpenCode or Devin, that session appears as a row you can open. Separate
Codex, Claude Code, OpenCode, and Devin sessions aren't listed in the chat
list; in the Mac app, **Saved sessions** shows your Codex and Claude Code
sessions.

Next: [Watch, steer, and stop Coder](/docs/following-coder).
