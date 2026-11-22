# Inky: avenues from other projects

Nov 22, 2026 · @Christopher David

## Summary

The finding that most changes the picture is about the model, not the harness. Thinking Machines measured Inkling 2 Mini in minimal harnesses, most of them shell-only (bash-only, mini-swe-agent and the four-tool pi). The shell-edit habit is probably trained behavior. The status doc shows no run of Inky's tasks in those harnesses.

Seven additions follow from the research. Section marks (§) refer to the Inky status doc of 2026-11-22.

- **Run a control arm.** Put pi and mini-swe-agent on the held-out tasks against the same server. It separates harness loss from model, quantization and speed limits.
- **Verify shell edits instead of refusing them.** Codex CLI intercepts edits made through the shell. A guarded edit command on `PATH` and a diff guard after each command keep Class-E receipts without fighting the model.
- **Move control to the sampler.** A silent thinking budget, forced tool choice and typed arguments change no prompt bytes. A small llama.cpp test suggests that telling a model its budget can make it worse.
- **Explore in a forked context.** Return `file:line` ranges to the main context and discard the search. The main context stays small, where decode is fastest.
- **Tighten the evals.** Five trials per task with paired comparison, grader audits done by reading failures, and a Dev pool minted from killed mutants.
- **Three engine items.** A per-request utility gate so DSpark can default on, a drafter built from past outputs, and a fidelity check of the IQ4\_NL weights against the release.
- **Change who writes skills and what the Reflector sees.** Harness search works elsewhere when the proposer reads raw traces. Model-written skills show no average benefit; curated ones do.

Most sources are vendor posts and preprints, measured on other models and tasks. Treat each item as a hypothesis for a paired Dev run.

## Editing and the shell-edit habit (F2)

The shell-edit habit is most likely the model's trained interface, so refusing it fights the model. Thinking Machines reports Inkling 2 Mini's coding scores from minimal harnesses, most of them shell-only: SWE-bench Verified 79.2% "bash only", SWE-bench Pro v2 (Hard) 77.9% in mini-swe-agent, Terminal Bench v2.1 71.6% in pi, which adds only read, write and edit tools to bash ([model card](https://thinkingmachines.ai/model-card/inkling-2-mini/)). A study of edit interfaces found the same thing in other models: they choose formats by habit, even when another format is accepted far more often ([Fit, Not Format](https://www.preprints.org/manuscript/202610.1889)).

This argues against the refusal rule in §9.2 item 2(b). Three other routes keep the Class-E guard and the Desktop diff view without pushing the model off its distribution.

| Route | Who does it | What it means for Inky |
| --- | --- | --- |
| Catch edits made through the shell | Codex CLI treats `apply_patch` as a virtual CLI. It intercepts the call inside shell tool calls and puts an `apply_patch` entry on `PATH` so scripts can reach it ([openai/codex #2646](https://github.com/openai/codex/pull/2646)). | Put a guarded edit command on the sandbox `PATH` that calls `ik_exec_edit`. A heredoc edit then gets the guard, the receipt and the diff event. |
| Verify after the command | Proposal; I found no direct precedent. | Snapshot tracked files before `run_command`, diff after, run `oa-exec-diff-verify` on the diff, emit the `edit <path>` event, revert on refusal. Every edit route gets a receipt. |
| Ask for a pointer, not quoted text | Hashline tags each line with a 2–3 character content hash and the model edits by tag ([The Harness Problem](https://stencil.so/blog/the-harness-problem)). "Verified selection" uses a line range plus the first and last line as evidence ([Fit, Not Format](https://www.preprints.org/manuscript/202610.1889)). | `edit_file` already has 0% misses, so the gain here is output tokens, not accuracy. |

The token numbers are the reason to care about the third route. Hashline cut output tokens by 17–61% for ten of 16 models and raised them for two. Its accuracy gains were large over patch and small over string replace. Verified selection solved 1.43–1.77 times as many tasks per output token as search/replace. Bare line ranges are the trap: about 20% of large edits applied and broke the file, against about 2% for search/replace.

**Large rewrites.** Cursor found that rewriting the whole file beat diffs for files under 400 lines. It made rewrites fast by using the existing file as a deterministic draft, reaching about 1,000 tok/s on a 70B model ([Cursor](https://cursor.com/blog/instant-apply)). OpenAI ships the same idea as a `prediction` request parameter ([Predicted Outputs](https://developers.openai.com/api/docs/guides/predicted-outputs)). Inky's lookup drafter already reached 65.6–79.0% acceptance in §7.3 and lost only on verify cost. Re-measure it on the Sprint 72 grouped-MoE verifier, with the target file passed as an explicit prediction on `write_file` and large edits.

**Metrics to add.** The same study recommends reporting first-edit acceptance, silent-error rate and tasks solved per output token. A rejected first edit made a run cost 1.2–1.8 times as much.

**Test.** Dev-9 in three arms: current build, guarded edit command plus post-command verification, and the refusal rule. Compare pass rate, output tokens per task and the share of edits with a Class-E receipt.

## Output tokens: thinking budgets and time awareness (F1)

The vendor documents the runaway thinking that F1 describes, and other runtimes already ship the sampler fix proposed in §9.2 item 1. The model card says the model can get stuck in long reasoning loops, especially while debugging code, and that minimum effort does not turn reasoning off. It recommends limits on output tokens and agent turns ([model card](https://thinkingmachines.ai/model-card/inkling-2-mini/)).

**Copy the llama.cpp design, minus one part.** Its reasoning-budget sampler counts tokens inside the thinking block, then forces a short stop message and moves the model to its answer. A per-request budget exists on master. A pending extension adds an intro message that tells the model its budget, plus a soft warning at a set fraction ([llama.cpp #25961](https://github.com/ggml-org/llama.cpp/pull/25961)).

The intro message is the part to skip. In a reviewer's test on 20 hard items, Gemma4-26B scored 4/20 with the budget alone and 2/20 with the intro message, with twice as many items running to the token ceiling. The reviewer's reading: a model told about a budget can treat the forced cutoff text as part of its plan and keep reasoning. So enforce silently, and log two counters per run: budget hits, and tokens of reasoning that appear after a forced stop.

**Treat effort as a fitted number.** Inkling's template accepts a numeric effort from 0.0 to 0.99 as well as named levels, and the default is high ([inkling2-mlx](https://github.com/internet-projects/inkling2-mlx)). A fixed per-session value keeps the KV prefix intact, so it fits the offline ReAnchor pass. LangChain's Terminal Bench work shows why the top setting is not the safe one: maximum reasoning scored 53.9% because of timeouts, one level lower scored 63.6% ([LangChain](https://langchain.com/blog/improving-deep-agents-with-harness-engineering)). Open question: which effort does Inky send today?

**Tell the agent how much budget is left.** All six held-out failures were timeouts, and Inky gets no signal that time is passing. LangChain injects time-budget warnings to push the agent from building to verifying, because agents estimate time badly. For Inky the warning can ride on a tool result, which is append-only and cache-safe. In the product, where there is no wall-clock limit, a step budget plays the same role.

**Check before finishing.** LangChain's most common failure was the agent re-reading its own code, judging it fine and stopping. Their fix intercepts the exit and requires a verification pass against the task. The model card flags the same weakness: the model may misreport the results of its work. This strengthens §9.2 item 3: run the check on exit as well as after edits.

**Test.** Held-out and Dev-9 with effort at 0.3, 0.5, 0.7 and 0.9, crossed with a silent thinking budget of 256, 512 and 1,024 tokens. Report output tokens per task, pass rate at 300 s and unlimited, and the two budget counters.

## Context without breaking the KV cache (F3)

Two patterns from other projects fit Inky's cache constraint: explore in a side context that never enters the main one, and steer at the sampler instead of rewriting earlier turns. §9.4 item 3 already covers capping tool output at the source, so this section lists only what is new.

**Steer at the sampler.** Manus calls KV-cache hit rate the single most important metric for a production agent and keeps its context append-only. It never adds or removes tools mid-task. It constrains the next action by masking token logits behind a prefilled reply, with modes that allow any reply, require a tool call, or require a call from a named group ([Manus](https://manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus)). Locals applies each model's tool-call grammar on the GPU, so an agent can call only offered tools and a reply cut off at its length limit still closes its call ([Locals](https://github.com/Generated-Computers/Locals)).

For Inky this turns planned text nudges into hard, cache-safe controls. The step 8 and step 14 zero-edit nudges can restrict the next call to an edit or a plan. The thinking budget and typed tool arguments use the same mechanism. None of them changes a prompt byte.

**Explore in a fork.** Cognition found its agents spending more than 60% of the first turn retrieving context. Its retrieval sub-agent runs up to 8 parallel tool calls per turn for at most 4 turns, where ordinary agentic search takes 10–20 serial turns. It returns files with line ranges, not a summary, because ranges can be checked and summaries can mislead ([Cognition](https://cognition.com/blog/swe-grep)).

Inky can do this with one model. Fork from the pinned prefix checkpoint, run the search there, and append only the `file:line` ranges to the main context. The 15–25 read steps of F4 then cost no main-context tokens, and decode stays near the fast end of the §7.8 curve (61.8 tok/s at 1K against 41.1 at 24K). Cognition trained its models for parallel search, so first measure whether Inkling batches read calls when asked: tool calls per step on Dev-9.

**Compact by masking.** JetBrains compared LLM summarization with simply omitting old tool outputs, in SWE-agent on SWE-bench Verified across five model configurations. Masking halved cost against the raw agent and matched summarization's solve rate ([The Complexity Trap](https://arxiv.org/abs/2508.21433)). If Stage 2 in `compact.coax` asks the model for a summary, replace it with masking: keep actions, drop old outputs, leave a path to the saved output. That is Manus's "restorable compression", and it costs one re-prefill and no decode.

**Two smaller habits.** Manus rewrites a todo file as it works, which keeps the goal at the end of the context where attention is strongest. It also leaves failed actions in context so the model does not repeat them. Both apply to what survives a compaction: restate the plan at the tail and keep error lines.

**Test.** Dev-9 Buckets B–E with and without the exploration fork. Report mean main-context tokens, KV hit rate, steps before first edit, and pass rate.

## Finding the right files (F4)

Other projects cut exploration with three things Inky lacks: a ranked map before the first step, a fixed localization routine in place of open search, and compiler feedback attached to the edit result.

| Technique | Who does it | Inky version |
| --- | --- | --- |
| Ranked repo map | Aider sends a map of key symbols and signatures. It ranks files on a dependency graph and keeps what fits a budget that defaults to 1k tokens ([Aider](https://aider.chat/docs/repomap.html)). | Have the Coax compiler emit definitions, references and `#load` edges. Rank them toward identifiers named in the task. Add the top slice to the user turn at step 0, next to the git-mined candidates of §9.3 item 1. |
| Fixed localization routine | Agentless drops model-chosen actions and runs localization, repair and patch validation in order. It solved 32.00% of SWE-bench Lite at $0.70 ([Agentless](https://arxiv.org/abs/2407.01489)). | Use it as the stall fallback. At the step 8 nudge, run two forced calls: pick files from the map, then pick edit locations from their outlines. |
| Diagnostics with the edit | Hermes checks syntax in-process, then asks a language server, and reports only the diagnostics the edit introduced. It waits about 80 ms and never blocks the write ([Hermes](https://hermes-agent.nousresearch.com/docs/user-guide/features/lsp)). | Extend the pre-edit Coax lint to a checker pass on the edited file, compared against the pre-edit result. A wrong edit is caught without a separate `coax test` step. |

The before-and-after comparison in the third row matters for Coax. A file with existing warnings would otherwise bury the one error the edit caused.

**Keep the map small and factual.** An ETH Zurich study found that repository context files did not generally improve task success and raised inference cost by over 20%. Agents followed the instructions; the unhelpful part was the repository overview. The authors recommend limiting such files to non-standard practices ([Evaluating AGENTS.md](https://arxiv.org/abs/2602.11988)). That bears on F5 as well. The 2.5 KB loader should favor Coax-specific rules over overviews, and any map or instruction change needs a paired Dev run.

**Test.** Steps before first edit and bytes read per task on Dev-9 Buckets B–E, with the map alone, the stall routine alone, and both.

## Engine: speculation on a sparse MoE, sampling, weights

Other work confirms the MoE verify cost found in §7.3. It also offers four ideas the plan lacks: a per-request utility gate, a drafter built from past outputs, a draft-length rule fitted from one layer, and a fidelity check on the weights.

**A utility gate would let DSpark default on.** A Georgia Tech and NVIDIA study measured the §7.3 problem directly. Draft tokens activate more weights, verification costs 2–3 times more, and speculation can slow MoE serving by up to 1.5 times. Their fix, Cascade, runs a short test phase per request, turns speculation off when token gain over verify cost falls below one, and otherwise picks the best draft length. It held slowdown to 5% and beat a fixed draft length by 7–14% ([Cascade](https://arxiv.org/abs/2506.20675)). Inky already backs off adaptively. Gating each request on measured time would switch DSpark off on open prose, where §7.8 shows 0.89–0.93 times baseline, and remove the reason it is opt-in.

**Fit the draft length instead of sweeping it.** A recent paper models MoE verify time as bytes requested over kernel bandwidth. The best draft length comes out as a staircase function of acceptance rate. Grouped kernels beat per-slot kernels only when their byte share is below their bandwidth ratio. On llama.cpp Metal, MLX-LM and CUDA that happened at about 5–8 tokens per pass or not at all, and the fit takes under an hour from one layer ([Lan, 2026](https://arxiv.org/abs/2610.34440)). §7.8 does not separate expert grouping from reading the KV cache once per pass. Its 1.81–1.93 times at 8K–24K is credited to the KV read, and it shows only 1.01–1.02 times at 1K–4K. The single-layer fit would show whether grouping helps at short context or a per-slot kernel is faster there. It would also replace the planned 2, 3, 5, 7 sweep with a fitted rule.

**Draft from history, not only from the prompt.** SuffixDecoding keeps suffix trees over the current prompt and over previous outputs, and drafts longer when the match is longer. It reports up to 5.3 times speedup on agentic workloads, 2.8 times faster than EAGLE-2/3 ([SuffixDecoding](https://arxiv.org/abs/2411.04975)). Inky's lookup drafter sees one prompt. A tree built from opt-in and Dev journals would also cover tool-call scaffolding and Coax idioms that recur across sessions. Use the lookup draft when the match is long and DSpark otherwise. The Ch 247 gate already defines which journals may feed it.

**Reference points from other Inkling 2 Mini runtimes.** A two-GPU vLLM build runs DSpark with 7 draft tokens and reports 2.39 tokens per target step overall, 3.31 on tool-calling turns, for 1.47 times decode speed ([W4A16 build](https://huggingface.co/Tech2wild/Inkling-2-Mini-W4A16)). Inky's 4.71 accepted tokens per round on file edits compares well, though the two counts may be defined differently. Tool-calling turns are the best case everywhere.

**Check weight fidelity, not only bandwidth.** The release ships in NVFP4 and MXFP8. An MLX port that loads those tensors as released measured 98.6% next-token agreement with the vendor's reference and mean KL 0.0018 ([inkling2-mlx](https://github.com/internet-projects/inkling2-mlx)). Inky runs an IQ4\_NL GGUF, and the status doc gives no agreement figure for it against the release. If that file was converted from the 4-bit release, it has been quantized twice. §9.4 item 5 frames native loading as a bandwidth gain; measure it as a quality question too, on code and tool-call positions.

**KV in 8 bits: one data point each way.** Locals stores keys and values in 8 bits, which raises its per-request limit on a 24 GB Mac from about 35,800 to 67,584 tokens and slows writes by 4–13% ([Locals](https://github.com/Generated-Computers/Locals)). The W4A16 page notes one of 69 tool scenarios flipped under FP8 KV.

**Sampling on the GPU, a suggestion of mine.** Sampling at temperature T equals taking the argmax of logits/T plus Gumbel noise, so the existing GPU argmax kernels can serve `temp=0.6`. Truncation then needs only a threshold: a cutoff relative to the top logit is one extra reduction, while top-k needs a k-th value. With noise seeded per position the sampled token is a fixed function of the logits, so the speculative verifier works under sampling as it does under greedy. This changes the truncation rule, so it needs a Dev quality check. It is an alternative to the partial top-k kernel in §9.4 item 2.

**Left out on purpose.** MoE-Spec caps the experts loaded per layer during verification ([MoE-Spec](https://arxiv.org/abs/2602.16052)). It is not exact, so it fails the 0 ULP parity gate.

## Evals: more tasks, sound graders, noise (F6)

The eval plan is right in outline. Other projects add five specifics: a control arm in the vendor's harnesses, repeated trials, grader audits done by reading failures, tasks minted from the repo, and a fixed environment.

**Add a control arm.** In the status doc, Inky as a harness is compared only with `mcworker`, its own predecessor. Thinking Machines reports Terminal Bench v2.1 at 71.6% with the model running in pi, and its SWE-bench figures in bash-only setups ([model card](https://thinkingmachines.ai/model-card/inkling-2-mini/)). Run the held-out and Dev-9 tasks in pi and mini-swe-agent against the same `psionic-inkling-serve`. If they pass tasks Inky fails, the gap is harness. If nothing passes, it is model, quantization or speed.

Read the card's high scores with care. OpenAI stopped reporting SWE-bench Verified in February 2026 as contaminated. The same card shows 5.6% on Terminal Bench v4.0 and 15.1% on FrontierCode v2.0, so fresh hard tasks are still hard for this model.

**Run each task several times and compare pairs.** Inky samples at `temp=0.6`, so one run per task is one draw. Anthropic's eval guidance is to report standard errors, cluster them when questions are related, resample each question, and compare systems on paired per-question differences. Clustered errors on popular evals came out over three times the naive ones ([Anthropic](https://www.anthropic.com/engineering/statistical-approach-to-model-evals)). For Inky: five seeded trials per held-out task, builds compared on per-task differences, tasks from one session treated as a cluster. Time to pass is censored at the timeout, so report it with a survival estimate, not a mean over passes.

**Audit graders by reading failures.** OpenAI audited 138 SWE-bench Verified problems its model did not consistently solve, with at least six engineers per case. It found material issues in 59.4%. In 35.5% the tests enforced implementation details, such as importing a function name the task never mentions. In 18.8% they checked behavior the task never asked for ([OpenAI](https://openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/)). The first category is F6. Step 2 greps graders for identifier checks; add the other direction. For every failed run that compiles and passes its own tests, a person reads the diff against the grader, and each suite reports a grader-fault rate.

**Mint Dev tasks from killed mutants.** SWE-smith builds an environment for a codebase and then synthesizes hundreds to thousands of tasks that break existing tests, 50k instances across 128 repositories ([SWE-smith](https://arxiv.org/abs/2504.21798)). Inky's repo already has the generator. `coax mutate` produced 1,866 mutants on `apple.coax` alone, 1,861 of them killed. Each killed mutant is a bug-fix task with a deterministic fail-to-pass grader and no identifier grep. One-operator mutants are easier than real work, so use them as a large Dev pool and for the noise floor. Keep real sessions as the locked held-out target.

**Fix the environment and record it.** Anthropic measured a 6-point gap on Terminal-Bench 2.0 between its most and least resourced setups. It recommends identical hardware conditions for scaffold and inference stack, runs at several times and days, and skepticism toward differences under 3 points ([Anthropic](https://www.anthropic.com/engineering/infrastructure-noise)). That backs the `host_clean.py` work in §8, including the GPU-utilization check. Add repeat runs on different days to the protocol.

## Self-improvement loops: what has worked elsewhere

Automated harness search now has verified wins outside OpenAgents. The loops that worked differ from the Level 2–3 design in three ways: the proposer reads raw traces, it changes tools and middleware more than prompts, and people curate the skills.

**Give the proposer raw history.** Meta-Harness searches over harness code with a coding agent as proposer. The proposer reads the source, scores and execution traces of every prior candidate through a filesystem. The authors argue that existing text optimizers fail in this setting because they compress feedback too hard. Its discovered harnesses beat the best hand-built ones on TerminalBench-2 ([Meta-Harness](https://arxiv.org/abs/2603.28052)).

Inky's Reflector receives anonymized failure summaries (§5.2, Level 2). Give it the whole Dev archive instead: each candidate's code, score and full journals. The held-out lock stays as it is. Compressed feedback may also explain the ten DSPy and GEPA attempts with no verified win. That is a hypothesis, not a finding.

**Make each change predict its effect.** Agentic Harness Engineering took a bash-only seed harness from 69.7% to 77.0% on Terminal-Bench 2 in ten iterations, past the hand-designed Codex CLI harness at 71.9%. Each edit ships with a prediction that is checked against the next round's per-task results. Ablations credited tools, middleware and long-term memory, not the system prompt, and the frozen harness transferred to three other model families ([AHE](https://arxiv.org/abs/2604.25850)).

Two things carry over. Add a predicted per-task effect to each candidate in `improve-policy.coax`; repeated misses are a cheap overfitting alarm. And aim Level 3 at tools and guards, which is where Inky's own evidence points: prompt wording did not break the shell habit.

**Keep an archive, later.** The Darwin Gödel Machine keeps an archive of agent variants and branches from any of them, not only the current best. It raised SWE-bench from 20.0% to 50.0% ([DGM](https://arxiv.org/abs/2505.22954)). Inky's hill-climb keeps a single `champion.json`. An archive multiplies comparisons, so it needs the larger task pool first.

**Have people write the skills.** SkillsBench found curated skills raised average pass rate from 33.9% to 50.5%, and focused skills beat exhaustive bundles ([SkillsBench](https://arxiv.org/abs/2602.12670)). Its first version also tested skills the agent wrote for itself before the task: about −1.3 points on average across five configurations ([v1](https://arxiv.org/html/2602.12670v1)). A follow-up that forms skills after execution and verifier feedback, closer to Level 2, found the gains unstable on frozen tasks. Reusing raw trajectories often beat the distilled skills ([SkillEvolBench](https://arxiv.org/html/2605.24117)).

So keep `mine.coax` and the two-arm gate, and change the author. The Reflector clusters failures and drafts. A person writes the final skill, at most 800 tokens and two or three modules. Expect few model-written drafts to pass the gate, and read that as the gate working.

## Weights, when they come off hold

The weight-level targets that work elsewhere are repository specialization and retrieval, and both run on thousands of examples. The 26-row routing LoRA is far short of that scale. Nothing here argues for lifting the hold in §1.3 step 6.

**Repository specialization.** SERA's Soft Verified Generation produces training trajectories from any repository without unit tests, thousands from a single codebase. It uses supervised fine-tuning only. The authors report it as 26 times cheaper than reinforcement learning and 57 times cheaper than earlier synthetic-data methods at equal performance ([SERA](https://arxiv.org/abs/2601.20789)). Coax is the natural case, since the base model has presumably seen little or none of it. This conflicts with law 4, because specialization trains more than the interface. Treat it as a deliberate exception that must clear the quarantine gate on log-loss and calibration.

**A retrieval adapter fits law 4.** Cognition trained its search models against a deterministic reward: weighted F1 over the files and lines retrieved, measured against ground truth ([Cognition](https://cognition.com/blog/swe-grep)). That is an interface skill with an exact grader. The repo's own history supplies the labels, namely the files and lines each commit touched. A small adapter trained on that target would serve the exploration fork described above.

**The routing LoRA may lose its targets.** Its two aims are the shell-edit habit and long thinking before tool calls. If post-command verification and the silent thinking budget work, neither needs training. That agrees with §9.6.

**DSpark distillation.** Measure draft acceptance on Coax edits before distilling. File edits already reach 4.71 accepted tokens per round. If Coax sits near that figure, distillation buys little.

## Coax and Lean

Coax is where Inky cannot lean on the model's training, so compiler-side help is worth most there. For Lean, other agents get goal states and batched tactic trials from tools Inky does not have yet.

**Coax: semantics over syntax.** A PLDI 2025 paper constrains decoding with the type system and cuts compilation errors by more than half. In its measurements, syntax errors were only about 6% of compile errors in generated TypeScript ([type-constrained decoding](https://arxiv.org/abs/2504.09246)). If Coax errors split the same way, a grammar on the sampler would catch little. The checker pass on each edit, described under "Finding the right files", targets the larger class. A type-constrained sampler for Coax is a research project; park it.

**Coax: a curated language card.** The skills and context-file studies agree on what helps: short, specific, non-standard facts written by people. A Coax card fits that description. It would list syntax that differs from languages the model knows, the most frequent error codes in Dev journals with their fixes, and the test commands. Make it the first skill a person curates.

**Lean: goal state and batched attempts.** lean-lsp-mcp gives agents the proof goal at a line, file diagnostics, premise suggestions from a simplified LeanHammer, and Mathlib search. Its multi-attempt tool runs several snippets at one line and returns the goal state and diagnostics for each ([lean-lsp-mcp](https://github.com/project-numina/lean-lsp-mcp)). Inky proves by editing a file and running `lean-run verify`. A goal tool and a multi-attempt tool are deterministic and read-only, and one call screens several tactics. They belong at Level 0 and serve the Lean algebra and logic suite, where core smoke already averages 46.3 s per proof.

## Proposed changes to the ranked plan

The plan's ordering principle holds: measure first, weights last. These rows add to it or amend single items, in the order I would try them.

| Order | Change | Plan item it touches | First test |
| --- | --- | --- | --- |
| 1 | Add a pi and mini-swe-agent control arm to the rerun; five trials per task; compare builds on paired per-task differences | §1.3 step 1 | Held-out 6 and Dev-9 on the same server and host |
| 2 | Audit graders by reading failed runs that compile and pass their own tests; report a grader-fault rate | §1.3 step 2 | All 27 session-history graders |
| 3 | Measure IQ4\_NL agreement with the released NVFP4 and MXFP8 weights | §9.4 item 5 | Next-token agreement and KL on code and tool-call positions |
| 4 | Make the thinking budget silent, with no budget announcement; fit a fixed numeric effort | §9.2 item 1 | Effort by budget grid on Dev-9 |
| 5 | Verify shell edits instead of refusing them: guarded edit command on `PATH`, plus a diff guard after each command | §9.2 item 2(b) | Three-arm Dev-9 run |
| 6 | Put remaining time or steps in tool results; require a verification pass on exit | §9.2 item 3 | Timeouts and false "done" reports on Dev-9 |
| 7 | Force tool choice at the sampler for the stall nudges; type tool arguments | §9.3 items 3–4 | Wasted steps on Dev-9 Buckets B–E |
| 8 | Exploration fork returning `file:line` ranges; ranked Coax repo map; checker diagnostics with each edit | §9.3 items 1–2 | Steps before first edit; main-context tokens |
| 9 | DSpark utility gate; fitted draft length; history-based drafter; target file as prediction on rewrites | §1.3 step 3, §9.4 item 4 | Decode tok/s by turn type at 1K–24K on an idle host |
| 10 | Dev pool minted from killed mutants | §9.5 | Noise floor on 100 or more tasks |
| 11 | Lean goal and multi-attempt tools; a human-written Coax card | Level 0, Level 2 | Lean algebra and logic suite; paired skill gate |
| 12 | Raw Dev archive for the Reflector; a prediction with each change; people write the skills | Levels 2–3, on hold | After the held-out score moves |
| 13 | Retrieval adapter on commit-derived labels; repository specialization | Level 4, on hold | After Levels 0–3 |

Rows 1–3 are measurements and can ride on the rerun already planned as step 1. Row 5 is the one that reverses a current recommendation.
