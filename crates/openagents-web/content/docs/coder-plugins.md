# Coder's plugins

Coder's abilities come as plugins you can turn on and off. Type
`/plugins`, or press `Ctrl+P` (`Cmd+P` on macOS) or `F2`.

| Key | Does |
| --- | --- |
| Up, Down | Choose a plugin. |
| Space | Turn it on or off. |
| Enter | Open its settings. |
| Tab | Move between fields and buttons in settings. |
| Esc | Go back. |

Your choices are saved and kept when you quit.

## The built-in plugins

| Plugin | Starts | What it does |
| --- | --- | --- |
| **Claude Code** | On | Runs chats on Claude Code with this computer's Claude sign-in: everything Claude Code can do, with your own settings and skills, plus Coder's handoffs to other agents. While it's on and signed in, new chats can run on it. |
| **Codex** | On | Runs chats on Codex with this computer's ChatGPT sign-in, with its own models. Auto can pick it, or you can choose it with `Shift+Tab`. |
| **OpenCode** | On | Runs chats on OpenCode, with its free model or the models you set up in it; Coder checks each step before it runs. Listed only when OpenCode is installed. |
| **Coder loop** | On | Coder's own coding loop, on OpenAgents models with your sign-in or on your OpenRouter key. |
| **ACP Subagents** | On | Hands tasks to other coding agents installed here, such as Devin CLI, Cursor, Grok Build, Goose, Amp, or Kimi Code. |
| **Solver** | On | Takes a GitHub issue to a pull request. See [Solve an issue](/docs/coder-solver). |
| **Solver swarm** | On | Hands several issues to several Solvers at once. |
| **Agent channel** | On | Lets the agents a chat hands work to tell each other what they're changing, and warns them about overlaps. |
| **Token savers** | On | Cuts what the agent reads and pays for: long build and test output is trimmed to its errors, with the full output saved to a file. |
| **OpenAgents CLI** | On | Lets Coder run the `openagents` command in this folder, for example to read and comment on GitHub issues. |
| **Jev** | On | Quick judgment calls while Coder works, like whether a step is done, from Jev, TypeSafe's small decision model. Uses your TypeSafe key or a compatible service. |
| **OpenRouter BYOK** | Off | Use OpenRouter models with your own OpenRouter key. |
| **Brainstorm** | Off | Looks up public profiles and how much influence they have, when you ask with `/brainstorm`. |
| **Boat Cloud**, **GCE Cloud** | Off | Run handed-off work on cloud machines you already have access to. |

Turning a plugin off keeps its settings and keys, so turning it back on
needs no setup.

## OpenRouter BYOK

Use any OpenRouter model on your own key
([get one at openrouter.ai](https://openrouter.ai/keys)).

1. In `/plugins`, choose **OpenRouter BYOK** and press `Enter`.
2. Paste your key in **OpenRouter API key**. Press `Tab` to reach
   **Save**, then `Enter`. Coder checks the key and shows **Verified**.
3. Back in the list, make sure the plugin is on (`Space`).
4. Type `/models` and pick a model. `openrouter/free` is OpenRouter's free
   router; fixed models also ask for a reasoning level and an output
   limit.

Or start Coder with the key in your environment, and Coder picks it up:

```sh
export OPENROUTER_API_KEY=sk-or-...
coder
```

Coder also reads `OPENROUTER_API_KEY` from a `.env` file in the folder you
open. Keys are never shown on screen; to remove one, clear the field and
save.

## Jev

Jev answers small typed questions (yes or no, pick one, a score) with
probabilities, which Coder and its own loop use for quick decisions.

1. Get a key at [typesafe.ai](https://typesafe.ai).
2. In `/plugins`, choose **Jev** and press `Enter`.
3. Pick the connection (TypeSafe, or Vercel AI Gateway), paste the key,
   then press `Tab` to reach **Save** and `Enter`.

Or set `TYPESAFE_API_KEY` in your environment or `.env` before you run
`coder`.

## ACP Subagents

Open **ACP Subagents** in `/plugins` to see the coding agents Coder found
on this computer, each with a checkbox. New ones start on.

- `Space` or `Enter` turns the chosen agent on or off.
- `R` looks for installed agents again.

Each agent uses its own sign-in. Install and sign in with its own program
first: for Cursor, `cursor-agent` with its login or `CURSOR_API_KEY`.
Devin CLI bills its own paid service for each run; turn it off here if you
don't want Coder to use it.

## Token savers

When it's on, long build, test, and lint output (from `cargo test`,
`npm test`, `pytest`, `go test`, `make`, and the like) reaches the agent
trimmed to the lines that matter, with the whole output saved to a file
it can open when it needs more. This works on Claude Code and on Coder's
own loop, and Codex is asked to run such commands the same way. Turn it
off to send all output through unchanged.

## Plugins you install

On macOS and Linux, plugins you install with `openagents plugin install`
also appear in the list. `Enter` shows one's details and the commands to
turn it on or off. See [Plugins](/docs/plugins) for writing and sharing
your own.

## From the command line

```sh
openagents coder plugins list
openagents coder plugins disable token-savers
openagents coder plugins enable openrouter-byok
printf '%s' '{"api_key":"sk-or-..."}' | openagents coder plugins configure openrouter-byok --stdin
openagents coder plugins check openrouter-byok
```

The plugin IDs are `claude-code`, `codex`, `opencode`, `microcoder`
(Coder loop), `acp-subagents`, `solver`, `solver-swarm`, `comms` (Agent
channel), `token-savers`, `openagents-cli`, `jev`, `openrouter-byok`,
`brainstorm`, `boat-cloud`, and `gce-cloud`.

Next: [Solve an issue](/docs/coder-solver).
