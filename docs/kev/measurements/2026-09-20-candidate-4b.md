# Replacement 4B evaluation

## Candidate and policy

This evaluation pins `jaredpalmer/kev-4b` at
`c4bfa11b0dc07691884f2d97f1c4c4c05c92e416`, the shared Qwen3-4B base at
`906bfd4b4dc7f14ee4320094d8b41684abff8539`, and upstream code at
`86db6d924cee68fa9a1319d2c3e6010b9b233d60`. The candidate's artifact lock
and independent fixture directory are
[`kev-4b-c4bfa11`](../../../crates/kev/fixtures/variants/kev-4b-c4bfa11/).
Historical artifacts and measurements keep their original bytes.

The plan was committed as `390ea18f82` before candidate workload calls.
The measurement plan is all open calibration and development items in `coder-turns-v2`, `program-selection-v2`,
`support-v2-three-way`, and `external-v1`: 76, 68, 157, and 160 items.
Questions and labels stay frozen. No calibration maps or thresholds are
fitted. The comparison is the historical 4B at `1a0cb0a` and fresh hosted
`jev-latest`, including the program-selection rows already recorded by
[#9457](https://github.com/OpenAgentsInc/openagents/issues/9457).

Both local models serve Metal bf16 with fp32 heads and fp32 LoRA merging,
eager block-causal attention, a 4,096-token packed limit, and a 4,096 MiB
forward-memory budget. That admits one forward for each variant. They run
serially on the same 128 GiB M5 Max. Oversized requests count as refusals;
raising the bound after seeing responses would be a separate experiment.
The hosted service reports its model name but no verifiable weight digest.

The program-selection acceptance rule is the one frozen in
[the v2 baseline](../../decision-models/2026-09-20-program-selection-v2.md).
For other families, report paired errors, Wilson accuracy intervals, and
paired bootstrap intervals separately by family and split. These are
benchmark descriptions, not independent deployment trials. No locked read
or default replacement follows from this open measurement alone. A material
4B gap can justify a separate 8B control; more parameters do not repair an
input refusal or an ambiguous label.

## Conformance

The pinned upstream fp32 CPU run generated seven request fixtures,
13 encodings, and isolation, permutation, forgery, and packed/separate
probes. The Rust release test run used the actual candidate artifacts,
not the conditional no-weights path. All 44 unit and integration tests
passed: six library tests, five API tests, six conformance tests, five
encoding tests, 18 refusal tests, and four Jev-client HTTP tests.

The maximum Rust/upstream probability difference was `5.335e-6`.
Rust packed/separate differed by at most `1.431e-6`, versus upstream's
`4.172e-7`. The secret probe assigned the secret option 0.0219 in both
the sibling-only and absent conditions, and 0.9166 when it was in state.
The permutation and forged-delimiter controls passed. These small controls
establish runtime agreement, not broad prompt-injection resistance.

Reproduce acquisition and conformance:

```sh
./scripts/fetch-kev-artifacts.sh --root /path/to/artifacts \
  --lock crates/kev/fixtures/variants/kev-4b-c4bfa11/artifact-lock.json
KEV_VARIANT=kev-4b-c4bfa11 \
KEV_ARTIFACT_DIR=/path/to/artifacts/kev-4b-c4bfa11 \
KEV_BASE_DIR=/path/to/artifacts/qwen3-4b \
RUST_TEST_THREADS=1 cargo test --release --locked -p kev \
  --features serve,metal --tests -- --nocapture
```

Gym now accepts `--items` for evaluation as well as recorded views. This
resumes the support reference interrupted after 61 rows, appending only the
96 missing open items. It rejects unknown, locked, or excluded item IDs
before opening a door, preserves suite order and identity, and retains the
existing duplicate-trial guard. All 157 hosted support rows are unique.

## Verification scope

`CARGO_TARGET_DIR="$PWD/target" RUST_TEST_THREADS=4
./scripts/verify-rust.sh --skip-postgres` completed: artifact-acquisition
regressions, workspace formatting, both strict Clippy configurations,
both workspace test configurations, Rust 1.95 workspace and Rust 1.94
standalone Kev compiler checks, and dependency policy. PostgreSQL acceptance
and the relay soak were explicitly skipped, so this is a partial gate.
The real-weight release conformance run above separately enabled Metal
compilation; its correctness tests execute CPU fp32. Workload serving
executes Metal bf16. No FoundationModels inference was claimed by this run.

## Open workload results

The candidate runtime content digest is
`sha256:5622bbba5cae1d35c4572ea382f76cbc4c4c63ef18452c916d80cf9e25a08991`.
All 461 requested items have recorded outcomes for each door. Only action
has refusals: 14 per local model, all `branch_too_long`; hosted Jev
answered every item. There were no missing harness results. Accuracy below
includes refusals in its denominator. Constants are label counts on these
identical open items, not a fitted router.

| Family | New 4B | Historical 4B | Jev | Best constant | New 4B Wilson 95% |
| --- | ---: | ---: | ---: | ---: | --- |
| action | 15/32 | 16/32 | 29/32 | `respond` 31/32 | 0.309–0.636 |
| shell_outcome | 39/44 | 31/44 | 34/44 | `pass` 24/44 | 0.760–0.950 |
| program | 53/68 | 53/68 | 61/68 | `none` 41/68 | 0.667–0.862 |
| routing | 65/80 | 57/80 | 74/80 | `billing` 29/80 | 0.713–0.883 |
| severity | 25/29 | 23/29 | 27/29 | `0` 10/29 | 0.694–0.945 |
| urgency | 41/48 | 37/48 | 46/48 | `no` 24/48 | 0.728–0.928 |
| boolq | 72/80 | 70/80 | 72/80 | `yes` 47/80 | 0.815–0.948 |
| mnli | 71/80 | 70/80 | 66/80 | `entailment` 31/80 | 0.800–0.940 |

The support suite retains independently disputed labels; BoolQ and MNLI
retain external dataset labels. Coder action has one open positive against
31 `respond` turns, and shell labels are recorded execution outcomes.
Authored items and turns from the same sessions are correlated. Wilson and
item-bootstrap intervals describe these samples under an independent-item
approximation; they do not establish a deployment population guarantee.

### Paired errors

The table counts items correct only for the candidate and only for its
reference. Missing or refused outcomes count as incorrect. Intervals are
10,000 paired item bootstrap draws with seed 9458. Exact McNemar p-values
are retained in the JSON reports, without a multiple-comparison correction;
no universal noise floor or significance-based admission is used here.

| Family | Reference | Candidate-only / reference-only | Accuracy difference, paired 95% |
| --- | --- | ---: | --- |
| action | historical | 0 / 1 | -0.031 [-0.094, +0.000] |
| action | jev | 1 / 15 | -0.438 [-0.625, -0.250] |
| shell_outcome | historical | 8 / 0 | +0.182 [+0.068, +0.295] |
| shell_outcome | jev | 9 / 4 | +0.114 [-0.045, +0.273] |
| program | historical | 2 / 2 | +0.000 [-0.059, +0.059] |
| program | jev | 3 / 11 | -0.118 [-0.221, -0.015] |
| routing | historical | 10 / 2 | +0.100 [+0.025, +0.188] |
| routing | jev | 1 / 10 | -0.113 [-0.188, -0.037] |
| severity | historical | 3 / 1 | +0.069 [-0.069, +0.207] |
| severity | jev | 2 / 4 | -0.069 [-0.241, +0.103] |
| urgency | historical | 7 / 3 | +0.083 [-0.042, +0.208] |
| urgency | jev | 0 / 5 | -0.104 [-0.208, -0.021] |
| boolq | historical | 5 / 3 | +0.025 [-0.037, +0.100] |
| boolq | jev | 2 / 2 | +0.000 [-0.050, +0.050] |
| mnli | historical | 3 / 2 | +0.013 [-0.037, +0.062] |
| mnli | jev | 8 / 3 | +0.062 [-0.013, +0.150] |

### Calibration and confident errors

No map was fitted. The retained reports separate calibration and
development for every model and family. Brier is Gym's selected-answer
probability squared error, not multiclass Brier. Refusals are excluded from
probability metrics and retained in accuracy and coverage. These are raw
probabilities, not calibrated execution permissions.

| Family | New 4B ECE / Brier | Historical ECE / Brier | Jev ECE / Brier | Confident errors: new / historical / Jev |
| --- | --- | --- | --- | ---: |
| action | 0.374 / 0.241 | 0.258 / 0.175 | 0.183 / 0.128 | 0 / 0 / 1 |
| shell_outcome | 0.143 / 0.110 | 0.204 / 0.170 | 0.133 / 0.128 | 0 / 0 / 0 |
| program | 0.118 / 0.153 | 0.086 / 0.107 | 0.079 / 0.079 | 5 / 0 / 0 |
| routing | 0.080 / 0.114 | 0.163 / 0.164 | 0.049 / 0.039 | 2 / 4 / 0 |
| severity | 0.144 / 0.133 | 0.134 / 0.097 | 0.102 / 0.043 | 0 / 0 / 0 |
| urgency | 0.031 / 0.110 | 0.085 / 0.153 | 0.171 / 0.069 | 1 / 1 / 0 |
| boolq | 0.089 / 0.089 | 0.091 / 0.096 | 0.053 / 0.076 | 5 / 5 / 2 |
| mnli | 0.112 / 0.100 | 0.097 / 0.109 | 0.083 / 0.106 | 7 / 6 / 0 |

### Shell error direction

The candidate answers `pass` on all 24 true-pass rounds, but also on five
of 20 true-retry rounds. Jev makes three false passes and seven false
retries; historical 4B makes 12 false passes and one false stop. Thus the
candidate's higher accuracy is not a uniform improvement in the decisions
the shell loop consumes. A confirmation rule must price false passes
separately before this family is admitted.

### Program selection

The candidate made no spurious selection on the 31 negative real turns,
but missed the sole positive. On authored inputs it made two spurious
selections, missed 11 requests, and chose one wrong program. Historical
4B missed eight authored requests and Jev missed none. The candidate also
made five errors at probability ≥ 0.9, versus zero in either reference.
It therefore fails the rule frozen in #9457. The 12 new locked cases stay
unscored. Fewer spurious selections did not compensate for lost recognition.

## Recommendation by workload

- **Action: no replacement.** The serving bound refuses 14/32 states, and
  even the answered subset does not establish a useful improvement over
  constant `respond`. More parameters cannot repair the input budget.
- **Shell outcome: further evaluation.** 39/44 beats both measured
  references and the constant. The paired interval versus Jev includes
  zero, and 44 rounds from a few sessions cannot admit a new execution
  policy. Use this pinned 4B as the next local candidate for an independently
  labeled confirmation set.
- **Program selection: no replacement.** It fails the predeclared open
  rule, with more missed authored requests and confidently wrong answers.
- **Support routing: further work.** It improves on historical 4B but
  remains below Jev by nine items. Urgency and severity also need independent
  confirmation; neither result admits a local replacement.
- **External labels: retain as a transfer control.** BoolQ ties Jev and
  MNLI has five more correct answers, but both paired intervals include
  zero. The candidate has more confidently wrong answers than Jev. These
  datasets do not authorize a Coder routing change.

No default changes, locked reads, or training runs were made. An 8B control
was not run: the immediate Coder blockers are input coverage, rare-class
coverage, and the program question contract. A broader model-size search
would not resolve those measurement limits. The support gap is recorded
for later targeted comparison; it does not justify making 8B the default.

## Request sizes

The Rust `workload_shapes` example uses the serving tokenizer, renderer,
and encoder without a forward pass. It records token counts and hashes
of encoded state, with no raw request content. Every scored Gym request
has one question; the separate HTTP benchmark also measures five.

| Family | State tokens: min / median / max | Packed tokens: min / median / max | Over 4,096 packed tokens |
| --- | ---: | ---: | ---: |
| action | 100 / 3625 / 6001 | 199 / 3724 / 6100 | 14/32 |
| shell_outcome | 124 / 661 / 3486 | 205 / 742 / 3567 | 0/44 |
| program | 4 / 21 / 757 | 164 / 181 / 917 | 0/68 |
| routing | 7 / 12 / 21 | 57 / 62 / 71 | 0/80 |
| severity | 7 / 11 / 15 | 42 / 46 / 50 | 0/29 |
| urgency | 8 / 11 / 17 | 42 / 45 / 51 | 0/48 |
| boolq | 37 / 125 / 555 | 75 / 163 / 593 | 0/80 |
| mnli | 14 / 41.5 / 121 | 60 / 87.5 / 167 | 0/80 |

The state encoder has its own 8,192-token ceiling. Its truncation flag is
retained in the shape records; those oversized action requests were
refused by this run's packed-token limit, not scored on truncated state.
On the seven conformance requests, Metal bf16 differed from upstream CPU
fp32 by at most 0.0116723 probability, with no argmax changes. All 13 raw
encoding controls are retained in `metal-bf16-probes.json`.

## Reproduce the workload comparison

The stores are `crates/gym/results/<suite>-kev-update-<door>.jsonl`,
where door is `candidate`, `historical`, or `jev`. Program selection reuses
its already frozen `program-selection-v2-historical-4b.jsonl` and
`program-selection-v2-jev.jsonl` references. Each store's receipt chain
verified through `gym compare`; reports are retained in
[`data/candidate-4b/`](data/candidate-4b/).

```sh
kev-serve --adapter-dir /path/to/artifacts/kev-4b-c4bfa11 \
  --base-dir /path/to/artifacts/qwen3-4b --default kev-4b-c4bfa11 \
  --device metal --dtype bf16 --port 18454 \
  --max-tokens 4096 --memory-budget-mib 4096
gym eval --suite crates/gym/suites/coder-turns-v2.json \
  --door kev-4b-c4bfa11=http://127.0.0.1:18454 --timeout 300 \
  --record /path/to/new-candidate-store.jsonl
gym compare --suite crates/gym/suites/coder-turns-v2.json \
  --store /path/to/new-candidate-store.jsonl
python3 scripts/report-kev-candidate.py crates/gym/suites/coder-turns-v2.json \
  candidate=/path/to/new-candidate-store.jsonl \
  historical=crates/gym/results/coder-turns-v2-kev-update-historical.jsonl \
  jev=crates/gym/results/coder-turns-v2-kev-update-jev.jsonl
```

Use a fresh result path for an independent repeat. To resume an interrupted
run, derive a one-ID-per-line file from the expected open IDs absent from
its store, then pass that file with `gym eval --items`. Never select items
by their observed correctness. `report-kev-candidate.py` rejects duplicate
or locked rows and mismatched suite, question, partition, or model identity.
It preserves unanswered items in the full denominator.

## Quiet-host serving measurements

The fresh eager server ran on the same 128 GiB M5 Max, on AC power, with
Metal bf16, an fp32 head, and the bounds above. The process monitor detected
no other model measurement client or Cargo build during this run. Normal
desktop services remained; the retained host snapshots state that condition.
The run spans 21:18–21:20 UTC on 2026-09-20. Build and host identities are in
`serving-build.json`.

Each case has three warm-up requests followed by 20 measured serial requests
on one persistent HTTP connection. Percentiles use nearest rank. All 184
requests succeeded. The short document has 26 state tokens; the long one
has 578. New states append a changing ticket reference, adding nine tokens.
Each question has three options. Reported input tokens exclude padding;
this baseline uses none. No state cache is present.

| State | Questions | State reuse | Packed tokens | HTTP p50 / p95 (ms) | Model p50 / p95 (ms) |
| --- | ---: | --- | ---: | ---: | ---: |
| Short | 1 | repeated | 55 | 83.5 / 84.7 | 83.1 / 84.3 |
| Short | 1 | new | 64 | 86.8 / 87.6 | 86.3 / 87.2 |
| Short | 5 | repeated | 171 | 192.9 / 219.0 | 192.2 / 218.4 |
| Short | 5 | new | 180 | 204.6 / 206.2 | 203.9 / 205.6 |
| Long | 1 | repeated | 607 | 635.1 / 682.4 | 634.3 / 681.5 |
| Long | 1 | new | 616 | 752.1 / 779.3 | 751.2 / 778.5 |
| Long | 5 | repeated | 723 | 952.4 / 1018.2 | 951.4 / 1017.1 |
| Long | 5 | new | 732 | 925.2 / 985.8 | 924.1 / 984.8 |

Model time covers mask construction, forward inference, and pointer readout;
HTTP time additionally includes the local transport and server request path.
All three warm-up timings and raw responses are retained in
`http-eager-quiet.json`. This is a small, ordered shape sweep, not a tail
latency guarantee or a matched request-shape comparison with upstream's
published benchmark. New and repeated states differ in length; their
timing differences do not measure a cache speedup.

The fresh process reached **9.13 GiB maximum RSS** and **22.34 GiB peak macOS
footprint**, including loading and these requests. The full earlier quality
process reached 24.43 GiB footprint while serving larger Coder requests.
Neither is a 32 GiB host proof or a per-request working-memory measurement.
The reserved forward estimate is 3,424 MiB, admitting one variant forward
under the 4,096 MiB budget; the host-wide cap is two.

The first timing sweep overlapped another task's Lev measurements. Its
`http-eager-contended.json` remains explicitly excluded from quiet-host
acceptance. A separate startup attempt aborted when the monitor detected
a build, before any timing requests were sent.
