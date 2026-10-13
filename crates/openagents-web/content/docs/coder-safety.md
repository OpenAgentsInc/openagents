# Safety

Coder does ordinary coding work without stopping to ask: reading files,
editing them, and running commands such as builds and tests. A short list
of actions that are hard to undo always waits for you, whichever agent is
answering.

## What always waits for you

| Action | For example |
| --- | --- |
| A production deploy | `wrangler deploy`, `gcloud run deploy`, `fly deploy`, `vercel --prod` |
| Merging a pull request | `gh pr merge` |
| Reading or changing a secret | `gh secret`, `gcloud secrets`, `op read`, reading `.env` files, private keys, or credential files |
| A payment | `openagents wallet send`, `lncli`, `bitcoin-cli`, creating charges with `stripe` |
| A DNS change | `gcloud dns record-sets` |

The same list holds on Claude Code, Codex, OpenCode, and Coder's own
loop, and wrapping a command in a shell (`bash -c '…'`) doesn't hide it.
Pushing commits is not on the list.

## The approval card

When one of these comes up and you're at the terminal, Coder asks you one
plain question, such as "Let Claude (Haiku 5.5) merge a pull request?",
with the exact command.

| Key | Does |
| --- | --- |
| Y | Allow it, this one time. |
| N, Esc | Deny it. |
| PageUp, PageDown | Read the whole command. |

Either way the agent carries on with the rest of the work. If nobody is
there to answer (in a script, for example), the action is held: the
agent finishes what it can and hands that step back to you.

## Set the rules for a project

A project can tighten these rules in `.openagents/approvals.json` at its
root. Each rule is `allow`, `ask`, or `deny`:

```json
{"schema": "openagents.approval-policy.v1",
 "rules": {"pr.merge": "deny", "deploy.staging": "ask"}}
```

The rules are `deploy.staging`, `deploy.production`, `pr.review`,
`pr.merge`, `dns`, `secrets`, and `payments`. A file can make merging run
freely or refuse it, but it can never lower a production deploy, a
secret, a payment, or a DNS change below asking you, because an agent
could write that file.

## Your own checks

You can add your own checks that run before each step, in the same format
as Claude Code's hooks: put them in `.openagents/hooks.json` in a project,
or in `~/.openagents/hooks.json` for every project. A check that exits
with code 2 blocks the step. Your checks add to the list above; they
can't open what it holds.

## Where Coder makes changes

- **In the terminal app**, a chat works directly in the folder you opened.
  Use Git as usual, and `/rewind` to put back the files a turn changed.
- **Background agents** (`/agent`), **Solver**, and **chats run from
  scripts** (`openagents coder chat`) each work in their own copy of the
  project: a Git worktree on its own branch. The folder they started from
  is read-only for them; an edit there, or a command that writes there,
  is refused.
- **Everywhere**, commands that throw away uncommitted work are refused,
  such as `git stash`, `git reset --hard`, `git checkout -- .`,
  `git restore`, and `git clean -f`. Coder points to a fresh worktree
  instead.

## Your keys and passwords

- Keys you give Coder are kept in files only you can read, under
  `~/.openagents/coder/`, and are never shown on screen.
- Commands Coder runs in the background have keys taken out of their
  environment and out of everything the chat reads.
- Messages that look like they hold a password or key are left out when
  chats sync to your account, and memory notes refuse them.

See also [Privacy and security](/docs/privacy-and-security).

Next: [Sync, share, and export](/docs/coder-sync).
