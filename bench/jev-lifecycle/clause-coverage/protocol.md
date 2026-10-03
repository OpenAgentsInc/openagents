# Advisory review of requirement coverage

## Purpose and timing

Review the 12 final candidates from the exposed beta/gamma native pilot after
that panel ends. This is a separate, exploratory component experiment. It does
not change the panel's prompts, checkers, repairs, acceptance, costs, timings, or
win gates. No review answer triggers a repair or execution.

The policy is designed while the native panel is running. The coordinator has
reported a gamma failure and later reported additional defects found by static
review among some primary-check passes. Those statements establish that this
is not a blind accuracy experiment. The tool author has not read the candidate
patches, exact checker failures, or those additional defect details while
implementing this policy. The earlier broad review and its evidence omissions
motivate the changed question and source unit.

Freeze the prototype, tests, this protocol, and the imported source modules
before preparing the 12 real requests. Retain that freeze and every skipped
preparation. Do not change the policy after seeing Jev responses. Any later
change needs a separate experiment and retained earlier artifacts.

The imported closure is `context.py`, `gateway.py`, `candidate.py`,
`seed_manifest.py`, and `capture_limits.py`. Bind each exact file digest,
including the transitive candidate-validation helpers.

## Inputs and evidence

The preparer accepts only an immutable public source commit, the exact public
task manifest entry, its pinned syntax index, the canonical final candidate
manifest and payload, and the expected source-archive and candidate digests.
It does not read acceptance receipts, independent tests, reference fixes,
native conversations, arm names, model identities, or review labels.

The preparer verifies the candidate payload, every changed file's original
bytes and mode against the pinned Git tree, and all indexed bytes used for
entry-point discovery. It reads immutable Git blobs and the candidate archive
without checking out or executing the candidate. Candidate deletions remain
explicit. Symlinks, invalid mandatory UTF-8, incorrect identities, and source
read failures produce `ready=false`.

The request contains:

1. The exact public title and prompt, plus its package names. Sentence splitting
   preserves each clause's exact text; it does not rewrite or infer requirements.
   At most 24 clauses are admitted. A compound sentence remains compound.
2. Every complete changed implementation file. Rust files outside directories
   named `tests`, `benches`, or `examples`, except `test.rs` and `tests.rs`, are
   implementation files. Cargo manifests, Cargo locks, and build scripts are
   mandatory too. A complete source file can contain inline tests.
3. Every complete pinned required public reading. These are the original
   specification even if the candidate edits a corresponding document.
4. Complete unchanged public entry-point files, subject to the optional budget.
   Discovery scans at most 48 implementation Rust files under the named package
   source roots. Explicit public-task paths come first, then lexical path order.
   The pinned AST signatures identify files containing `pub` functions; test
   declarations and signatures marked as parse errors do not count. Changed
   files are already included. Among remaining files, `src/lib.rs` files come
   first, then files with more public functions, then lexical path order. At
   most eight entry-point files are eligible. This heuristic is not a resolved
   call graph and can miss relevant callers or dependencies.
5. Other complete changed files, after entry-point files and only if space
   remains. Their role is supporting evidence. Tests and comments cannot by
   themselves establish implemented behavior.
6. A source catalog with path, role, original blob when available, current
   content digest, byte count, line extent, completeness, deletions, and explicit
   omission reasons. No file is sliced.

There are at most 128 changed files and a 2 MiB read bound per file. Included
files are complete UTF-8 file bodies, not assertions of valid Rust syntax.
The candidate is not reparsed, compiled, or run in this experiment. Retain
invalid-source and oversize statuses separately from model uncertainty.
Candidate validation also has a 256 MiB aggregate read bound, 256 archive
entries, and a 30-second deadline. These validation limits do not change the
original native capture or acceptance limits.

## Request bound and questions

The complete serialized request, including questions and JSON escaping, must
fit the existing gateway client's 128 KiB bound. Complete changed implementation
and complete public contracts are mandatory. If that mandatory request does
not fit, mark the case skipped and make no call. Do not drop a changed
implementation file, truncate a contract, or silently send a prefix. Optional
files are admitted in the order above with 1 KiB left for final catalog status
updates; any file that does not fit is omitted whole. Verify the final request
size again and retain its exact SHA-256 and bytes.

Ask one independent `Choice` question per exact clause, in one batched request.
Each question contains the clause because question IDs are routing metadata,
not meaning available to the model. It refers to the current candidate source,
actual shown caller paths, pinned contracts, and source omissions. The four
mutually exclusive labels are:

- `demonstrated`: visible implementation and relevant callers demonstrate the
  requested clause; existing unchanged handling counts.
- `missing_handling`: visible code concretely omits or violates requested
  behavior. An absent source file does not establish this conclusion.
- `insufficient_evidence`: an essential path, dependency, external behavior, or
  interpretation remains uncertain.
- `non_code_requirement`: the clause only requests a reading or process step.
  A clause that also requires behavior is judged for that behavior.

The wording asks whether the caller uses the relevant helper and covers the
clause's conditions. It supplies no known defect examples, solution symbols,
private checker names, or outcome hints. Every clause uses the same rubric.
No probability threshold, automatic acceptance rule, aggregate correctness
score, or repair policy is introduced.

This design follows the distinction between a claim being supported,
contradicted, or unsupported by its context in TypeSafe's
[citation-check cookbook](https://docs.typesafe.ai/cookbooks/citation_check).
The [Choice documentation](https://docs.typesafe.ai/primitives/choice) and
[API reference](https://docs.typesafe.ai/api), read on October 3, 2026, support
batched typed questions and explicitly defined options. These examples do not
establish coding-review accuracy or a calibrated confidence threshold here.

## Calls and budget

The prototype only prepares evidence; it contains no network call path. The
coordinator later calls the existing gateway client with the prepared state
and questions. Use `typesafe-ai/jev` through the same Vercel gateway. The alias
is not a version pin. Retain exact requests, responses, returned identity,
provider metadata, internal fallback information, wall time, and billed usage
reported by the gateway.

Review candidates in original panel order. Make at most one request per ready
candidate and at most 12 total requests. There are no application retries or
replacement cases. Stop admitting calls when known review cost reaches $0.05,
or when a sent call has unknown accounting. This is an observed admission
threshold; an already-admitted request can overshoot it. Retain overshoot and
failed calls. Skipped preparations have no model call and zero inference cost;
their preparation time still counts in this experiment's overhead.

Use the existing gateway client's 30-second socket timeout and enforce a
90-second outer deadline around each call. Retain an interrupted call's launch
intent and any partial receipt; if its charge is unknown, stop further
admission. Do not infer a zero charge from the absence of a response.

Before execution, the coordinator binds all prepared request hashes to the
freeze, verifies they fit the bound, and registers the budget and schedule.
The preparer does not enforce a cross-call budget because it never calls a
model. The caller must enforce these admission rules. Do not begin calls until
the original native panel has ended.

## Analysis

Report preparation and call latency separately and together, requested and
answered clause counts, source bytes/files, omissions, skipped/invalid cases,
all four label counts, full probabilities, and known/unknown costs. Keep every
candidate in the accounting, including preparation skips and service failures.

Compare advisory flags with independently retained defect evidence after
responses are fixed. Report coverage of known failing cases and flags among
primary-check passes. A primary-check pass is not proof of correctness, so do
not compute raw accuracy by labeling every such flag a false positive. Source
gaps, uncertain labels, and demonstrated omissions need separate explanation.
Neither static review nor these four labels provide exhaustive correctness
ground truth. No native speed or coding-quality benefit follows from this
component experiment alone.

## Invocation

The task input is one safe public task object, not a private execution config:

```sh
python3 coverage_review.py \
  --harness /path/to/openagents \
  --repo /path/to/source-object-repository \
  --task public-task.json --index index.json \
  --candidate-dir retained/native \
  --candidate-sha256 CANDIDATE_DIGEST \
  --archive-sha256 SOURCE_ARCHIVE_DIGEST \
  --output NEW_PREPARATION_DIRECTORY
```

`preparation.json` retains the evidence status, request identity, omissions,
and elapsed time. `request.json` is written only for `ready=true`. No tracked
panel or product files are modified.
