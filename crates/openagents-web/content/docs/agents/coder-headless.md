# Run Coder headless

Coder, our coding agent, runs from a script or from another agent with
`openagents coder chat`: one prompt in, a stream of JSON lines out, and
a session you can continue.
Coder runs on your own computer, in your own Git checkout.

## What you need

- The `openagents` command. Install it (macOS and Linux):

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

- A Git checkout to work in. Coder works in its own worktree of it, on a
  branch named `chat/<session>`, so your folder is left alone.

## Step 1: send one prompt

```sh
cd ~/code/my-repo
openagents coder chat -p "Add a test for the date parser." --json
```

Give the prompt one of three ways, exactly one per call:

| Flag | Where the prompt comes from |
| --- | --- |
| `-p TEXT` or `--prompt TEXT` | The command line |
| `--prompt-file FILE` | A file |
| `--stdin` | Standard input, up to 1 MiB of UTF-8 |

```sh
echo "Explain what src/lib.rs does." | openagents coder chat --stdin --json
```

## Step 2: read the JSON lines

With `--json`, each line on standard output is one JSON object. The last
line is `finished`, or `{"error": "..."}` if the run failed.

```json
{"event":"entry","session":"S","index":0,"entry":{"source":"user","text":"Add a test for the date parser."}}
{"event":"delta","session":"S","text":"I'll add "}
{"event":"entry","session":"S","index":1,"entry":{"source":"tool","name":"edit","input":"...","output":"...","running":true}}
{"event":"entry","session":"S","index":2,"entry":{"source":"assistant","text":"I added a test ...","model":"...","elapsed_ms":5120}}
{"event":"finished","session":"S","reply":"I added a test ...","tokens":18342,"trajectory":"/Users/me/.openagents/coder/..."}
```

| `event` | Fields | Meaning |
| --- | --- | --- |
| `delta` | `session`, `text` | New words of the answer being written |
| `entry` | `session`, `index`, `entry` | One entry of the chat, sent again each time it changes |
| `delegation_delta` | `session`, `delegation`, `text` | New words from a helper agent Coder handed work to |
| `delegation_entry` | `session`, `delegation`, `index`, `entry` | One entry of that helper's chat |
| `finished` | `session`, `reply`, `tokens`, `trajectory` | The run ended: the last answer, tokens used, and where the saved chat is |

An `entry` has a `source` and these fields:

| `source` | Fields |
| --- | --- |
| `user` | `text` |
| `assistant` | `text`, `model`, `elapsed_ms` |
| `tool` | `name`, `input`, `output`, `running` |
| `delegation` | `id`, `name`, `task`, `running`, `output` |

Exit codes: `0` when it finished, `1` when it failed, `64` for a wrong
flag, `130` when it was stopped.

## Step 3: continue the same session

Each run has a session id: the `session` field on every line. Pass it
back with `--session` to continue that chat, with its history and its
worktree:

```sh
openagents coder chat --session S -p "Now run the tests and fix any failure." --json
```

Without `--session`, a new session starts. Two runs can't write to the
same session at once; the second one fails. List and read saved
sessions:

```sh
openagents coder sessions list
openagents coder sessions read S
```

## Step 4: choose who runs the work

`--mode` picks the agent that does the work:

| `--mode` | Who runs it | Needs |
| --- | --- | --- |
| `auto` (the default) | We pick for each turn | Nothing |
| `coder` | Coder's own loop | A model set with `openagents coder models set` |
| `codex` | Codex, on this computer's ChatGPT login | Codex logged in, and the Codex plugin on |
| `opencode` | OpenCode, on this computer | OpenCode installed (`npm install -g opencode-ai`) and the OpenCode plugin on |

```sh
openagents coder chat --mode opencode -p "Rename the config module to settings." --json
```

A mode you pick holds for the rest of that session, also on turns sent
without `--mode`. Send `--mode auto` to let us pick again.

Turn plugins on and off from the command:

```sh
openagents coder plugins list
openagents coder plugins enable opencode
```

## Other flags

| Flag | What it does |
| --- | --- |
| `--instructions TEXT` or `--instructions-file FILE` | Extra instructions for this run |
| `--in-place` | Work in this folder instead of a new worktree |
| `--in DIR` | Run as if started in `DIR` |
| `--approvals stdin` | Ask before risky steps: an `{"event":"approval","id":...}` line, answered by writing `confirm ID` or `reject ID` to standard input |
| `--approvals tool-free` | Run with no tools that need approval |

## Choose the model

`openagents coder chat` has no model flag. Set the model once:

```sh
openagents coder models list
openagents coder models set google/gemini-3.8-flash
```

## See also

- [The `openagents` command for agents](/docs/agents/cli)
- [Coder scripts](/docs/coder-scripts), the guide for people
