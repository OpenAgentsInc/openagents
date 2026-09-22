# Terminal-Bench tasks that resemble recent Coder work

Date: 2026-09-22. This is a task-selection study, not a benchmark run.

The closest matches are **`vllm-deepseek-streaming`, `headless-terminal`,
`fix-git`, `cancel-async-tasks`, `batched-eval-parity`, and
`math-eval-grader`**. They exercise problems that recur in the recent chats:
broken tool-call streams, terminal behavior, stranded changes, concurrent
workers, and evaluation results that need independent verification.

The old twelve-task cross-section misses most of these. Its strongest
matches are `fix-code-vulnerability` and `build-cython-ext`. Keep the
observed Coder delegation golden as a separate measure of the actual
workflow; a Terminal-Bench task does not test the whole issue-to-merge loop.

## Scope and sources

Cloned [harbor-framework/terminal-bench][upstream] and inspected commit
`3b5caaa4863d64dda7f0957bf4fc2d4f019202d4`, dated 2026-09-21. The survey
covers **156 task directories: 66 under `tasks/` and 90 under `archive/`**.
It excludes checker fixtures and task templates elsewhere in the repository.
The task names and instruction summaries were screened across both
directories; the shortlisted instructions were read in full. Every upstream
task link below pins that commit.

**Current** below means a directory under `tasks/` at this commit.
**Archived** means a directory under `archive/`. Neither label establishes
membership in a particular published Harbor dataset release. In particular,
this checkout is not the older `terminal-bench@2.0` pin used by Coder.

The local comparison window is **2026-09-08 00:00 through 2026-09-22 00:15,
America/Chicago**, ending before this research request. Sources:

| App | Local history inspected | Records used |
| --- | --- | --- |
| Grok | `~/.grok/sessions/*/*/updates.jsonl`, with session summaries | Timestamped `user_message_chunk` records; retained examples in this window begin on September 11. |
| Devin | `~/.local/share/devin/cli/sessions.db` | `prompt_history` joined to `sessions`; retained examples begin on September 10. This is the CLI history, not `/usr/local/share`. |
| Claude | `~/.claude/history.jsonl`, with the project transcript layout checked | Typed prompt history through September 19. Pasted-text placeholders alone do not establish what the omitted text said. |
| Codex | `~/.codex/thread_history_1.sqlite` and `~/.codex/state_5.sqlite`, with rollout locations checked | Projected user messages joined to task metadata, including recent desktop conversations through September 21. |

The examples below are a qualitative sample of substantive requests, not
an estimate of the percentage of time spent on each activity. These stores
also contain delegated jobs, repeated prompts, synthetic messages, and
automation. Those records do not count as independent human preferences;
the evidence table uses identifiable direct requests. Assistant output and
tool logs are not treated as user demand. Recent files can contain older
conversations, so the window uses message timestamps rather than file
modification dates.

The report retains paraphrases and local record locators. Raw conversations,
credentials, private implementation code, and private prompts are not
included. The sibling Coder repository was read as reference material.

## Where the goldens are

The current OpenAgents episode golden is
[`crates/coderbench/goldens/devin-fan-out-six.atif.jsonl`](../../../crates/coderbench/goldens/devin-fan-out-six.atif.jsonl).
Its supporting files are:

- [Task manifest](../../../crates/coderbench/tasks/devin-fan-out-six/task.json):
  the pinned repository, six independent questions, expected answers,
  execution path, and grading requirements.
- [Metadata](../../../crates/coderbench/goldens/devin-fan-out-six.meta.json):
  `observed`, orchestrator `coder`, recorded on 2026-09-20 at repository
  commit `34df6bc026aa68947979f172614a2ae533e4ed77`.
- [Live grade](../../../crates/coderbench/goldens/devin-fan-out-six.grade.txt)
  and [evidence](../../../crates/coderbench/goldens/devin-fan-out-six.evidence.json):
  six verified answers, an unchanged workspace, and successful completion.
- [Measurement](2026-09-20-observed-fanout.md) and
  [CoderBench guide](../../coderbench.md): what the golden establishes and
  what an offline trace comparison cannot establish.

There is one episode golden in that directory at inspection time. Its trace
SHA-256 is
`be816833193205cb511af43326f8aa7972d94aaf81ce6c1708566edb8d23c0db`.
It proves a bounded read-only fan-out, not successful implementation and
integration of six issues.

The sibling `~/work/coder/bench/golden/` is a different collection: tool and
runtime contract corpora used by crate tests. Its `bench/tasks/` also holds
repository-derived regression tasks. The Terminal-Bench selection lives in
`~/work/coder/crates/coder-bench/bench/suites/tb2-cross-section.suite.json`,
with rationale in the adjacent `tb2-cross-section.md`. That manifest pins
`laude-institute/terminal-bench-2` at
`69671fbaac6d67a7ef0dfec016cc38a64ef7a77c`.
The sibling checkout inspected was `559b59983f894b9934b9dfff4519a15be283e72d`.

## Recent requests behind the ranking

Dates and times in this table use America/Chicago. The locators identify
local evidence; they are not public transcript links.

| Evidence | App and local locator | Paraphrased request |
| --- | --- | --- |
| E01 | Grok, September 15–16, session `01a0a805…`, `updates.jsonl` lines 222, 1086, 2828, 2997, and 3116 | Build interactive terminal panes, fix typing and scrolling delays, restore streaming updates, and make keyboard shortcuts work. |
| E02 | Devin, September 18, `prompt_history` IDs 697, 702, and 705 | Diagnose repeated `MALFORMED_FUNCTION_CALL` failures during ordinary repository questions and fix the agent's stream handling. |
| E03 | Claude, September 17, `history.jsonl` line 8041; Devin, September 21, `prompt_history` IDs 870–871 | Explain dirty main and outstanding worktrees, recover stranded work, and finish integration without losing changes. |
| E04 | Claude, September 10, `history.jsonl` lines 7645 and 7657 | Run several Devin workers through Coder, keep resource use light, and wind down stale or stuck workers. |
| E05 | Codex, September 20 at 22:23:06, task `01a0c10c…` | Refill available worker capacity from unblocked issues, separate coding from CPU-contended gates, and make scheduling part of the harness. |
| E06 | Codex, September 19 at 21:56:17 and September 20 at 05:47:17, task `01a0bca1…` | Act on grader and refusal-accounting audit findings; retain a real observed golden and fix fixtures without weakening the grader. |
| E07 | Claude, September 19, `history.jsonl` line 8172; Codex, September 20 at 13:10:44, task `01a0c002…` | Investigate calibration and evaluate changes to Kev models, serving, cached execution, and out-of-domain behavior. |
| E08 | Grok, September 17, session `01a0afa5…`, `updates.jsonl` lines 623 and 884 | Build a useful repository map and demonstrate embedding-based cosine retrieval and a decision-model alternative. |
| E09 | Grok, September 12, session `01a09867…`, `updates.jsonl` lines 1 and 303 | Compare existing agent tooling and QA behavior with a reference implementation and inspect the installed CLI. |
| E10 | Devin, September 17, `prompt_history` IDs 643–644; Grok, September 16, session `01a0a805…`, line 3916 | Get the rebuilt development terminal running and diagnose why the wrong or unchanged version appears. |

Additional direct requests establish the coverage gaps: Claude's September
19 glossary and ATIF planning requests (`history.jsonl` lines 8193 and
8200); Codex's September 21 plugin-document consolidation request
(`userMessage` ID `01a0c470-d9ab-7023-9558-e06d5b1aa79b`); Devin's September
21 Minecraft agent-idling and coordination reports (`prompt_history` IDs
940 and 949); and Codex's September 21 video-trimming and transcription
requests (`userMessage` IDs `01a0c538-2ec8-7002-a14f-9402ec0c948b` and
`01a0c53c-6139-7b50-8c04-41b12becd71b`).

## Ranked matches

Rank combines closeness of the reported problem, similarity of the work
needed to solve it, and recurrence across the sampled requests. It does not
measure difficulty, expected pass rate, or execution cost. Adjacent ranks
are judgment calls. A matching task can use a different implementation
language; this report adds no product implementation or benchmark adapter.

| Rank | Task and location | Why it resembles the chats | Limit of the match |
| --- | --- | --- | --- |
| 1 | [vllm-deepseek-streaming][vllm-deepseek-streaming] — current | Repair corrupted streamed responses and tool-call JSON. E02 is almost the same operator complaint. | The actual failures involved a different provider and runtime. |
| 2 | [headless-terminal][headless-terminal] — archived | Implement an interactive shell interface with key input, control characters, startup files, and interactive programs. Strong terminal and programmatic-control overlap with E01 and E04. | Does not grade GPU rendering, font quality, pane layout, or perceived latency. |
| 3 | [fix-git][fix-git] — archived | Find changes that disappeared after switching branches and merge them back. Closest direct match to E03. | Smaller than reconciling multiple live workers and their worktrees. |
| 4 | [cancel-async-tasks][cancel-async-tasks] — archived | Bound concurrent jobs and preserve cleanup after keyboard interruption. Closely matches worker lifecycle and cancellation concerns in E04–E05. | Python async jobs are a smaller problem than subprocess groups, worktrees, and resource admission. |
| 5 | [batched-eval-parity][batched-eval-parity] — current | Repair evaluation while preserving per-example meaning, ordering, scores, calibration, and packed/padded equivalence. Strong match to E06–E07. | Its generative-model evaluator differs from the typed decision API and Gym. |
| 6 | [math-eval-grader][math-eval-grader] — current | Build a grader from labeled accepted/rejected cases, preserve actual model outputs, and calculate accuracy from the evidence. Matches E06's concern about grading the right thing. | Mathematical equivalence and PDF answer extraction are task-specific. |
| 7 | [sglang-qwen-burst][sglang-qwen-burst] — current | Restore text/tool-call ordering and missing text after serving acceleration. Matches E02's broken tool flow and E07's requirement that optimization preserve behavior. | No sampled request establishes this particular speculative-decoding bug. |
| 8 | [fix-code-vulnerability][fix-code-vulnerability] — archived | Inspect a repository, identify an input-handling defect, report it, patch it, and verify the intended error behavior. Fits the audit-to-fix loop in E06. | A Bottle/CWE repair is narrower than the Rust runtime audit. The current archived instruction requires a fix as well as a report. |
| 9 | [build-cython-ext][build-cython-ext] — archived | Read build errors, repair dependency compatibility, compile, install, and confirm the real consumer works. Matches the build-and-run loop in E10. | NumPy/Cython packaging differs from Cargo and terminal development. |
| 10 | [mteb-retrieve][mteb-retrieve] — archived | Load a pinned embedding model, rank documents by cosine similarity, and return the requested result. Direct algorithmic overlap with E08. | Does not build a repository map or validate code understanding. |
| 11 | [rs-archive-clone][rs-archive-clone] — current | Probe a reference tool and reproduce behavior, including errors, exit codes, file modes, and side effects. Matches the compatibility investigation in E09. | Reed-Solomon recovery is a substantial unrelated component. The task requires black-box cleanroom work and forbids disassembly. |
| 12 | [llm-inference-batching-scheduler][llm-inference-batching-scheduler] — archived | Plan constrained batches, include each request once, and compare cost and latency with a baseline. Connects E05's scheduling work with E07's inference work. | Static tensor-shape packing is not a live issue scheduler; this is a structural analogy. |
| 13 | [embedding-drift-monitor][embedding-drift-monitor] — current | Repair statistical utilities and alert behavior when false alarms and missed changes invalidate the result. Relevant to E06–E07. | KS/PSI/MMD drift tests are not the same as decision calibration or benchmark regression. |
| 14 | [hf-model-inference][hf-model-inference] — archived | Run a local model behind an HTTP endpoint with structured confidence output and error handling. Fits the local decision-model serving work in E07. | A sentiment endpoint omits tenancy, quotas, receipts, and the question contract. |
| 15 | [wal-recovery-ordering][wal-recovery-ordering] — current | Repair durable ordering, concurrent acknowledgment, deterministic replay, and independent snapshots. Useful for the state and evidence integrity concerns behind E06. | The direct chat evidence is broader audit work, not a request to implement a WAL engine. |
| 16 | [pytorch-model-cli][pytorch-model-cli] — archived | Deliver a usable local inference executable, weights, and an exact output contract. Related to E07 and E10. | MNIST inference is much smaller than serving Kev or integrating Lev. |

For an initial **eight-task relevance set**, use ranks **1–6, 8, and 9**:
stream repair, terminal control, Git recovery, cancellation, evaluation
parity, grader correctness, repository repair, and build compatibility.
For **current-directory tasks only**, start with
`vllm-deepseek-streaming`, `batched-eval-parity`, `math-eval-grader`, and
`sglang-qwen-burst`; then add the more specialized matches in ranks 11, 13,
and 15 if those work areas are the target.

These are proposed selections. No suite, task pin, development split, or
held-out designation changed. Reading task instructions to select this set
does not create fresh held-out evidence for subsequent prompt tuning.

## How the old twelve-task selection compares

The old selection deliberately covered tool-efficiency regimes and distinct
skill families. That is a different objective from resemblance to recent
requests. The earlier [Gym audit](../../gym/terminal-bench.md) explains its
measurement role; its historical runtime observations are not revalidated
here.

| Existing task | Fit to the recent sample |
| --- | --- |
| `fix-code-vulnerability` | Strong: inspect, diagnose, repair, and verify. |
| `build-cython-ext` | Strong workflow match: compile-fix-install-use. |
| `nginx-request-logging` | Secondary: configure a service and inspect real behavior; less direct than model-serving and stream repair. |
| `count-dataset-tokens` | Secondary: dataset/tokenizer plumbing, but misses evaluation semantics and calibration. |
| `merge-diff-arc-agi-task` | Partial: integration mechanics match; the ARC puzzle is unrelated. Prefer `fix-git` for stranded work. |
| `sqlite-with-gcov` | Partial: build and instrumentation workflow, without the recurring product-debugging loop. |
| `git-leak-recovery` | Partial: Git archaeology overlaps, but secret recovery and history scrubbing are a different objective. |
| `sanitize-git-repo` | Partial: a security maintenance task, with no repeated history-sanitization request established in this sample. |
| `regex-log` | Narrow: a useful deterministic text-processing probe, not a representative Coder episode. |
| `openssl-selfsigned-cert` | Narrow: an exact sysadmin checklist, without a strong recurring certificate request in the sample. |
| `schemelike-metacircular-eval` | Weak subject match: sustained implementation work, but little evidence of interpreter construction. |
| `password-recovery` | Weak: disk-image forensics is not the Git/worktree recovery the recent chats ask for. |

## Other plausible matches and important gaps

- [nextjs-performance][nextjs-performance] resembles requests to fix
  noticeable interaction delays while preserving behavior. The web stack
  and warehouse workflows differ from the terminal work in E01.
- [payments-pipeline-fix][payments-pipeline-fix] resembles worker-startup
  and restart debugging. [session-window-debug][session-window-debug]
  resembles event-state debugging, but its sessions are single-threaded
  event-time windows, not agent conversations.
- [pretrain-shard-corruption][pretrain-shard-corruption] and
  [pytorch-model-recovery][pytorch-model-recovery] become stronger choices
  for a training-focused slice. They should not displace the runtime tasks
  merely because the chats mention models.
- `git-multibranch` is deployment via Git hooks, not a worktree-recovery
  task. `constraints-scheduling` is meeting scheduling, not worker
  scheduling. `polyglot-rust-c` is a dual-language puzzle, not ordinary
  Rust engineering. Task names alone would produce misleading matches.
- Documentation research, protocol design, glossary maintenance, and
  scoped docs-only commits recur across all four sources. The surveyed
  tasks provide little direct coverage. `large-scale-text-editing` is
  specifically a constrained Vim-macro transformation, not a substitute
  for that work.
- Minecraft agent behavior, in-world coordination, terminal visual polish,
  and video editing/transcription also appear in the sample. CoreWars,
  graphics puzzles, music transcription, and hurdle-video analysis share
  surface vocabulary but have different acceptance criteria.

The largest missing task is the recurring **read the backlog, claim an
unblocked issue, create an isolated worktree, implement, verify, integrate,
and continue** episode. Extend CoderBench with observed episodes for that
workflow, including steering, cancellation, recovery, and scoped
documentation changes. Use the Terminal-Bench shortlist to test components
of the work and retain the Coder golden to test orchestration.

## Verification and limits

This change only records the comparison. It does not run benchmark agents,
containers, oracle solutions, training, or the Rust gate. Local artifact
links, the golden's recorded digest, upstream task paths at the cloned
commit, and the documentation diff were checked. No pass-rate, runtime,
cost, or current harness-runnability claim follows from this study.

[upstream]: https://github.com/harbor-framework/terminal-bench/tree/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4
[batched-eval-parity]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/batched-eval-parity/instruction.md
[build-cython-ext]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/build-cython-ext/instruction.md
[cancel-async-tasks]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/cancel-async-tasks/instruction.md
[embedding-drift-monitor]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/embedding-drift-monitor/instruction.md
[fix-code-vulnerability]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/fix-code-vulnerability/instruction.md
[fix-git]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/fix-git/instruction.md
[headless-terminal]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/headless-terminal/instruction.md
[hf-model-inference]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/hf-model-inference/instruction.md
[llm-inference-batching-scheduler]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/llm-inference-batching-scheduler/instruction.md
[math-eval-grader]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/math-eval-grader/instruction.md
[mteb-retrieve]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/mteb-retrieve/instruction.md
[nextjs-performance]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/nextjs-performance/instruction.md
[payments-pipeline-fix]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/payments-pipeline-fix/instruction.md
[pretrain-shard-corruption]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/pretrain-shard-corruption/instruction.md
[pytorch-model-cli]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/pytorch-model-cli/instruction.md
[pytorch-model-recovery]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/pytorch-model-recovery/instruction.md
[rs-archive-clone]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/rs-archive-clone/instruction.md
[session-window-debug]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/session-window-debug/instruction.md
[sglang-qwen-burst]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/sglang-qwen-burst/instruction.md
[vllm-deepseek-streaming]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/vllm-deepseek-streaming/instruction.md
[wal-recovery-ordering]: https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/tasks/wal-recovery-ordering/instruction.md
