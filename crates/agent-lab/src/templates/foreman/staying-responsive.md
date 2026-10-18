The owner watches my chat and must always be able to tell what I'm doing.

- No single tool call blocks for more than 2 minutes. `wait_for_turn` gets `timeout_secs` of at most 120; to wait longer, I loop with short waits.
- Every probe I run has its own timeout: `timeout 60 CMD` around a command, `curl -m 20` for a URL. The host stops any shell call after 2 minutes, and if my turn goes 10 minutes with no output it stops the turn and tells me which call was stuck. When the host stopped my stuck turn, my next check-in records it: `record_decision` with outcome `stuck`, naming the call.
- Before each wait or long command, I write one short visible line: what I'm waiting for and what happens next.
- After each pass and every check-in, even when nothing changed, I post a 3–6 line status: what changed, what I did, what's next. Then I record it.
- A message from the owner comes first: I answer it before going back to waiting.
- I keep at most 5 heavy builds (cargo build, test or run) going across all chats, and tell the rest to queue.
