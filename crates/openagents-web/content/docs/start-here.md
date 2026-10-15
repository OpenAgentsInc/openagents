# Start here

Your first half hour with Coder, step by step: install it, log in, give it
a first task in one of your repositories, read what it did, and answer a
step that waits for you. Every command is here to copy.

You need a Mac, Linux, or Windows computer, a terminal, and a project in a
Git repository. A coding agent you already use, such as Claude Code or
Codex, helps but isn't required.

## 1. Install Coder

On macOS or Linux, in a terminal:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

Open a new terminal, so it finds the new commands, and check:

```sh
coder --version
```

It prints `coder` and the version. If it says the command isn't found,
see [Troubleshooting](/docs/troubleshooting). More on what gets installed
and how updates work: [Download and install](/docs/download).

## 2. Log in

```sh
coder login
```

Your browser opens [openagents.com/device](https://openagents.com/device)
with an eight-letter code, such as `BCDF-GHJK`. Check that it's the same
code your terminal shows, then approve. The code works for 10 minutes;
run `coder login` again if it runs out.

Coder then asks once where this computer's chats should live:

1. **Sync all my chats**: they show on openagents.com and on your phone.
2. **Keep chats on this computer**: nothing is sent.

Type `1` or `2`. You can change it later with `/sync on` or `/sync off`.

Logging in is optional. Without it, Coder still answers through Claude
Code, Codex, or OpenCode if one of them is set up on this computer.
Logged in, it can also use our models, with a free daily allowance (see
[Pricing](/docs/pricing)).

## 3. Open your project

```sh
cd ~/code/my-project
coder
```

The screen is the chat, with the input at the bottom. At the input's top
right is who answers: **Auto** until the first reply, then the agent and
model, such as `Auto → Claude (Haiku 5.5)`. At its bottom right are your
folder and Git branch.

## 4. Give it a first task

Start with a question, so you can see how it reads your code. Type this
and press `Enter`:

```text
What does this project do? Name the three files I should read first.
```

Then a small change:

```text
Add a short "Getting started" section to the README. Show me the diff.
```

Coder reads, edits, and runs commands in this folder on its own. You
don't need to approve each edit.

## 5. Read what it did

Each step Coder takes shows as one line in the chat: files read,
searches, each edit with its diff, and each command with its output.

- Press `Ctrl+O` to open every step in full, and again to fold them back.
- Click one of Coder's messages to see the model that wrote it, such as
  `Claude (Haiku 5.5)`, for about three seconds.
- Use `PageUp` and `PageDown`, or the mouse wheel, to scroll; `Ctrl+End`
  jumps to the bottom.
- Run `git diff` in another terminal to see every change together.

Don't like the change? Type `/rewind`, pick your message, and Coder puts
the files back as they were before it.

## 6. Answer a step that waits for you

A short list of steps that are hard to undo always waits for your yes or
no: a production deploy, merging a pull request, reading or changing a
secret, a payment, and a DNS change. To see one safely, make a file that
looks like a secret, with nothing real in it:

```sh
echo "DEMO_KEY=not-a-real-key" > .env
```

Then ask Coder:

```text
Run cat .env and tell me what it prints.
```

Coder stops and asks you one plain question, with the exact command.

| Key | Does |
| --- | --- |
| `Y` | Allow it, this one time. |
| `N` or `Esc` | Say no. |
| `PageUp`, `PageDown` | Read the whole command. |

Press `N`. The command doesn't run, and Coder carries on with the rest of
the work without it. Delete the practice file when you're done:
`rm .env`. Everything on the list, and how a project can tighten it, is
in [Safety](/docs/coder-safety).

## 7. Keep going

- `Ctrl+N` starts a new chat. `/resume` opens an earlier one.
- `Shift+Tab`, before a chat's first message, picks who answers.
- `/` lists every command; `Ctrl+C` quits, and your chat is saved.

Next, try a task-based guide:
[Fix a failing test](/docs/fix-a-failing-test),
[Use your Claude Code or Codex](/docs/use-claude-code-or-codex), or the
[keyboard shortcuts](/docs/keyboard-shortcuts).

Next: [Download and install](/docs/download).
