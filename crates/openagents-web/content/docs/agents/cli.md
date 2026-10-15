# The openagents command for agents

The `openagents` commands an agent uses most, with their exact forms. The
full list, with every flag, comes from the command itself:

```sh
openagents capabilities
```

That prints the version and every command as JSON. The same tree is in
the repository at
[`crates/coder/src/cli_route/tree.json`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/coder/src/cli_route/tree.json)
(schema `openagents.cli-tree.v1`), checked against the command's own help
by a test.

## Install

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

The installer puts `openagents` and `coder` in `~/.openagents/bin`.

## Output for programs

| Rule | Detail |
| --- | --- |
| `--json` | One JSON document per command on standard output. Put it anywhere before `--`. After `inference`, `--json BODY` is the request body instead |
| Errors under `--json` | `{"error": "..."}`, with `"usage": true` for a wrong flag |
| Streams | `openagents coder chat --json` and `openagents chat send --json` print one JSON object per line |
| Exit codes | `0` done, `1` refused or failed, `64` wrong usage, `130` stopped |

## Two kinds of credential

| Credential | Looks like | Used by | How to give it |
| --- | --- | --- | --- |
| API key | `oak_<id>.<secret>` | `openagents inference` | `OPENAGENTS_API_KEY`, or `--key KEY` |
| Login key (an agent acting as you) | `oa_sk_...` | `projects`, `chat`, `keys`, and the other account commands | `OPENAGENTS_TOKEN`, or `openagents login --key` |

Save a login key once. The command reads it from standard input, never
from the command line:

```sh
openagents login --key < key.txt
```

It is kept in `~/.openagents/coder/account.json`. `OPENAGENTS_TOKEN`
takes its place when set, and `OPENAGENTS_ORIGIN` points the command at
another site (the default is `https://openagents.com`).

## Models: `inference`

```text
openagents inference [run] MODEL [TEXT] [--input TEXT] [--json BODY] [--instructions TEXT]
  [--stream] [--format text|json|events] [--api responses|chat] [--max-output-tokens N]
  [--max-price IN/OUT] [--base URL] [--key KEY] [--pay x402 --max-msat N]
openagents inference models [--base URL] [--key KEY]
openagents inference rates [--base URL]
```

```sh
openagents inference openagents/chat "Say hello." --stream
openagents inference google/gemini-3.8-flash "Say hello." --pay x402 --max-msat 10000
```

`MODEL` is a model id or a task name: `openagents/auto`, `chat`, `fast`,
`code`, `long`, `reason`, or `classify`. `--pay x402` pays per request from
your wallet and can't be used with a key. `--base` (or
`OPENAGENTS_API_BASE`) changes the API address, `https://api.openagents.com/v1`
by default.

## Chat: `chat`

```text
openagents chat send MESSAGE [--thread ID] [--no-run] [--timeout SECONDS] [--project PROJECT]
openagents chat threads [--all] [--limit N] [--account]
openagents chat read --thread ID
openagents chat follow --thread ID
openagents chat answer --thread ID TEXT
openagents chat stop --thread ID
openagents chat export --thread ID
```

`openagents chat MESSAGE` is the same as `chat send`, and a `MESSAGE` of
`-` is read from standard input. Under `--json`, `send` prints one event
per line. See [Start a chat and stream the answer](/docs/agents/chat-and-stream).

## Coder: `coder`

```text
openagents coder chat [-p TEXT | --prompt-file FILE | --stdin] [--session ID]
  [--instructions TEXT | --instructions-file FILE] [--mode auto|coder|codex|opencode] [--in-place]
openagents coder sessions list | read ID | delete ID
openagents coder models list | set SLUG
openagents coder plugins list | enable ID | disable ID
openagents coder status
```

See [Run Coder headless](/docs/agents/coder-headless).

## Projects: `projects`

```text
openagents projects list
openagents projects show PROJECT
openagents projects create NAME [--repo OWNER/NAME]... [--instructions TEXT] [--protection standard|only_me]
openagents projects files list PROJECT
openagents projects files add PROJECT PATH... [--protection standard|only_me]
openagents projects files get PROJECT FILE [--output PATH]
openagents projects notes add PROJECT --title TITLE (--text TEXT | --file PATH)
openagents projects members PROJECT
openagents projects invite PROJECT [--github USERNAME] [--role editor|viewer] [--ttl DURATION]
openagents projects threads PROJECT
openagents projects read PROJECT [CHAT]
```

See [Create a project and invite a person](/docs/agents/projects).

## Keys: `keys` and `login`

```text
openagents login --key [--account-dir DIR]
openagents keys list
openagents keys create --name NAME [--expires never|30d|90d] [--kind signin|api]
openagents keys show KEY
openagents keys revoke KEY
openagents keys rotate KEY
```

A new key is printed once. `--kind signin` (the default) makes a login
key; `--kind api` makes an `oak_` API key, which doesn't expire.

## Bitcoin: `wallet` and `x402`

```text
openagents wallet balance
openagents wallet receive [--amount SATS] [--bitcoin]
openagents wallet send TO [--amount SATS] [--yes]
openagents wallet history [--limit N]
openagents x402 fetch URL [--method M] [--body FILE|-] [--max-msat N] [--max-fee-msat F]
```

`wallet send` asks before it pays unless you add `--yes`. `x402 fetch`
pays a `402` answer from any x402 server, up to `--max-msat`. See
[Pay per call with x402](/docs/agents/x402).

## MCP: `mcp serve`

```sh
openagents mcp serve
```

Serves the command's read-only commands as MCP tools over standard input
and output (newline-delimited JSON-RPC; protocol `2025-06-18`,
`2025-03-26`, or `2024-11-05`). Commands that spend, sign, or show a
secret are never served. See [Connect to our MCP servers](/docs/agents/mcp).
