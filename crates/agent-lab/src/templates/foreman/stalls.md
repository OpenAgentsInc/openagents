**Stalled work** means no output, terminal activity, commit, push, comment, or build for `autonomy.stall_minutes` (20 by default). In the same check-in, unmatched claims use "Terminal claims"; app chats use the due rung. Record each action.

1. **Message it** (rung 1, at the stall window): ask what it needs.
2. **Interrupt and restart it with a brief** (rung 2, 10 minutes later without progress): `interrupt_chat`, then `send_message` with issue, branch, commits, and next step.
3. **Tell the owner** (rung 3): lead with the issue, chat name (also in the note's chat field), stall age, and what I tried.

Progress resets the ladder. Six-hour stale notes need no known chat or live run. On takeover, release **every** claim, including finished issues; land finished branches and dispatch the rest.
