# Matched Opus controller experiment

This development experiment compares direct Claude Code with the same executor
inside Coder One. The [protocol](protocol.json) fixes the tasks, order, controls,
metrics, and stopping rule before inference. The runner hashes the protocol,
policy, system prompt, and its own source before starting a trial, and refuses
to resume with different bytes.

Run from `bench/terminal-bench` on the Linux benchmark host:

```sh
python -m tbench.matched --artifact /path/to/the/pinned/coder-one
python -m tbench.matched --artifact /path/to/the/pinned/coder-one --run
```

The first command materializes all 12 jobs without inference. The second runs
one trial at a time, using at most six host-wide Claude slots and requiring
55 GiB of free disk before admission. Existing trials keep running. The runner
uses the existing Claude login without refreshing it. Credentials remain in
memory and go through Harbor's environment redaction.

Both arms use Claude Code 2.1.280, Opus 5.5, medium effort, six tools, the same
[system prompt](system-prompt.md), five-minute prompt caching, bypass permissions
inside the task container, and the task's original resource and verifier
settings. Both have a 1,800-second Harbor agent timeout and a 1,680-second agent
work allowance. Setup and grading are measured separately. Neither arm has an
explicit token or dollar cap.

The [Coder policy](coder-policy.json) derives from tunable v2. It fixes the
executor to Opus medium, removes routing and cross-model handoff, and retains
preparation, coverage packing, monitoring, requirement checks, support judgments,
and repair after a failed check. It uses the original long-task shell-command
ceiling at the shortened experiment deadline. Its primary dispatch receives
75% of the remaining allowance, reserving time for checks and a possible repair.
The plain agent can allocate its whole allowance to its single session. This
internal allocation is part of the controller treatment; the outer allowance
is equal. This does not test Luna routing or unmodified v2 at an eight-hour
deadline.

The plain adapter shares Coder's pinned installation and read-only doctor to
control setup differences, but it invokes Claude directly and never starts a
Coder episode. Its input is the task instruction. Coder receives that same
instruction and supplies its generated briefing to Claude. Its extra inference
and checks count toward its total cost and agent time.

Three fresh repetitions on each of two previously inspected tasks are a small,
selected development comparison. They do not estimate all 66 TB4 tasks. Count
all started attempts; do not discard failures or pick the cheapest successful
trial. Compare pass rates first, then aggregate cost and time over the fixed
schedule. Subscription usage prices are model-usage valuations, not incremental
cash charges. Unknown charges make a total a lower bound.

The result report is linked from the
[Terminal-Bench index](../../../../docs/terminal-bench/README.md).
