# Coder from scripts

`openagents coder` runs the same chat as the terminal app, without the
screen: for scripts, CI jobs, and other agents. It comes with Coder; see
[Download and install](/docs/download). Run `openagents coder --help`
for every command.

## Send a message

```sh
openagents coder chat -p "Explain the files in this folder. Change nothing."
```

The reply prints to standard output. Other ways to give the message:

```sh
openagents coder chat --prompt-file task.md
echo "What changed in the last commit?" | openagents coder chat --stdin
```

## Continue a chat

Give the chat a name with `--session`, and use it again to continue:

```sh
openagents coder chat -p "Add a test for slugify" --session slug
openagents coder chat -p "Now run it" --session slug
```

Without `--session`, each call starts a new chat. Chats are saved like
the terminal's, so `/resume` in `coder` opens them too.

## Where it works

In a Git project, each scripted chat works in its own copy of the project
(a worktree in `.coder/worktrees/`, on a `chat/ID` branch), so your
checkout stays as it was and several chats can run at once. Add
`--in-place` to work in the folder itself, and `--in DIR` to work in
another folder.

## Choose who answers

| Option | Who answers |
| --- | --- |
| `--mode auto` | Coder picks, as Auto does in the terminal (the default). |
| `--mode coder` | Coder's own loop. |
| `--mode codex` | Codex, on this computer's ChatGPT login. |
| `--mode opencode` | OpenCode, on this computer: a headless OpenCode chat. With `--json` its answer streams one JSON object per line as OpenCode writes it, and `--session` continues the same OpenCode chat. Needs OpenCode (`npm install -g opencode-ai`) and its plugin on (`openagents coder plugins enable opencode`). |

A choice holds for the chat's later messages too; `--mode auto` lifts
it. `--instructions TEXT` or `--instructions-file FILE` adds standing
instructions for the chat.

## Models

These set the model for Coder's own loop. Pick a slug from the list:

```sh
openagents coder models list                       # the models you can pick
openagents coder models set openrouter/free        # pick one
openagents coder models set auto                   # let OpenAgents pick
openagents coder models set SLUG --reasoning low --max-tokens 4000
```

## JSON output

Add `--json` to stream one JSON object per line as the chat runs (each
step, its result, and the reply), ending with a `finished` line that has
the reply and the chat's file:

```sh
openagents --json coder chat -p "List the TODOs in src/" --session todo
```

## Ask before acting

```sh
openagents coder chat -p "Clean up the build scripts" --approvals stdin --json
```

With `--approvals stdin`, any command that isn't read-only waits: an
`approval` line appears in the output, and you answer by writing
`confirm ID` or `reject ID` to standard input.

## Hand work to another agent

```sh
openagents coder delegate --to codex --task "Review the last commit for bugs" --read-only
openagents coder delegate --to claude-code --task "Write tests for src/parse.rs"
```

`--to` takes `claude-code`, `codex`, `opencode`, `coder`, `auto`, or any
agent ID from `openagents coder agents list`. `--model` picks its model.
It prints the agent's short result.

## Everything else

| Command | Does |
| --- | --- |
| `openagents coder status` | Who answers, the model, plugins, and the folder. |
| `openagents coder plugins list` | Plugins, and whether each is on. `enable ID` and `disable ID` change one. |
| `openagents coder agents list` | Agents found on this computer. `enable ID`, `disable ID`, `refresh`. |
| `openagents coder sessions list` | Saved chats. `read ID` prints one; `delete ID` removes one. |
| `openagents coder export ID` | Save a chat as a file. |
| `openagents coder memory list` | What Coder remembers here. `memory instructions` lists the instruction files it reads. |
| `openagents coder trace upload --last` | Upload the latest chat as a trace. |

`--state DIR` keeps Coder's data in another folder, so a script doesn't
touch your own settings and chats. Keys come only from environment
variables (such as `OPENROUTER_API_KEY`) or configuration on standard
input, and are never printed.

## The openagents command

`openagents` does more than Coder: chat with OpenAgents, change settings,
work with GitHub issues, connect a phone, and call models on the
OpenAgents API. Run `openagents --help` for the list, and see
[The openagents command](/docs/cli).

Next: [Coding agents](/docs/coding-agents).
