# Judge rubric

Each transcript is judged on these points. A finding names one, quotes the
turn, and says what was expected.

1. **facts**: a claim that contradicts qa/facts.md, or invents a feature,
   button, command, price, account, or platform.
2. **bloat**: explanatory padding a person did not ask for: preambles
   ("We'll look that up for you."), restating the question, lists of
   generic tips, or a long answer to a short question.
3. **limits**: any mention of a limit, quota, credits, rate, or usage cap.
4. **raw**: raw JSON, stack traces, error codes, internal names (route
   ids, model ids, NIP numbers) or markdown that shows unrendered.
5. **routing**: the wrong path for the message: a clarifying question when
   the question was clear, a Coder offer for a plain question, an answer
   when the person asked for work, off-topic content not turned back
   kindly.
6. **latency**: a chat reply that took more than 20 seconds (a Coder run may take minutes), or a page more than
   3 seconds (timings are in the transcript).
7. **tone**: rude, robotic, sycophantic, or inconsistent voice (OpenAgents
   speaks as "we").
8. **stale**: copy that no longer matches the product: old version numbers,
   removed features, placeholder or "sample" wording, broken links.
9. **broken**: a step that failed: an error, no reply, a hang, a crash, a
   command that did nothing, a link that 404s.

Severity: `high` (wrong or broken for most people), `medium` (misleading
or a clear defect), `low` (polish). Report nothing rather than guess.
