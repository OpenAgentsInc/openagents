# Archive confirmation: executed file checks and reproduced defects

Status: frozen before candidate generation or any official outcome is read.
This follows the negative eight-task confirmation and the development mini
controls. It does not reuse either population as confirmation. See
[selection and limitations](archive-confirmation-design.md).

## Population and retained candidates

Use all 12 declared archived tasks at upstream commit
`3b5caaa4863d64dda7f0957bf4fc2d4f019202d4`. Environment-only preflight succeeded
for every task. Run three Luna and three Astra attempts per task: 72 planned
candidates, with two concurrent trials per executor and four total. Do not choose
tasks, attempts, or stopping points from outcomes. There is no interim grade join.

Both executors use the same previously retained v13 binary:
`7df7cde47d0f16c8f9200cbd21899254588c0b587f3bbe6f2272fd2eedfb0f5d`.
The unchanged `prospective-policy.json` and `prospective-astra-policy.json` record
final scenario checks, the original combined verdict, and bounded snapshots.
They enable no new repair, escalation, or candidate selection. Original task
allowances remain in force, including tasks with shorter deadlines. This is not
a same-model Coder ablation and does not measure a new runtime policy.

Build each public task image before generation and pin its immutable image ID in
a staged copy of `task.toml`. That reference is the only task-file change. The
provenance manifest hashes every original and staged file; instructions, official
graders, solutions, resources, and inputs are unchanged. The reviewer uses the
same image. Public initial inputs all fit the snapshot bound; a candidate that
later exceeds it remains in the denominator with unavailable review evidence.

The jobs retain official results for the later join, but neither the check
pipeline nor its operator reads those results before prediction sealing. Ordinary
trial collection may write result-derived files; they are not model input or
available evidence for selecting a rule. Launcher output stays in retained files,
not progress messages. No retry replaces an agent failure. A setup failure stays
listed with an unknown grade. Any later environment-only regrade must retain the
original failure and prove identical candidate bytes and grader assertions.

## Frozen check rule

Use the check binary built at `ca3c91b581`, SHA-256
`68c86604d69910b36bcdd6246524530f2024f607e9fb003197bb06ff011edd53`.
The cohort orchestration is `archive_run.py`; the check composition is
`archive_checks.py` and `reproduce.py`, frozen with this protocol.

1. **Extract the public file contract before any candidate exists.** Run the
   unchanged #9628 extractor on each pristine pinned image, with Jev off. Retain
   the complete original plan and its digest. Select only instruction-derived
   `path` and `format` items naming normalized paths inside `/app`, with no command
   or `not_executable` reason. Reseal and retain that selected plan. Run it on the
   untouched image once for provenance, without using that result to fit a rule.
   This deliberately excludes command, interface, example, and exit-code items:
   the earlier contract study found a split-command extraction error, and command
   execution can depend on packages absent from the restored environment.
2. **Run those file checks on the exact candidate.** Require the same complete,
   attributable snapshot as reproduced review. Mount `/app` read-only in its
   public image, with no network or credentials. The Rust component reads and
   compares the actual files. Any `differed` result establishes a file-check
   failure. A setup error, unreadable evidence, or unavailable snapshot stays
   unknown; a matched item does not establish task completion.
3. **If no file-check failure is established, reproduce defects.** Use the frozen
   Astra high reviewer with `--prompt literal-v2`, strict literal citations, the
   unchanged two Jev questions, and score `min(reproduced, required) >= 0.8`.
   Keep at most three findings and seven commands; commands have 30 seconds and
   the whole review 300 seconds. The root and candidate are read-only, there is
   no network, memory is 2 GiB, and writable `/tmp` is 128 MiB. Missing runtime
   packages, unsupported tests, invented expected values, and timeouts alone
   cannot establish candidate failure. Outside retained files are usable only
   after exact comparison with the same paths in the immutable public image.
4. **Combine by OR.** Return fail when a file check differs or a valid reproduced
   finding meets 0.8. Otherwise return unknown. Skip the expensive review when
   the cheap arm already decides the OR expression, recording that skip. This
   composition never calls a candidate passed. Report the cheap arm separately
   and the incremental detections from review, without pretending skipped reviews
   were negative standalone results.

The baseline signals are the same candidates' recorded `checks.final` and
`verdict.combined`. Their policies and predictions are unchanged. No source-only
review, report refit, readiness opinion, threshold search, citation recovery, or
new call after opening labels contributes to the confirmation rule.

## Seal, measure, and decide

Retain every check input, plan, command, output, native reply, Jev request and
answer, candidate identity, unavailable reason, and usage record. Commit a digest
of all candidate calls and baseline calls before opening official results. Join
by job and trial identity, with duplicate and mismatched joins refused.

Report exact failure-call counts, failure precision and recall with Wilson 95%
intervals, paired whole-task bootstrap differences (10,000 resamples, seed 9584),
per-executor and per-task results, within-task separation, false alarms, missing
grades, and unknown check coverage. Every officially failed candidate remains in
recall, including unavailable snapshots and interrupted reviews. Undefined
precision is undefined, not zero. Preserve all 72 scheduled attempts and all
infrastructure records even when fewer than 72 yield a grade.

Only a joint improvement over the original scenario checks can support #9584's
completion claim. Both point estimates must improve; inspect and report the
paired intervals before claiming the difference is established. A negative or
inconclusive result stays negative or inconclusive. Do not fit on these outcomes
and call the revised rule held out. These selected CPU archive tasks cannot
establish TB4 completion rates, a Fable win, or a deployed stopping policy.

## Cost, verification, and limitations

The executor soft native budgets are $0.09 per Luna trial and $3 per Astra trial:
$111.24 across all 72 if each consumes its bound. Each requested reproduced review
has the existing $2 soft limit, at most another $144 if all 72 need review. A call
already in flight can exceed a soft bound. Record actual list-price usage, all
unknown usage, and Jev separately; do not present these bounds as an invoice.
No model call occurs in environment preflight or deterministic file extraction.

The corrected mini controls and scoped Rust gate precede this freeze. Eleven
Python regressions pass on coderos, including compatibility with a retained Rust
plan digest, refusal of altered plans, and excluding command or candidate-authored
items from the cheap arm. The Mac's default Python 3.9 cannot import `tomllib`;
the harness uses its existing Python 3.11-or-newer environment on coderos.

The first plan-only preflight failed because Docker's archive-copy endpoint would
not write through the read-only root to `/tmp`. Preserve it under
`checks-preflight-docker-copy`. The corrected transport streams input bytes into
the writable tmpfs. This happened before candidate generation, without inference.
The environments vary in installed tools: several Ubuntu tasks have no Python,
OCR, or graph-query runtime. The check must disclose that limitation and return
unknown when it prevents a valid test; it must not treat a package missing from
the retained environment as a candidate bug.

The next plan preflight found that the Nix-built checker needs its host loader.
Preserve that attempt under `checks-preflight-native-loader`. The working runner
mounts only the checker and four individually hashed runtime libraries at `/opt`,
read-only, and invokes that loader explicitly. It does not replace any task
runtime, expose host credentials, or give those mounts to the reasoning reviewer.
The public plans now execute successfully on all 12 images. Nine selected file
checks occur on three tasks (`constraints-scheduling`,
`model-extraction-relu-logits`, and `openssl-selfsigned-cert`); the other nine tasks
have no selected cheap item. Keep all 12 tasks and the same rule. This coverage
inventory precedes every candidate and does not justify adding task-specific code.

## Infrastructure amendment before candidate generation

The first scheduled jobs (`9584-literal-r1`) produced 72 setup refusals, zero
agent episodes, and zero verifier logs. The strict contamination guard found
eight task names in the unrelated offline finish-replay exclusion list added by
`54f1f1bbd3`. No candidate or model call existed. Preserve all 72 attempts as
infrastructure failures with unknown grades; they are not candidate failures.

[#9642](https://github.com/OpenAgentsInc/openagents/issues/9642) moves that list to
explicit offline CLI arguments, outside live product guidance. It leaves the
contamination guard and finish rule unchanged. Both original policies pass the
actual guard, and all 211 published finish-replay rows remain byte-identical.
The scoped Rust gate passes at `285d6056df`.

Permit one infrastructure restart as `9584-literal-r2`, in a separate cohort
directory. This amendment is recorded before any candidate generation. Keep the
same 12 tasks, image pins, policies, three attempts per executor, frozen executor
and check binaries, literal-v2 prompt, strict validator, threshold, and budgets.
Copy the successful pre-candidate public plans byte-for-byte. Do not include the
72 setup refusals in candidate precision or recall, but retain them in the
scheduled-attempt accounting. A later agent failure is not eligible for this
restart. The launcher now takes an explicit `--run-id r2`; it still refuses any
job directory that already exists.

The check pipeline may inspect a completed attempt while later attempts run.
Completion is the existence of its `result.json`; the file's contents remain
unopened. This overlaps independent work without changing a check, retrying a
review, or stopping from outcomes. Each completed check is immutable on rerun.
`seal_archive.py` refuses a cohort that lacks any of the 72 scheduled attempts,
including unavailable candidates. `join_archive.py` verifies that exact seal is
in a commit on `origin/main` and that all candidate, check, and baseline records
still match before it opens an official result. The measurement preserves every
failed candidate in recall and reports within-task comparisons separately.
