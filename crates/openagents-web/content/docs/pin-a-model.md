# Pin a model or use Auto

Every new chat starts on **Auto**: Coder looks at your message and picks
who answers and on which model. You can pin your own choice instead, for
one chat or for every new chat. This guide shows when each makes sense
and how to do it.

## What Auto does

- It picks among the agents that are on and logged in on this computer.
  With no other reason to choose, it tries Claude Code, then OpenCode,
  then Coder's own loop.
- A short question can go to Coder's own loop on a cheap model: our quick
  model on your free daily allowance when you're logged in, else the
  cheapest model on your login (Haiku on the Claude plan). It never picks
  Opus or Sonnet for that unless you chose them.
- It skips an agent whose usage window has run out.
- Once it has answered, the chat stays with that agent and model.

Use Auto when you don't mind who answers and want the cheap answer for
small things. Pin when you need a particular model, such as the strongest
one for a hard change, or want the work to stay on one plan.

## Pin one chat

Both choices are made before the chat's first message.

1. Press `Shift+Tab` to pick who answers: Auto, Claude Code, Codex,
   OpenCode, or another agent on this computer.
2. Type `/models` to pick that agent's model and reasoning level. Type to
   search, press `Up` and `Down` to move, and `Enter` to go on.
3. Send your message.

The label at the input's top right then shows your pick alone, such as
`Claude (Opus 5.5)`, without "Auto". The chat keeps it. To use another
model later, press `Ctrl+N` for a new chat.

| Who answers | What `/models` offers |
| --- | --- |
| Claude Code | Haiku 5.5 (the default), Sonnet 5.5, Opus 5.5, and Fable 5.1, each with a reasoning level from low to max. |
| Codex | GPT-6.1-Sol (the default), GPT-6-Astra, GPT-6-Sol, and GPT-6-Luna, each with a reasoning level. |
| OpenCode | OpenCode's own list: free models first, then your favorites and recent models, then each provider's models. |
| Coder's own loop | **Auto** (we pick the model), the models your account can use, and OpenRouter models on your own key. |

## Start every new chat on one agent

To make new chats start on Claude Code, Codex, or OpenCode without
pressing `Shift+Tab` each time:

```sh
openagents coder models set claude-code
```

Use `codex` or `opencode` in its place, or `opencode/PROVIDER/MODEL` for
one OpenCode model. Check what's set:

```sh
openagents coder models list
openagents coder status
```

To go back to Auto choosing:

```sh
openagents coder models set auto
```

## Say it in plain words

Coder also keeps standing choices you tell it:

```text
Always use Claude Opus for code in this repo.
```

Other examples: "Use Codex less." "Never use Devin." Coder saves each as
a pin, for one project, one kind of work, or everywhere. An agent you said
never to use is never started. Type `/preferences` to see your pins in
plain words.

## Lean Auto toward cost, speed, or quality

Type `/preferences` and set **Cost efficiency**, **Speed**, and
**Intelligence** from 1 to 5. Press `Up` and `Down` to move, and `Left`
and `Right` (or `-` and `+`) to change a value; each change is saved at
once.

More detail: [Choose who answers](/docs/coder-agents).

Next: [Queue messages while Coder works](/docs/queue-messages).
