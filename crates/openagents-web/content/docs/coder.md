# Coder

Coder is our coding agent. You ask for coding work in a chat, and Coder
does it on your own computer, in your own project, with a coding agent
you're already signed in to there, such as Codex, Claude Code, or Grok
Build. It uses that computer's own git and GitHub sign-in.

## What you need

- **A computer.** A Mac with OpenAgents for Mac, or any macOS, Linux, or
  Windows computer with OpenAgents Terminal. We don't offer a hosted
  computer yet.
- **A coding agent signed in on it.** See [Coding agents](/docs/coding-agents).
- **A project.** A Git checkout with at least one commit.

## Run Coder on the computer you're at

1. Open a chat in the Mac app, or run `openagents` in your project's
   folder.
2. Ask for the work: "add a unit test for slugify that covers an empty
   string".
3. The reply says Coder is starting, and Coder's steps stream into the
   chat: what it reads, the commands it runs with their results, and its
   progress (`step 5 · ≈40% done`).
4. When it finishes, the chat shows a summary, the files it changed, and
   where the change is. See [Worktrees and changes](/docs/worktrees-and-changes).

In the Mac app, Coder works in the chat's project. If the chat has none, it
uses the first project you picked in **Phones and computers**, then the
last project it worked in. If none of them is a Git checkout, the chat asks
you to **Choose folder…**.

## Run Coder from your phone

1. [Connect your phone](/docs/connect-a-computer) to your Mac.
2. On the Mac, in **Phones and computers**, click **Choose folder…** to
   pick a project, and turn on **Let my phone start Coder here**.
3. On the phone, ask for the work in a chat.

If the Mac starts Coder at once (the default), Coder starts there with no
tap, and the chat shows where it runs with **Stop**. Otherwise the reply
shows **Run Coder on** your Mac; tap it. Without **Let my phone start
Coder here**, the work is recorded but doesn't start by itself.

If the computer is asleep or offline, the message waits on your phone and
goes as soon as the computer is back. It is never sent twice.

## Start at once, or ask first

By default, Coder starts as soon as the reply says a message is coding
work. To have the reply offer **Run Coder** instead, choose **Ask first**
in the Mac app's **Settings → Coder**, or run:

```sh
openagents settings set coder.start ask_first
```

The same setting decides for your phone's requests to that computer.

## Coder approves its own steps

By default, Coder doesn't stop to ask permission for each step: it runs
commands, edits files, and can push, so the work gets done end to end. It
works in its own copy of your project, never in your checkout. It can still
ask you a question when it needs one; answer in the chat. To run it inside
a stricter boundary, change `coder.access` in the
[settings](/docs/settings).

## No step or time budget

A run ends when Coder finishes, asks you something, or you stop it. If it
keeps repeating an approach that already failed without making progress,
it stops itself and says so.

Next: [Coding agents](/docs/coding-agents).
