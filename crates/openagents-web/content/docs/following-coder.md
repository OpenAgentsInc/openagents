# Watch, steer, and stop Coder

A Coder run lives in its chat. You can follow it, add to it, answer it, or
stop it from the Terminal or the `openagents` command.

## Follow along

- The run's first card names the coding agent, the model, why that agent,
  and the project.
- Each step shows as a short line: reads and searches fold into one row
  ("Read 3 files, Searched 2 patterns"); each command or edit has its own
  row with its result. Press `Ctrl+O` in the Terminal to see it in
  full.
- The working line shows the step, Jev's estimate of how much is done, and
  the time: "Coder is working · step 5 · ≈40% done · 9s".

Closing the Terminal doesn't stop the run. Open the chat again
and it follows the run from the start. From a shell:
`openagents chat follow --thread ID`.

## Send more while it works

`Enter` to send, `Ctrl+Enter` to queue for later.

- **Send** (or Enter) sends now, even while Coder works: it stops the
  current turn and continues with your message, or, before a turn starts,
  steers it.
- **Queue** (or Ctrl+Enter) keeps your message for after the current turn.
- Shift+Enter adds a line.

## Answer a question

When Coder asks a question, or asks to go ahead with a step, it shows as a
card in the chat. Answer by typing in the chat. Your answer is information for Coder;
it never widens what Coder may do.

## Follow up after it finishes

A message after a run finishes goes to OpenAgents with what the run did. A
question such as "what did you change?" is answered in the chat. A request
for more work, such as "now add a test", continues the same task, in the
same copy of your project.

## Stop it

- **Terminal:** Esc or `/stop`.
- **Shell:** `openagents chat stop --thread ID`.

The run ends as stopped, and the chat says so. Its work so far stays in its
copy of your project.

Next: [Worktrees and changes](/docs/worktrees-and-changes).
