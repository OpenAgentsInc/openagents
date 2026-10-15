# Queue messages while Coder works

You don't have to wait for Coder to finish before you type. Each message
can either go now, changing what Coder is doing, or wait in a queue and go
when the current reply ends. This guide shows both, in the terminal and on
the website.

## The two keys

| Key | Does |
| --- | --- |
| `Enter` | Send now, even while a reply is being written. |
| `Ctrl+Enter` | Queue for later: the message waits and goes when the reply ends. |
| `Shift+Enter` | A new line in your message. |

The input's bottom edge says it short: "Enter to send, Ctrl+Enter to queue
for later".

## In the terminal

1. Give Coder a task that takes a while, such as "run the whole test suite
   and fix what fails".
2. While it works, type a follow-up, such as "then update the changelog",
   and press `Ctrl+Enter`. The message waits under the input.
3. Queue more if you like. They run in the order you queued them, one
   after each reply.

To change your mind, press `Esc` while messages are queued: they go back
into the input, where you can edit or delete them. With an empty input,
`Enter` sends the next queued message now.

**When to use `Enter` instead.** Press `Enter` when the current work
should change course now. Claude Code and Codex read your message during
the turn they're on; on Coder's own loop and OpenCode, the reply stops and
your message goes next.

**If `Ctrl+Enter` sends at once.** Many terminals send `Ctrl+Enter` as a
plain `Enter`. There, press `Ctrl+Q` (or `Ctrl+J`) to queue; the hint
under the input names the key that works. Kitty, Ghostty, foot, and
Windows Terminal tell `Ctrl+Enter` apart.

## On the website

The [web chat](/new) takes the same keys, with `⌘+Enter` on a Mac.

- Queued messages show above the [message box](/new). Click one to change
  its words, or press **Remove** to drop it.
- `Enter` (or **Send**) while a reply is being written stops that reply
  and sends your message now. What the reply wrote so far stays, marked
  "You stopped this answer."

## In the apps

The Mac and iPhone apps queue too; how is in
[Watch, steer, and stop Coder](/docs/following-coder).

Next: [Share a project with a teammate](/docs/share-a-project).
