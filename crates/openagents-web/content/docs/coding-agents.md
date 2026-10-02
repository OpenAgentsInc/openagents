# Coding agents

Coder doesn't bring its own model account. It runs the coding agents you
already use, on your computer, under your own sign-in with each one.

| Agent | On by default | Model when you don't name one |
| --- | --- | --- |
| Codex | Yes, first | `gpt-6.1-sol`, medium reasoning |
| Claude Code | Yes, second | `claude-opus-5-5` |
| Grok Build | Yes, third | Grok Build's own default |
| OpenCode | No | None: you always name its model |
| Devin | No | Devin's own default |

Coder tries them in order and uses the first one that is signed in on
this computer and has room in its usage window. OpenCode and Devin are
off until you turn them on: OpenCode needs a model named, and Devin bills
its own paid API for each run.

## Sign in

Each agent is signed in with its own program, on the same computer and as
the same user that runs Coder:

1. Install the agent's command-line program (Codex, Claude Code, Grok
   Build, OpenCode, or Devin).
2. Run it once in a terminal and sign in.
3. Check: the Mac app's sidebar and **Settings → Coder** say **Signed in**
   for it, and OpenAgents Terminal lists it under **Agents**.

Coder only checks whether each agent is signed in. It never reads or sends
your sign-in itself.

## Ask for one

Name the agent in your message: "run this with Claude Code", "do it with
Codex". Coder puts that agent first for the run, if your settings allow it
on that computer. If it can't use it, because it isn't signed in, isn't
turned on, or has no room right now, the start says why and names the one
running instead. This works from the phone too.

There's no model picker in the apps yet. To choose a model, name it in the
settings (`codex:MODEL`, `opencode:PROVIDER/MODEL`); see the
[settings reference](/docs/settings).

## Failover

If an agent turns Coder away during a run, Coder records it and switches
to the next allowed agent, and the chat says it switched. An agent turned
away earlier is skipped until it has room again.

## Choose which agents Coder may use

- **Mac app:** **Settings → Coder → Agents Coder may run.** Turn each on or
  off. Coder tries the ones that are on from the top; one you turn on goes
  last. At least one stays on.
- **Anywhere:** `openagents settings set coder.providers codex,claude`
  lists them in order.

Delegated sessions show inside their Coder chat: when a task hands work to
OpenCode or Devin, that session appears as a row you can open. Separate
Codex, Claude Code, OpenCode, and Devin sessions aren't listed in the chat
list; in the Mac app, **Saved sessions** shows your Codex and Claude Code
sessions.

Next: [Watch, steer, and stop Coder](/docs/following-coder).
