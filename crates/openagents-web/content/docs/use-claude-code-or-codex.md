# Use your Claude Code or Codex

Already pay for Claude or ChatGPT? Coder runs Claude Code and Codex on
your own login, so their work comes out of your plan, not out of anything
we charge. You keep one chat, with the same keys, commands, and safety
whichever agent answers.

## 1. Set up the agent once

Install the agent's own program and log in to it, on the same computer and
as the same user that runs Coder ([more](/docs/coding-agents)).

Claude Code:

```sh
npm install -g @anthropic-ai/claude-code
claude
```

Codex:

```sh
npm install -g @openai/codex
codex
```

Log in when the program asks, then quit it (`Ctrl+C`). Coder never reads
or sends your login; it only checks that you're logged in.

## 2. Start Coder

```sh
cd ~/code/my-project
coder
```

A new chat starts on **Auto**: Coder picks who answers from the agents
that are on and logged in here. Once it has answered, the label at the
input's top right says who, such as `Auto → Claude (Haiku 5.5)`.

## 3. Pick the agent yourself

Before the chat's first message, press `Shift+Tab` to step through who can
answer: Auto, Claude Code, Codex, OpenCode, then the other agents found on
this computer. Stop on the one you want and send your message. A chat
keeps its agent; press `Ctrl+N` for a new chat to switch.

You can also just say it: "use Codex for this".

## 4. Watch your plan's usage

Whenever a turn runs on your Claude or Codex plan, the label shows how
much of that plan's usage windows you've used, such as
`5h 12% · week 15%`: 12% of the five-hour window and 15% of the weekly
one. Auto skips an agent whose window has run out, and leans away from
Claude Code when its five-hour window is nearly used up.

## 5. Use both

One chat can hand a piece of work to the other agent and keep going:

```text
Have Codex review this change for bugs.
```

The other agent works in its own conversation, shown as a row under the
input. Press `Down` to look at it and `Esc` to come back; its report joins
your chat when it's done.

## What stays the same

- **Your setup.** Claude Code runs with your own settings and skills, and
  every agent reads your project's `AGENTS.md` and `CLAUDE.md`.
- **Your keys.** `Enter` sends, `Ctrl+Enter` queues, and `Esc` stops,
  whichever agent answers.
- **Your safety.** Deploys, merges, secrets, payments, and DNS changes wait
  for your yes or no on every agent. See [Safety](/docs/coder-safety).

## Turn one off

Type `/plugins` (or press `Ctrl+P`), choose **Claude Code** or **Codex**,
and press `Space`. From a shell:

```sh
openagents coder plugins disable codex
```

Turn it back on with `openagents coder plugins enable codex`.

Other agents work the same way: Coder runs OpenCode, Grok Build, Devin
CLI, and Cursor too, each on its own login. See
[Choose who answers](/docs/coder-agents).

Next: [Pin a model or use Auto](/docs/pin-a-model).
