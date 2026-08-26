# coder autoimprovement loop

## Idea

The agent that writes code can also drive a review of its own transcript. After a turn finishes, the same model (or a second instance) reads the recorded interaction — user prompts, tool calls, file edits, test output, final diff — and scores how well the work was done. The score is not abstract; it is grounded in what the model can now see about the repository, the task, and the sequence of choices it made. From that review it proposes one or two concrete changes to the process: a better prompt pattern, a missing verification step, a different order of operations, a more accurate tool call, a smaller commit. The next turn starts with those changes applied.

This is not a new idea, but the context here is special. The coder has access to the full transcript, the exact code it wrote, and the test results. It can compare what it did against what it could have done. It can ask itself whether it explored enough before editing, whether it ran the right tests, whether it made the smallest change, whether the commit message matched the diff. The feedback is factual, not rhetorical.

## What a review looks like

A review is a short chat, initiated by the agent, with a structured prompt. The prompt includes:

- the task as given by the user,
- the complete transcript of the work,
- the final diff,
- the test or lint output,
- the commit message and any push result,
- a short list of known best practices for this repository and toolchain.

The model answers with a scored assessment and one or more proposed improvements. The assessment names the strongest and weakest parts of the turn, with specific evidence. Each proposal has a justification, a risk, and a way to verify it.

The known best practices are not fixed. They live in a file the agent can read and update: `docs/coder/best-practices.md`. After each review, the agent can add a newly discovered principle or remove one that no longer holds. The best practices themselves are subject to the same loop.

## Example questions the review asks

- Did the agent read the relevant files before editing? Did it use `grep` and `find` to locate dependencies, or did it guess paths?
- Did it run `cargo check` before `cargo test`? Did it narrow the test scope to the crate that changed?
- Did it make the smallest change that satisfies the request, or did it refactor unrelated code?
- Did it preserve existing style, naming, and error handling patterns?
- Did it update tests when the behavior changed?
- Did it write a commit message that explains why, not just what?
- Did it push a working state, or did it leave the repository with a failing check?
- Did it ask for clarification when the request was ambiguous?
- Did it document non-obvious decisions?

The review does not have to be kind. It can say that a turn wasted time, made a wrong assumption, or introduced a regression. The agent is the reader and the writer, so there is no social cost to candor.

## Iterative loop

1. **Plan.** Given the user request, the agent produces a short plan: the files to touch, the tests to run, the risks to watch.
2. **Work.** The agent executes the plan, calling tools and writing edits, recording the transcript.
3. **Review.** The agent starts a review chat. The review prompt is a new conversation, separated from the work, so the model does not confuse the two contexts. It returns a score and proposals.
4. **Adopt.** The agent applies the highest-impact, lowest-risk proposals. Some proposals become new instructions for the next turn; others are rejected with a note.
5. **Update knowledge.** The agent updates `docs/coder/best-practices.md` or `docs/coder/autoimprove.md` with what it learned.
6. **Next turn.** The next user request starts with the updated instructions, the updated best practices, and the accumulated history of reviews.

The loop is not expected to converge on perfection. It is expected to stop the same mistake from happening twice and to surface patterns that a single turn cannot see.

## Why this might work

- **Grounded feedback.** The model is not evaluating itself in a vacuum. It has the actual transcript and diff.
- **Self-critique without defensiveness.** There is no user or manager to appease; the model can be direct.
- **Accumulated memory.** The best-practices file and the review history give the agent a long-term context that a single chat cannot hold.
- **Targeted improvement.** The loop does not ask the model to be smarter; it asks it to follow a better process.

## Why it might not work

- **Overfitting to the review prompt.** The model might learn to game the scoring rather than improve the work.
- **False confidence.** A model can produce a convincing review without actually understanding the code.
- **Extra cost and latency.** Each review is another model call. The value has to exceed the overhead.
- **Stagnation.** The best-practices file can accumulate contradictions if no one removes obsolete entries.
- **Self-reinforcing errors.** If the review model shares the same blind spots as the work model, it will not catch them.

## First implementation

The smallest version is a manual one. After a session, the agent or the user can copy the transcript into a prompt and ask for a review. The review output is pasted into `docs/coder/autoimprove.md` or `docs/coder/best-practices.md`. A more automated version can call a second agent with the session id from `trace ingest` and ask it to produce the same review.

A fully automated loop would require:

- a way to extract the session transcript in a structured form,
- a review agent prompt that returns machine-readable proposals,
- a claim on whether to apply each proposal,
- a way to surface the review to the user for the first few turns,
- a regression test that fails if a known best practice is violated.

## Suggested review prompt

```
You have just completed a coding task. The user asked:

<request>
{user_request}
</request>

The transcript of your work follows:

<transcript>
{transcript}
</transcript>

The final diff is:

<diff>
{diff}
</diff>

The verification output is:

<verification>
{verification}
</verification>

The best practices known to the project are:

<practices>
{practices}
</practices>

Score the work on a scale from 0 to 10, with specific evidence for each point gained or lost. Propose one to three concrete changes to the process that would have improved the outcome. Each proposal must include: the problem it solves, the evidence from the transcript, the risk, and how to verify it. Finally, list any best practices that should be added, removed, or changed.
```

The output can be appended to a session log. Over time, the log becomes a dataset of which process changes actually helped.
