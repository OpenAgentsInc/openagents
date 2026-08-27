---
name: openagents-cli
description: Use the OpenAgents CLI to read and write issues, projects, repositories, and the forum, to call any API route, and to sign a person in. Use it whenever the work touches OpenAgents itself rather than the files in this repository.
---

# The OpenAgents CLI

You are running inside `openagents coder`, which is one subcommand of the
`openagents` CLI. The `openagents` tool runs the rest of it. The same binary
answers, so what you see is this machine's build, not a remembered one.

## Finding a command, and reading its answer

The `openagents` tool's own description lists every command and subcommand,
read from this binary. You do not need to go looking for them, and you do not
need `--help` to find out that a command exists.

Use `<command> --help` for a flag you do not know. This file lists no flags on
purpose: a written copy goes stale the first time one changes, and the help
output cannot.

**Read the plain output.** It is what a person reads and it is small. A list of
three issues is 442 bytes plain and 20,000 as JSON, because the JSON carries
every issue's whole body — and a session that reads twenty thousand bytes to
answer one question pays for them on every turn after it, too.

Add `--json` only when you need one field out of one record. Prefer a narrower
command over a wider one you then read past: `--label`, `--state`, `--limit`
and a search term cost nothing and cut the answer to what was asked.

## What works with no credential

- `openagents computer probe|policy|status` — inspects this machine. The local
  machine controls all access; no account is involved.
- `openagents coder --offline` — answers from a built-in stand-in.
- `--help` anywhere.

Everything else reaches the API and needs a token. Without one you get:

```
openagents: No OpenAgents token is available for https://openagents.com.
Set OPENAGENTS_TOKEN.
```

Read that as "this person is not signed in", not as a broken command.

## What signing in involves

A token is per API origin, stored in the OS credential store, and carries
scopes. Two matter:

- `chat:account` — open a thread and talk to a model.
- `forge:write` — push to the forge and write to issues, projects, and the
  forum.

An ordinary sign-in mints both. Do not ask for one scope on its own: a token
minted for `chat:account` alone cannot push, and one minted for `forge:write`
alone cannot open a thread, and each failure arrives a command later where it
reads as the product being broken.

One privileged scope exists beyond these: `deployments:promote`, which the
`openagents deploy` commands need and which the server mints only for a
current operator. Do not request it unless the person is an operator asking to
deploy the fleet; `forge:write` cannot promote.

Check with `openagents auth status`, which names the account, the eligible
namespaces, and the expiry without printing the token.

### Signing a person in

You cannot complete this for them; it needs a browser and their approval.

1. Run `openagents auth login --headless`. It returns a URL and a short code
   and does not block.
2. Give the person the URL and the code, and say what they are approving: this
   CLI, on this machine, for their OpenAgents account.
3. When they say they have approved it, run `openagents auth login --resume`.
4. Confirm with `openagents auth status`.

Tell them what it is for in a sentence — "so the CLI can push to the forge and
open a thread" — rather than only handing over a link. A person asked to
approve something unexplained is right to refuse.

Never print a token, and never paste one into a file, a commit message, or an
issue.

## Reaching a route with no command

`openagents api <path>` sends an authenticated request to any API route and
writes the body as JSON. A path without a leading slash resolves under
`/api/v1/`. Use it when no named command covers what you need — several routes
have no command of their own.

## Two cautions

**Never read stdin from a tool call.** `--body-file -` (and every other
stdin-reading flag, like `auth login --token-stdin`) blocks forever when the
harness spawns the CLI with no stdin — no output, no error, no timeout. The
tool path now closes stdin so the call fails fast instead of hanging, but the
working pattern is unchanged: write the content to a real file with the write
tool and pass its path. The pipe form (`printf '%s' "$body" | openagents ...`)
belongs in the `bash` tool, where EOF arrives. A resumed turn after a hang
re-issues the failed write with a file path — never the verbatim call.

**You are already a coder session.** Do not start another one. To run work in
parallel, use the `delegate` tool, which is what it is for.

**Writes are real.** Closing an issue, posting to the forum, or pushing to a
repository is visible to other people immediately and is not yours to undo. Say
what you are about to write and get agreement before a first write in a
session. Reads need no ceremony.
