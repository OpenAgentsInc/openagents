# Jev lifecycle experiments

Small Python benchmark tools for source preparation, bounded evidence gathering,
and patch review. Product code remains in Rust. This directory is separate from
the frozen `bench/delegation-study` preparer and does not change product routing.

Use Python 3.11 or later for the lifecycle tools; the clause review runner
requires Python 3.13 or later. Export `AI_GATEWAY_API_KEY` in the invoking process.
The client follows the repository's Vercel transport in `crates/jev/src/doors.rs`:
`https://ai-gateway.vercel.sh/typesafe/v1/systemone`, model `typesafe-ai/jev`.
It makes one request, disables redirects and environment proxies, and requests
no evaluation fallback. The gateway alias does not pin an underlying model
version. Actual provider attempts and reported cost remain in every receipt.

## Preview preparation and one additional read

First create a syntax index for the exact commit with
[`briefing-lab`](../../crates/briefing-lab/README.md). Keep indexes and output
outside the inspected checkout. The manifest contains a `tasks` array with
`id`, `source_commit`, `title`, `prompt`, `packages`, and `allowed_paths`.
Optional `required_public_readings` holds public paths. Private evaluator fields
are excluded by the context builder.

```sh
python3 bench/jev-lifecycle/lifecycle.py prepare \
  --repo . --index /path/to/index.json \
  --manifest docs/audits/2026-10-03-independent-efficiency/system-one-delegation/replacement-public-task-manifest.json \
  --task-id alternative-beta --output /path/to/new-preview --live
```

Omit `--live` for deterministic preparation without a provider call. Every output
directory must be new. Read `deterministic.md`, `jev.md`, `result.json`, and
`source-probe.json`. The result contains exact task clauses mapped to candidate
IDs, a missing-implementation judgment, and one enumerated file choice. Source
retrieval checks the pinned Git identity and returns bounded complete lines.
No model-generated command runs. A source slice explicitly reports truncation.
The model's selected read can still be unhelpful.

Both packs use the same candidate pool and 16 KiB rendered limit. Complete
declarations and partial slices have different labels. Extra candidates and
omissions remain in `context.json`; a relevance score cannot recover code
missing from that pool. The requirement map is diagnostic, not an execution
plan or proof that a requirement is covered.

## Review a retained patch

```sh
python3 bench/jev-lifecycle/lifecycle.py review \
  --input docs/audits/2026-10-03-independent-efficiency/jev-lifecycle/review-inputs/R01.json \
  --output /path/to/new-review --live
```

The input carries `task`, `source_commit`, `diff`, and `source_context`.
Optional `observed_checks` must contain only public runtime observations, never
independent acceptance labels or checker text. The pilot inputs omit it.
The output recommends revising, inspecting, or running checks. It cannot accept
a patch. The initial retained experiment misses every demonstrated defect;
read the [audit](../../docs/audits/2026-10-03-independent-efficiency/jev-lifecycle/README.md)
before using these judgments in a workflow.

`focused_review.py` is the separate development follow-up. It asks concrete
contract questions over applied candidate source and is tailored to three
exposed task families. It is not a general correctness checker.

## Preview declaration selection

`spans.py` separates metadata selection from source materialization. It reserves
catalog positions for public operations, retains complete task clauses, and
renders the same bounded source pack for deterministic and supplied Jev choices.
This command makes no network calls:

```sh
python3 bench/jev-lifecycle/spans.py \
  --repo . --rev SOURCE_COMMIT --index /path/to/index.json \
  --task-manifest /path/to/public-tasks.json --task-id alternative-beta \
  --output /path/to/new-span-preview
```

Read `deterministic-pack.md`. `state.json` and `questions.json` contain the exact
semantic-selection input. Supply a retained gateway response with
`--answers /path/to/response.json` to also write `jev-pack.md`. The preview does
not claim complete dependency coverage. Required instructions and contracts
must be supplied separately in full, as the native pilot does.

## Run or inspect the native comparison

The [native pilot protocol](../../docs/audits/2026-10-03-independent-efficiency/jev-native-pilot/protocol.md)
defines a fixed 12-attempt comparison of bare Claude, deterministic lean
delegation, and Jev source selection. It uses Linux isolation, pinned historical
source, prebuilt Cargo seeds, independent checks, and a metered inference broker.
It requires separately provisioned private credentials; it is not a command to
run an unrestricted agent in the current checkout.

`make_native_plan.py` freezes the staged inputs and schedule. Then
`run_native_panel.py --plan PLAN --output NEW_DIRECTORY --credential-master PRIVATE_FILE --probe-result PROBE_RESULT`
runs each registered attempt once. It stops on unknown accounting or unconfirmed cleanup. Never
resume a partial plan by deleting outputs or replacing a failed attempt.

Recompute retained outcomes without execution or network access:

```sh
python3 bench/jev-lifecycle/report_native_pilot.py \
  --plan /path/to/evidence/plan/plan.json --configs /path/to/evidence/plan \
  --runs /path/to/evidence/runs --panel /path/to/evidence/panel/panel.json \
  --output /path/to/new-report
python3 bench/jev-lifecycle/trace_summary.py \
  --plan /path/to/evidence/plan/plan.json --runs /path/to/evidence/runs \
  --output /path/to/new-trace-summary.json
```

The reporter verifies candidate, provider, prompt, configuration, and receipt
bindings and preserves missing or invalid attempts. The trace summary counts
visible tool requests and repeated command hashes. A requested action is not
proof of execution; a repeated command is not automatically wasted work.

The separate [clause review runner](clause-coverage/RUNNER.md) prepares complete
changed source and public contracts for one advisory Choice per task clause.
It freezes all requests before calls, skips oversized mandatory evidence, and
preserves every candidate. Its answers do not accept or repair a native patch.

## Recompute and test

```sh
python3 -m unittest discover -s bench/jev-lifecycle -v
python3.13 -m unittest discover -s bench/jev-lifecycle/clause-coverage -v
python3 bench/jev-lifecycle/report.py \
  --artifacts docs/audits/2026-10-03-independent-efficiency/jev-lifecycle \
  --output /tmp/jev-lifecycle-results.json
```

Recomputation makes no model calls. It checks request and response digests,
then reports every retained attempt, unknown charge, known defect miss, and
accepted-candidate flag. `batching.py --request REQUEST --output NEW_DIRECTORY`
is a live ten-call comparison of four independent questions sent together or
sequentially in both orders. It preserves probability changes and response
disagreements. Use exact retained requests to inspect a result; a rerun through
an unversioned alias can differ.

API time and provider-reported usage do not include engineer time, native
executor time, or machine cost. A socket timeout is not a hard whole-request
deadline. Interrupted calls retain a durable launch intent with unknown cost;
inspect it before admitting more work. Private artifact permissions protect
the exact state and responses during collection. Review artifacts before
publishing them.

The [completed native pilot](../../docs/audits/2026-10-03-independent-efficiency/jev-native-pilot/README.md)
reports the measured savings and failed quality checks together. Its added
trace diagnostic and advisory review are separate from registered native scores.
