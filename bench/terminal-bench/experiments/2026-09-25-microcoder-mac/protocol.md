# Microcoder on this Mac

This development run tests whether the current Microcoder loop can add a
Terminal-Bench 4 pass outside the task used to develop its knowledge base.
The source checkout is `417d997c68`, with no local product changes. Run
`react-lead-form` first because its single-container task and separate
verifier fit Microcoder's implemented task runner. Do not use its public
winning trajectory as a prompt, test, or knowledge entry. The local task
checkout is Terminal-Bench 4 at `452bf305c6`.

Fable 5.1 low through Claude Code passed 2 of 5 public attempts. Its two
passes took 377 and 504 seconds, with recorded costs of $2.45582575 and
$3.0954345. Their median is 440.5 seconds and $2.775630125. The five
public trial IDs, including failures, are retained in the public replay
manifest under `~/.openagents/terminal-bench/public-replays/manifest.json`.

Use GPT-6 Luna at medium effort, the admitted knowledge base, Jev, and
Microcoder's default acceptance tests. Limit the first run to 15 minutes,
$0.50 in reported model/Jev/embedding spend, and 50 steps. Use the local
Colima `fire` Docker context without changing the global context. Run the
task's reference grading check before model inference. Record the run ID,
checkout and binary digests, task and grader outcomes, complete traces,
frozen tests, retrieved knowledge entries, cost components, and both agent
and whole-trial time. Keep setup failures and unknown charges. A passing
first run is a lead to repeat, not a measured pass-rate comparison.

This is a development comparison on different hardware and runtimes.
Microcoder grades with the task's tests directly, and its Docker network
is not Harbor's allowlist. Its local wall time cannot establish a
hardware-matched speed improvement over the public Fable attempts. It can
identify a cheaper and potentially faster configuration for controlled
follow-up.

## First-run result

The first attempt passed the task verifier (reward 1). It ended by the model
finishing after 36 steps and 425.6 seconds of recorded agent time. The
reported cost was $0.1104: $0.0929 for GPT-6 Luna, $0.0166 for Jev, and
$0.0009 for embeddings. Five knowledge entries appeared in prompts; the run
is knowledge-assisted. Its three frozen acceptance scripts passed at the end,
and the independent task verifier reported `PASS - all checks passed`.

The retained record is
`~/.openagents/microcoder/runs/react-lead-form-1790398585/`:
`summary.json`, `events.jsonl`, and the saved output artifacts. The local
binary was
`/Users/christopherdavid/.codex/worktrees/microcoder-mac/target/debug/microcoder`
at SHA-256
`5bbce96f95ec09572f797bae5a72a173d9d4f0dc0b664501b2ab77fbfabb9d38`.
The source checkout remained at `417d997c68`. The local Vite build passed
but warned that Node filesystem imports were externalized for browser
compatibility. The task verifier passed despite that warning; a browser
interaction test would be needed to assess the user-facing path independently.

One pass does not estimate a pass rate. Its recorded agent time is 14.9
seconds below the two successful public Fable 5.1 low attempts' median,
but the machines, setup, and runner restrictions differ. The next test
must hold the runner and configuration fixed, repeat the task with the
knowledge base on and off, and retain every failed attempt before making
a speed, cost-per-pass, or knowledge-contribution claim.
