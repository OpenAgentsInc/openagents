# Watch, steer, and stop Coder

A Coder run lives in its chat. You can follow it, add to it, answer it, or
stop it from the Mac app, the Terminal, the `openagents` command, or your
phone.

## Follow along

- The run's first card names the coding agent, the model, why that agent,
  and the project.
- Each step shows as a short line: reads and searches fold into one row
  ("Read 3 files, Searched 2 patterns"); each command or edit has its own
  row with its result. Click a row on the Mac, or press `Ctrl+O` in the
  Terminal, to see it in full.
- The working line shows the step, Jev's estimate of how much is done, and
  the time: "Coder is working · step 5 · ≈40% done · 9s".
- On the phone, the Coder card shows how long Coder has worked and, at the
  end, how the run ended.

Closing the app or the Terminal doesn't stop the run. Open the chat again
and it follows the run from the start. From a shell:
`openagents chat follow --thread ID`.

## Send more while it works

- **Send** while Coder works queues your message for its next turn. On the
  Mac, the queue shows each message with **Send now** and **Remove**; on
  the phone, **Edit queue** changes what's waiting.
- **Stop and send** stops the current turn and continues with your
  message.
- **Steer now** sends it before the next turn starts.

On the phone, long-press send to choose between these.

## Answer a question

When Coder asks a question, or asks to go ahead with a step, it shows as a
card in the chat. Answer by typing in the chat (on the Mac, an approval
also has **Approve** and **Deny**). Your answer is information for Coder;
it never widens what Coder may do.

## Follow up after it finishes

A message after a run finishes goes to OpenAgents with what the run did. A
question such as "what did you change?" is answered in the chat. A request
for more work, such as "now add a test", continues the same task, in the
same copy of your project.

## Stop it

- **Mac app:** **Stop Coder**.
- **Phone:** **Stop** on the run, or the stop button in its chat.
- **Terminal:** Esc or `/stop`.
- **Shell:** `openagents chat stop --thread ID`.

The run ends as stopped, and the chat says so. Its work so far stays in its
copy of your project.

Next: [Worktrees and changes](/docs/worktrees-and-changes).
