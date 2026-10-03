# Round 2: acceptance and stopping rules

This protocol is frozen with the registration manifest before round-2 model calls. The first experiment remains unchanged. Issue #10166 is now a development case: its failure, expected source spans, and Git behavior have informed this round. It cannot serve as unseen evidence for the new packer.

## State the hypothesis narrowly

An automatically prepared brief containing explicit task anchors, complete relevant functions, and named tests/specifications may increase acceptance or reduce the cost of obtaining an accepted change. Both conditions get equal external test feedback and one repair opportunity. The focused packer has a 16 KiB optional-evidence budget. Test cheap runtime facts as a separate ablation, with their probe implementation and output frozen separately; do not silently add them to the source-only condition.

The control gets the full task, mandatory instructions, and the same available tools and feedback. Treatment adds only the automatically generated, source-bound brief specified by its registered variant. Do not let treatment omit or replace required instructions.

## Freeze before launching

Record a manifest with SHA-256 digests for the common task, complete instruction block, source snapshot, packer binary/source, optional brief, fact-probe script and results, checker, feedback formatter, runner, and this protocol. Record model, effort, CLI version, resource limits, fixture policy, cache policy, slot, and planned run order. Use immutable external artifacts and exclusive run directories.

Freeze one clean held-out historical task and its pre-fix source before development results. A separate evaluator should select it and keep its identity, body, patch, and acceptance cases unavailable to the packer implementer. The evaluator may validate its historical reconstruction; the packer team must not tune on it. Hash the selection and checker privately before the first development call. If this separation is no longer possible, label the task a validation case and select a genuinely unseen holdout before claiming transfer.

Once development starts, run four pairs with one frozen treatment, subject only to the predeclared first-pair futility stop below. Balance order, for example AB, BA, AB, BA, with A=control and B=treatment. Record the order in advance. Do not alter the brief, thresholds, prompt, checker, or probe between pairs. If a harness defect invalidates a run, preserve it and invalidate/repeat the whole pair under a documented reason fixed before seeing acceptance; do not relabel model or candidate failures as infrastructure failures.

## Fixed feedback opportunity

Each run consists of one implementation turn, external checks, and at most one repair turn if checks fail. Successful first attempts stop. Both arms use the same rule, model, effort, tools, and total token/dollar/time limits. Keep the same Claude CLI process and its context for the one feedback turn; do not give one condition a fresh summary while the other retains its history. The repair receives only its own diagnostics and current files. Keep other-arm outputs and hidden checker source inaccessible.

Apply the same deterministic formatter before scoring each attempt in both arms. Preserve the raw candidate patch and its formatting status first, then record the formatter command, outcome, normalized patch, and both patch digests. Run checks against the normalized patch. A formatter parse failure is feedback, not a researcher repair. Keep raw and normalized outputs so formatting cannot conceal semantic intervention.

Return compiler errors, formatter failures, and a fixed per-failure diagnostic format. Do not supply the historical solution, a researcher-written fix, or selective extra hints. The same checker may produce different diagnostics because candidates differ; that is expected. Freeze diagnostic truncation and error ordering so one arm cannot receive a richer hand-curated report.

The final endpoint includes all allowed repair work and final checks. Count every model attempt and preparation step. Report first-attempt acceptance separately from final acceptance. A run ending in malformed output, timeout, excessive scope, instruction tampering, or an unaccepted patch remains a failed run.

## Clear practical win

These are engineering thresholds for this small panel, not a statistical claim about a model population. Adopt or revise them now, then freeze them. Do not choose a threshold after seeing results.

### Development gate

Treatment must accept 4/4 development runs after the fixed feedback opportunity. Otherwise the round does not advance. A first-pair treatment failure after its one permitted repair makes this gate unreachable: explicitly allow stopping that variant after the completed first pair. Preserve both runs and label the variant rejected by the predeclared futility rule. Otherwise finish the four-pair panel. A revised treatment starts a new round with new versioned artifacts and a fresh run sequence; do not retain only favorable development runs.

### Held-out gate

Freeze the treatment before the evaluator reveals the held-out task to the harness. Run exactly four new balanced pairs on that task. A practical win requires either of these predeclared routes:

1. **Efficiency without an acceptance regression:** treatment accepts 4/4 held-out runs and is no worse than control on acceptance in development or validation; treatment reduces median total CLI list-price estimate plus any paid briefing generation by at least 20%; at least 3/4 paired costs are lower; treatment median total wall time is at most 10% higher. Include all first attempts, repairs, and per-run preparation. Compare cost at the fixed endpoint and report accepted counts beside it; do not label failed control runs a measured cost through acceptance. Publish machine costs and engineering effort separately, rather than treating them as zero.
2. **Correctness:** treatment accepts at least 3/4 held-out runs and control accepts at most 1/4; treatment median total cost and wall time are each at most 10% higher. No treatment candidate may violate the explicit data-preservation or instruction-integrity rules, even if its ordinary tests pass.

Neither route permits a loss of applicable instructions or weaker acceptance checks. If neither route passes, report no clear win on the registered panel. Four pairs are not enough for a broad significance or superiority claim, even when an engineering gate passes. Publish every registered round and cumulative experiment cost.

Further rounds must state a new hypothesis, freeze it, and use a new holdout. Repeatedly checking the same held-out task while changing the packer turns it into development data. Set a round or monetary cap for each next batch before starting; user authorization to keep improving does not make an open-ended sequence statistically confirmatory.

## Stronger development checker

`ignored_worktrees_v2.rs` retains the first experiment's seven named tests, unchanged in substance. It adds ten explicitly labeled safety extensions on Linux:

- Recognized cache directories nested under several ordinary parents remain removable.
- Regular files named `target`, `node_modules`, and `.cargo-target-notes` remain protected.
- A directory named `.DS_Store` remains protected; an ignored regular `.DS_Store` metadata file remains the explicit disposable-file exception.
- An unknown ignored parent remains protected even when its child has a cache name.
- Ignored names with spaces and newlines remain protected with their contents intact.
- A symlink named `target` remains protected, and its external target is neither followed nor changed.

The checker uses only the historical `removable` and `remove` APIs. It creates temporary repositories and a local bare remote. It does not touch owner worktrees. Its helper constructs real files/directories/symlinks so type assertions are independent of candidate classification.

Add the following normative text to the common task in both arms before freezing it:

> This round strengthens the original task's safety contract. Recognized cache names such as `target`, `node_modules`, and `.cargo-target*` denote actual directories, not arbitrary regular filenames or symlinks. Unknown ignored data and symlinks must keep the worktree. A regular `.DS_Store` metadata file is the explicit file exception; a `.DS_Store` directory is not disposable. Nested recognized cache directories must remain eligible. Preserve names and contents containing spaces or newlines. Keep the worktree rather than archive its ignored files for this experiment.

The historical fix is an oracle only for the original seven cases. Its name-only classifier should fail some extensions. Report calibration by subset; do not require the historical fix to pass new requirements or present its new failures as a revision to the old audit. Calibrate the full suite using an independently reviewed reference implementation outside all executor roots, plus known-bad mutations or the prior candidates. Confirm that positive cases are reachable and safety failures are detected before paid calls.

Empty ignored directories are not a new acceptance requirement in this minimal suite. Retention is allowed. Do not add extra policy cases mid-round merely because a candidate inspires them; retain such findings as exploratory and place them in a later version.

## Immutable instruction coverage

Instruction integrity belongs in the harness, not in the product's Rust checker. Compile the instruction manifest from the pre-fix snapshot before any candidate runs. Include the full root `AGENTS.md`, relevant nested instruction files, and the full required Google style skill. Include any other applicable mandatory skill; if its applicability is uncertain, preserve it or explicitly resolve that question before freezing. A hash cannot prove that the chosen instruction set was complete, so retain the path list and its selection rationale for review.

Inject that complete block byte-for-byte through the same append-system-prompt in both conditions, alongside identical task and experimental operational overrides. Verify that the treatment user input begins with the exact control user-input bytes and adds only the frozen treatment suffix. Verify the full common system-append block separately. Do not infer coverage from an excerpt, summary, or a model saying it followed the rules. The guard must validate the common system-append block and control/treatment user-input relationship separately, then check instruction files before and after each turn. Pass the system-append text file to instruction_guard.py verify --prompt; use its pair command for the user inputs. Any candidate edit to an instruction file invalidates acceptance.

`instruction_guard.py` implements bounded byte-level checks and produces no model calls. It does not perform semantic applicability selection. Its three synthetic self-tests pass locally. Freeze its digest with the other harness artifacts. Its emitted common block is meant to be placed in both inputs, not as optional retrieved evidence.

## Persistent-session accounting and cache policy

The coordinator verified two input turns in one Claude process with session persistence disabled. In the observed CLI format, `total_cost_usd` and `modelUsage` are cumulative while `usage` and `num_turns` are per input turn. Use the final cumulative cost once. Compute repair cost as final cumulative minus first cumulative; do not sum cumulative result rows. Retain both raw result records privately and publish unambiguous per-turn and cumulative columns.

Use `--exclude-dynamic-system-prompt-sections` identically in both arms to keep per-workspace state out of the shared system prefix. Put complete mandatory instructions in the same append-system-prompt block. Before each task panel, run one tool-enabled no-op with the same model and flags to prewarm common instructions; record its cost and elapsed time separately as shared setup, not a treatment saving. Cache state still cannot be fully controlled, so retain cache creation/read counters and balanced run order.

## Measurement cautions

- Report measured CLI usage and cost basis. Subscription list-price estimates are not verified charges.
- Preserve cache creation/read counters and elapsed preparation/check/repair times. Cache reads are repeated exposure, not unique text; do not add parallel durations into serial wall time.
- Keep model turn time, full run endpoint, and machine cost separate. A slower but more correct run may be useful, but it is not automatically a cost win.
- Use the same target-slot warmup policy for both arms, unique logs and request identities, and declared toolchain/Git versions. A historical template's existence does not prove its dependencies are usable.
- Give paid model judgments explicit separate treatment labels. No System One call is needed for the deterministic source-anchor, instruction-integrity, or filesystem-type checks. If added later, record its cost and latency and compare it against the same candidate pool.
- The failed first experiment remains evidence. Its known issue, outputs, and new probe are development inputs now; never call this round a fresh test of the same unknown failure.

## Selected round-2 variant

The treatment uses `preview --focused --no-lexical --no-symbols`: explicit
references, complete containing declarations or small files, and nearby tests
and manifests. Large files without anchors use a labeled first-64-line fallback. Broad lexical and symbol candidate selection are disabled. A
fixed synthetic Git probe is appended only when the task contains all three
case-insensitive words `git`, `ignored`, and `worktree`. The probe runs on the
verification host. It never reads candidate or historical-fix source. This
round tests that bundle; any benefit cannot be attributed to the source pack
or probe alone without an ablation. The held-out task is selected before
model calls and cannot change this trigger or the source packer.

The development order is control, treatment, treatment, control, control,
treatment, treatment, control. The held-out order is the same. Each run has a
$10 CLI list-price estimate budget and 600 seconds of agent time; each check
has a 300-second limit. The maximum planned panel is 16 runs plus two shared
warmups. Stop and reassess after this batch rather than extending its sample
until a threshold passes. Further hypotheses require separate registrations.

The paired efficiency threshold requires both arms to accept 4/4 held-out
runs. Failed runs remain reported and are not called cost through acceptance.
