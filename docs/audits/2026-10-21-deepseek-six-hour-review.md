# DeepSeek-V4.1 six-hour commit audit

Reviewed by Codex on 2026-10-21. Window: **07:03:36–13:03:36 UTC**
(02:03:36–08:03:36 America/Chicago). Reviewed `main` at
`30ba21cb1efab4097872a7b49e438e1d947924e7`.

The native group can produce complete answers, but the build and launch path
does not yet establish which GPU arithmetic and executable a qualification
result covers. Three high-priority findings concern that identity. Six more
findings concern billing, restart readiness, prefill bounds, backpressure,
prompt modes, and the synthetic test data.

The [commit inventory](2026-10-21-deepseek-six-hour-review/commits.json)
contains all 14 other-agent commits in the window. Selection uses committer
time with `git log --since-as-filter`, so rebased commits are included even
when author timestamps differ. The two preceding Codex review commits are
excluded as review targets; their policy integration is checked separately
below. The generated throughput update is also accounted for below.

## Findings

P1 means fix before relying on a newly qualified build for launch. P2 means a
correctness or verification gap that needs a bounded follow-up. An executed
fixture demonstrates local behavior; source inspection identifies a path that
still needs its stated integration check. Neither is a new full-model run.

| ID | Priority | Finding | Evidence |
| --- | --- | --- | --- |
| F1 | P1 | Switching build architecture can reuse previously compiled CUDA objects. | Source and Cargo's documented rebuild rules; two-GPU rebuild not run |
| F2 | P1 | Launch does not enforce the worker, library, and head build hashes. | Startup accepts and runs a replacement worker whose hash differs from `BUILD.json` |
| F3 | P1 | CUDA silently ignores the accepted arithmetic option. | CLI-to-executor source trace |
| F4 | P2 | Flex-start settlement still overcounts normal runs and can incorrectly waive a start/delete race. | Actual guard functions with isolated fake-cloud fixtures |
| F5 | P2 | An unsuccessful repeat startup leaves a successful readiness marker. | Actual startup script, successful boot followed by a failed boot |
| F6 | P2 | The configured context size exceeds what an unchunked CUDA stage message can carry. | Real wire dimensions and codec bounds |
| F7 | P2 | Direct-link backpressure ends at an unbounded worker inbox. | Actual receiver-to-executor source trace |
| F8 | P2 | The serving adapter cannot request the oracle's chat-mode code prompt. | Existing adapter limitation exposed by the new CUDA path |
| F9 | P2 | The new synthetic generator produces values outside its stated range. | Compiled, unchanged generator extracted from the source |

### F1: architecture changes do not invalidate the shared CUDA build

**Relevant commits:** `2d929ceba4` and the subsequent stage-build changes.

The [build script][stage-build] keeps one persistent source folder and one
`/cache/target`, and selects a GPU through `CUDAARCHS` and `PSI_CUDA_ARCH`.
The [backend build script][cuda-build] reads both variables at runtime, but
only declares `NVCC` and `PSIONIC_CUDA` as environment rebuild inputs.
The persistent source sync deliberately leaves unchanged files' timestamps
alone. Changing only `--arch` therefore need not rerun the native compilation;
the next upload can put the previous architecture's kernels under a new
`sm_NN` path and write the requested architecture into `BUILD.json`.

This is a preexisting backend invalidation omission made consequential by the
new build script's shared cache. Cargo documents explicit
[environment change detection](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-env-changed);
runtime `std::env::var` reads do not get the automatic tracking of `env!`.
I did not perform a two-architecture CUDA build or establish that an existing
uploaded binary is mislabeled.

**Fix and acceptance:** track both architecture variables, make the effective
architecture explicit for test builds, and inspect the resulting fatbinary.
Build the same commit for SM89, then SM120, then SM89 with the existing cache;
verify actual embedded targets and execute on the corresponding devices.
Also load the repository's source/compiler-keyed Cargo build environment in
the container; the new script currently bypasses that helper. A separate
target directory for every task is not the remedy.

### F2: a model pin does not bind the executable that serves it

**Relevant commits:** `2d929ceba4`, `3e3372ee0c`, `b3162d369d`.

The builder writes a worker SHA-256, but [startup][startup] never reads
`BUILD.json` or checks that hash. It copies the worker and CUDA libraries
from mutable object paths. The head executable is also copied without an
expected build hash. The startup architecture check derives its expectation
from the URL, not the executable. The [worker's hello][worker-hello] reports
the constant `psionic-v41-stage/1+cuda`, so different builds or arithmetic
profiles can announce the same runtime string.

The local fixture keeps the expected worker hash unchanged in `BUILD.json`,
replaces the worker bytes at the same URL, and boots successfully. The
installed launcher executes `replacement-worker`. This tests pin enforcement
with shell substitutes; it does not demonstrate a compromised bucket or
attest a remote GPU. The checkpoint shard checks are separate and remain
useful.

**Fix and acceptance:** bind expected worker, head, and library hashes and the
effective numerical profile to the qualified execution configuration. Verify
them before publishing readiness, install a complete build atomically, and
compare the build identity across stages. Refuse a replaced worker, library,
head, or profile even when the model pin and protocol version still match.
Retain object generations or equivalent immutable artifact references with
the qualification report.

### F3: CUDA accepts an arithmetic selection that it never applies

**Relevant commit:** `2d929ceba4`.

[Options parsing][worker-options] accepts `--arithmetic f32|bf16|bf16-tp8`
alongside `--backend cuda`. The [CUDA dispatch][cuda-dispatch] does not read
`options.arithmetic`, and its stage configuration has no corresponding
selection. The CUDA branch of the [reference command][worker-main] bypasses
arithmetic parsing too. CPU dispatch does consume the option.

An operator can therefore request `bf16-tp8` while executing the existing CUDA
arithmetic, with the same runtime announcement. This is not proof that the
current one-machine run claimed TP8: its receipt explicitly says otherwise.
It is a configuration bug that can invalidate a future comparison's label.

**Fix and acceptance:** reject unsupported CUDA arithmetic selections before
loading weights, or implement and report them. Test every backend/arithmetic
combination, including an invalid value on the reference command. Qualification
reports must describe the effective profile rather than echo requested flags.

### F4: the flex-start billing fixes cover only part of settlement

**Relevant commits:** `d85e58b2f0`, `20ec8240ef`.

There are two distinct cases in [the settlement code][budget-settlement]:

1. **A VM actually starts and later stops through `sync`.** Manual `stop`
   reads `lastStartTimestamp`; `sync` neither requests nor saves it. A fixture
   requests at 08:00, starts at 09:00, and stops at 10:00. At $64.44/hour,
   `sync` records **$128.88 instead of $64.44**, counting the queue hour.
   Automatic expiry and preemption use this reconciliation path too.
2. **A pending VM starts between describe and deletion.** Manual `stop`
   decides `never_started` from the pre-delete `PENDING`/`PROVISIONING`
   snapshot and writes a zero-cost stop after deletion without checking the
   terminal lifecycle evidence. Fault injection of a start during that gap
   still produces `reason=never-started` and zero. This is a demonstrated
   classification race in a fixture, not an observed GCP billing event.

The existing 26 guard tests pass. Their new cases cover manual settlement
after a known start and a failed queued insert, but not these transitions.

**Fix and acceptance:** share lifecycle settlement across manual stop, sync,
and adoption; use the matching instance/operation identity and actual start
and end times. Keep uncertain spend reserved until terminal evidence resolves
it. Test successful queued starts followed by expiry/preemption, a start/delete
race, missing timestamps, reused names, and aged-out operations. Historical
closed records also need an explicit correction path if they already contain
the old estimate. This audit does not settle the disputed $136.53 invoice.

### F5: a failed second startup still looks ready

**Relevant commits:** `2d929ceba4`, `0019ec6d4e`, `76934e3fbb`.

[Startup][startup] writes `$root/ready` after success but never removes an
existing marker before replacing files or copying weights. Both `stage-run`
and the [head SSH readiness probe][head-ssh] trust that marker.

The fixture boots successfully, then repeats startup with a manifest whose
contents fail the expected hash. The second startup exits 1, but `ready`
survives and `stage-run` still invokes its worker. On a persistent disk after
a reboot or retry, the head can race incomplete installation and consume its
restart budget while weights are still copying. The real worker may refuse
the inconsistent files; this fixture does not demonstrate serving corrupt
weights.

**Fix and acceptance:** invalidate readiness before mutation; stage the full
installation and publish a marker bound to the manifest/build only after all
checks succeed. Test a second boot with a failed download, bad manifest,
changed build, and interrupted installation. Include the head's readiness
probe in that repeated-boot test, not only the worker wrapper.

### F6: context capacity and transport capacity disagree

**Relevant commit:** `2d929ceba4`, integrating the CUDA executor with existing
stage-codec limits.

The worker defaults to an 8,192-token context and sets `max_step_tokens` to
the same value. [Stage 0 submits the full prompt as one step][worker-prefill].
CUDA [forwards F32 residuals][cuda-wire] with shape `[tokens, 4, 5120]`, but
the [codec][stage-codec] limits both a section and a message to 256 MiB.

At 3,277 tokens, the residual alone needs
`3277 * 4 * 5120 * 4 = 268451840` bytes, already above 268435456. Headers,
tokens, pre-mixes, KV rows, and selections make the actual usable bound lower.
The 20k oracle prompt also exceeds the codec's 8,192 tokens per step and the
default context capacity. Raising `--max-context` alone cannot fix the wire.
The recorded 1,072-token run does not exercise this boundary.

There is also a poor failure path: [direct forwarding][worker-forward]
handles encode refusal with `self.refused`, which logs locally rather than
sending the front a typed refusal. The front waits for its step timeout and
can replace a healthy stage for a request-size problem. The buyer's byte
limit is not a token or stage-payload admission check.

**Fix and acceptance:** establish and advertise a checked input/output/context
budget; reject an oversized request before GPU execution with a request-limit
error. Add bounded chunked prefill if the supported profile requires longer
inputs. Check real-dimension payload sizes at every boundary and verify that
oversize input does not restart a healthy worker. Qualify a changed chunk
schedule separately. Until then, do not claim that the served profile covers
the original 20k case.

### F7: the real worker removes the link's executor backpressure

**Relevant commit:** `2d929ceba4`.

The direct transport has bounded sender queues, but the
[worker's receiver][worker-receiver] drains frames into `std::sync::mpsc::channel`,
an unbounded event queue. The compute thread processes them later. The
32-frame bound applies to a different queue after event dequeue, so it does
not cap this handoff. A slow or stuck executor can therefore keep accepting
large authenticated frames into host memory rather than pushing back on the
upstream link.

The current honest serial flow normally has little queued state. This is a
source-confirmed limit gap for a faulty peer or future overlapping prefill,
not a measured memory-exhaustion event in the reported run. Fake-stage link
backpressure tests do not exercise this worker's handoff.

**Fix and acceptance:** bound queued data by bytes as well as frame count and
keep cancellation/fencing serviceable under pressure. Stall the actual worker
executor, send valid frames through the authenticated link, and measure a
bounded queue/RSS while the sender blocks or fails within its deadline.
Repeat cancellation and recovery in that state.

### F8: the existing adapter always chooses thinking mode

**Integration gap, not newly introduced in this window.** The CUDA commit
reuses an adapter that cannot yet meet the paired p2 requirement.

[Prompt encoding][worker-encoding] constructs `Conversation::default()` and
copies only message roles and content; the [conversation default][conversation-default]
has `thinking_mode=true`.
[Answer parsing][worker-parser] likewise builds default thinking-mode options.
There is no chat/thinking selection in the stage CLI or the group prompt
payload. The one-machine receipt correctly identifies its code answer as a
different mode from oracle p2.

**Fix and acceptance:** carry an explicit supported mode through prompt
construction, stage state/settings, and final parsing, then compare exact
rendered IDs with the corresponding oracle prompt. Alternatively freeze a
separate thinking-mode code reference and label the narrower supported
profile; it cannot substitute for an unrun chat-mode comparison. Verify both
fresh requests and conversation continuation. Merely rerunning the current
command with the same code question will not supply paired chat-mode evidence.

### F9: the synthetic RNG's claimed range is wrong

**Relevant commit:** `2d929ceba4`.

The [new generator][synthetic-rng] takes 53 random bits with `>> 11`, then
divides by `2^52` and computes `2*x-1`. Its intended interval is `[-1, 1)`,
but the expression reaches approximately `[-1, 3)`, with positive bias.
The same function perturbs weights described as near one, so their intended
distribution changes too.

Compiling its unchanged implementation with seed 11 and 4,096 draws produces
2,063 values outside the declared interval, maximum 2.9971955, and mean
0.993464868. This does not invalidate a split-versus-unsplit equality on those
same weights, but it weakens the fixture's claimed input coverage and alters
cancellation and rounding stress.

**Fix and acceptance:** normalize the 53-bit result by `2^53`, or extract the
intended 52 bits. Test range and sign coverage with fixed seeds, then rerun
the affected synthetic CPU/GPU/process comparisons and identify the changed
fixture in future reports. Keep original-weight kernel results separate.

## Verification and planning still missing

- **A required CUDA/direct/lifecycle matrix.** The new CUDA process tests use
  the simple front transport and compare with an unsplit synthetic CUDA stage.
  Pylon's direct-link lifecycle tests start the worker with its default CPU
  backend. The one-machine report explicitly leaves real-group DS0 and G7
  checks unrun. Combine CUDA, direct transport, crash/freeze/cancel, bounded
  queues, and another request after recovery; repeat on both W12 placements.
- **A test mode that refuses missing prerequisites.** Root
  `group_stage_worker` tests return successfully when `PSIONIC_V41_STAGE` is
  absent or invalid. The CUDA crate's `PSIONIC_REQUIRE_CUDA` is useful, but
  does not enforce the root integration suite. Require the exact verified
  binary for qualification runs, report executed/skipped counts, and fail a
  purported qualification when the worker or GPU is absent.
- **The v2 observation interface.** Current CUDA `reference` returns tokens
  and text, and the worker calls `stage.step(..., false)` rather than
  exporting full logits. Direct common-prefix/full-distribution qualification
  requires a diagnostic interface using the serving kernels, cache, and
  schedule. It also needs the mode work in F8. The policy review specifies a
  requirement; it did not implement this interface or pass its thresholds.
- **Reproducible build inputs.** The builder uploads placement files from
  the caller's current checkout, even with `--commit OTHER`. Fetch them from
  that commit or explicitly identify them as separate hashed inputs. Record
  container-image digest, library hashes, effective target/profile, source
  tree, and artifact references together; the requested commit and URL alone
  do not describe the build.
- **GPU numbering under an inherited visibility mask.** CUDA only applies
  `--gpu` when `CUDA_VISIBLE_DEVICES` is absent. With a multi-device mask,
  every process otherwise selects the first visible device. The tests
  deliberately share one visible GPU, so they cannot check this case. Define
  logical versus physical indexing and test an inherited multi-GPU mask.
- **Prefill and parsing costs.** The 35.6-second prefill used a 75-second
  step limit; the default step limit is 30 seconds. Freeze W14's supported
  context/deadline before optimizing. `AnswerParser::push` also clones and
  reparses the entire answer prefix each token; measure that cost at the
  selected output cap rather than treating all post-first-token time as GPU
  execution.

## Findings already reported, and claims that are appropriately limited

The original-weight CPU report in `93496b2929` sets `g1_passed=false`, retains
all 16 prompt/mode rows, and distinguishes complete short runs from long
first-failure runs. The one-machine receipt in `9b624aa209` explicitly says
oracle parity, DS0 runtime checks, and G7 faults were not run. Those are
unfinished gates, not evidence that the author falsely reported passing them.
The typed CPU/CUDA runtime distinction and per-stage shard/hash loading are
useful integration improvements; F2 concerns the unbound executable, not a
missing checkpoint hash check.

The incorrect inference from exact oracle repeatability to exhaustive CPU
hardware emulation was addressed by `9d873415d7`. The Fibonacci overflow
boundary failure was already reported in `30ba21cb1e`; this audit does not
count it as a newly discovered finding. A plausible sky answer remains useful
smoke evidence, not numerical or task-quality acceptance.

There is one **Codex policy integration omission**: my `9d873415d7` review
changed the completion checklist but left the
[master delegation after-close rows](2026-10-16-master-delegation.md#after-close-checks)
for #12398 and #12394 waiting for the old CPU arithmetic order. Update those
rows to the direct-runtime v2 contract and independent lifecycle checks.
Their original wording in the other agent's commit preceded the policy change;
the later inconsistency is mine.

The generated update `3f7a76cdf9` adds five lines to the throughput document
and no serving behavior. It was inspected as a generated-doc change, not
treated as DeepSeek qualification or independently re-benchmarked.

## Follow-up order

1. Fix F1–F3 before collecting a new qualification result: make the actual
   target, executable, libraries, and arithmetic match the recorded identity.
2. Fix billing settlement and repeated-boot readiness (F4–F5). Add their
   regression cases before relying on another long cloud session.
3. Define a supported context/mode contract; fix or explicitly bound F6–F8.
   Correct F9 before using the synthetic suite as new evidence.
4. Finish the direct GPU comparator and the required CUDA/direct/fault matrix,
   then run frozen numerical and task-quality qualification. W3 can collect
   provisional measurements in parallel; changes above may require repeating
   affected measurements. Keep the current #12405, #12399, #12401, and #12390
   ownership with their existing agents.
5. Complete both placements and the declared deadline/answer profile before
   W16 visibility and epic acceptance. Child issue closure alone passes none
   of those checks.

## Reproduction and limits

The [local results](2026-10-21-deepseek-six-hour-review/results.json) retain
source hashes, fake-cloud billing outputs, startup outputs, and the compiled
RNG check. The executable
[reproduction script](2026-10-21-deepseek-six-hour-review/reproduce.py) runs:

```sh
python3 docs/audits/2026-10-21-deepseek-six-hour-review/reproduce.py \
  --output "$(openagents scratch)/deepseek-six-hour-audit.json"
```

At the reviewed head, the existing guard suite passes **26 tests**; the added
probes expose the findings above. The script uses the guard's existing fake
cloud and executes the actual startup script against temporary local metadata
and a fake bucket. It compiles only the extracted RNG with pinned Rust 1.97.1.
Temporary fixtures live under `openagents scratch` and are removed afterward.
No GCP request, model run, production service, or owner account is involved.

The cached code graph was queried first; its `32d2b71eaa` graph had no nodes
for the new stage/cloud modules. Impact tracing therefore used scoped source
reads and the actual commit diffs. Architecture-cache behavior, CUDA arithmetic
dispatch, queue bounds, and prompt-mode propagation are source findings; the
audit does not claim a new GPU, two-machine, or billing-console verification.
The 20k CPU/GPU runs and all qualification gates retain their prior status.

[stage-build]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/scripts/pylon/v41-stage-build.sh#L123-L159
[cuda-build]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-backend-cuda/build.rs#L29-L30
[startup]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/scripts/pylon/v41-stage-startup.sh#L91-L156
[worker-hello]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/serve.rs#L524-L541
[worker-options]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/lib.rs#L235-L310
[worker-encoding]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/lib.rs#L363-L419
[conversation-default]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/vendor/deepseek-recipe/deepseek-recipe-core/src/conversation.rs#L40-L50
[cuda-dispatch]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/cuda.rs#L579-L616
[worker-main]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/main.rs#L80-L124
[budget-settlement]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/scripts/dev/cloud-budget.py#L701-L783
[head-ssh]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/scripts/pylon/v41-head-ssh.sh#L17-L33
[worker-prefill]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/serve.rs#L746-L791
[cuda-wire]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/cuda.rs#L139-L163
[stage-codec]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-models/src/stage_state.rs#L449-L457
[worker-forward]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/serve.rs#L1017-L1022
[worker-receiver]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/serve.rs#L354-L359
[worker-parser]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/lib.rs#L967-L1014
[synthetic-rng]: https://github.com/OpenAgentsInc/openagents/blob/30ba21cb1efab4097872a7b49e438e1d947924e7/crates/psionic/crates/psionic-v41-stage/src/cuda/synthetic.rs#L153-L168

## Review of the remediation response (2026-10-21)

Reviewed the response against main at `8447cdff74828484827c1a16ec8090baa124f576`,
including #12407, #12408, #12409, and the new W12/W14 receipts. The fixes are
substantial, but **“acted on every item” is more accurate than “every item is
fixed.”** Build identity, VM identity, and output decoding still need work.
This addendum leaves the original findings and results unchanged.

### R1 — P1: Finish binding the complete build through qualification and launch

The startup checks now reject an individual worker or library that differs
from `BUILD.json`, and an explicit `--worker-build` pins that document. These
fix the original unchecked download. However:

- [`build_identity()`](https://github.com/OpenAgentsInc/openagents/blob/8447cdff74828484827c1a16ec8090baa124f576/crates/psionic/crates/psionic-v41-stage/src/lib.rs#L530-L550)
  reports the executable hash and target, without the library or build-record
  hashes. The group compares these announcements with the first stage's
  announcement. The
  [qualification report](https://github.com/OpenAgentsInc/openagents/blob/8447cdff74828484827c1a16ec8090baa124f576/crates/psionic/crates/psionic-v41-stage/src/qualify.rs#L1104-L1118)
  calls the executable hash `stage_build_sha256`. Two bundles with the same
  executable and different CUDA libraries have the same reported identity.
- Cloud build pins remain optional. The claim that launches pin exact builds
  describes an available option, rather than enforced launch behavior.
  The calibration setup also downloads `BUILD.json` and libraries without
  checking their hashes against an independently supplied build pin.
- `stage-run` rechecks files against the installed `BUILD.json`, but does not
  rehash that document against the build hash in its path and ready marker.
  In the local fixture, bundle A passed startup with its explicit pin. After
  replacing its installed library and updating its installed `BUILD.json`
  consistently, `stage-run` still succeeded under A's ready marker.

The [follow-up results](2026-10-21-deepseek-six-hour-review/follow-up-results.json)
retain both startup fixtures. They use a shell worker and fake libraries;
they demonstrate acceptance checks, not CUDA execution or a running-process
exploit. The omitted group/report fields are source-inspection findings.

Before accepting G1, include the verified bundle hash in worker announcements,
group configuration, qualification results, and launch records. Compare it
with the qualified profile's expected hash, not only with the first worker.
Require those pins for qualification and qualified serving; explicitly label
any development launch that omits them. Recheck `BUILD.json` itself before
starting a worker. Test identical executables with different libraries and a
self-consistent replacement of an installed build record and library.

### R2 — P2: Reused VM names can still supply the wrong timestamps

The original sync calculation and start/delete race have regression tests,
and all **37 guard tests pass**. The new reused-name test places the
replacement one hour later, outside `same_machine`'s 30-minute window. It
misses replacements inside that window.

[`describe()`](https://github.com/OpenAgentsInc/openagents/blob/8447cdff74828484827c1a16ec8090baa124f576/scripts/dev/cloud-budget.py#L638-L655)
compares creation time but ignores a known mismatch between the ledger's
instance ID and the described instance ID.
[`settle()`](https://github.com/OpenAgentsInc/openagents/blob/8447cdff74828484827c1a16ec8090baa124f576/scripts/dev/cloud-budget.py#L735-L759)
filters operation events by ID, then borrows the unvalidated describe
snapshot's start and stop timestamps anyway.

The executed fixture records instance `111` at 08:00, whose insert fails at
08:05. Instance `222` reuses the name at 08:10 and stops at 09:00. Settlement
reports instance `111` with instance `222`'s timestamps and **$53.70** at the
fixture rate of $64.44/hour, although the failed request never started.
This is a local reproduction, not a claim about an actual GCP charge.

Reject mismatched instance IDs before using describe timestamps, and resolve
the instance ID from the matching creation operation when the ledger lacks
one. Add the rapid-reuse case to the suite. Manual stop should also refuse to
delete a replacement instance when it discovers that the ledger's machine is
gone. The billing-console check remains an owner step; passing fixtures do
not confirm an invoice.

### R3 — P2: Output decoding still processes the full history on every token

`AnswerParser` now keeps a live parser, and its whole-answer replay was
removed. That is a useful fix. But
[`Server::run_step`](https://github.com/OpenAgentsInc/openagents/blob/8447cdff74828484827c1a16ec8090baa124f576/crates/psionic/crates/psionic-v41-stage/src/serve.rs#L1192-L1201)
still calls `TextStream::push` before the answer parser. That method appends
the token, decodes `self.ids` in full, and checks the full emitted prefix on
every call. At 4,096 tokens it submits 8,390,656 token IDs to the decoder
across those calls. The remaining cost is quadratic in answer length.

Use an incremental decoder for the raw-text stream, or avoid that duplicate
stream when the structured parser supplies the answer. Keep the split-UTF-8
and special-token tests, then measure end-to-end parsing at the output cap.
This is a source-level complexity finding; this review did not measure its
latency on the GPU. The response should say “incremental answer parser,”
without implying that all repeated-prefix processing is gone.

### Coverage and remaining evidence

- Independently ran the **37 billing tests** and **16 CPU stage tests**:
  `answer` (8), `budget` (3), `handoff` (1), and `identity` (4). All passed.
  The chat test matches the oracle p2 IDs. The handoff test uses the real
  serving loop and a gated CPU executor, exercises backpressure, then checks
  cancellation, fencing, and a new request.
- F1, F3, F5, and F9 have concrete source changes and regression coverage.
  This review inspected the architecture rebuild record but did not repeat
  the other agent's three-build 4080 check or its CUDA tests. They remain
  that agent's hardware evidence.
- The **3,022-token receipt remains disputed**. The checked four-stage CUDA
  budget is 2,872 tokens, and the local budget test passes. Retain the older
  measurement as historical evidence with the dispute attached; do not use
  its token count to certify the current context limit. Recover its exact
  prompt IDs and actual encoded boundary lengths first. Recounting retained
  inputs does not require a new GPU session; new timing does. The receipt
  does not retain enough prompt data to resolve this here.
- The new queue, parser, build, and admission changes postdate the W12/W14
  GPU runs. Repeat the required direct-link fault matrix for both placements
  and the answer/deadline measurements on the same pinned SM120 bundle that
  passes numerical qualification. After-close tracking is appropriate under
  the issue policy, but does not pass G1, G2, or final launch acceptance.
- G1 v2 remains in progress. Its machine script still schedules the 20k
  calibration case although the current unchunked profile cannot carry it.
  Report it as unsupported/missing coverage until a separately qualified
  prefill schedule exists. Do not interpret the bounded short-context
  profile as 20k qualification.

### Correction to this audit's reproduction command

The other agent correctly identified that the old reproduction command
fails against newer startup code. The script used the working tree; it did
**not** automatically check out its recorded review revision. That omission
was mine. It now checks the retained source hashes before running, gives a
clear error for changed sources, and accepts `--source-root PATH` for a
checkout of `4dab114ee5`, which contains both the historical source and audit.
The original `results.json` stays unchanged.

For the follow-up fixtures on current source, run:

```sh
python3 docs/audits/2026-10-21-deepseek-six-hour-review/follow-up-reproduce.py \
  --output "$(openagents scratch)/deepseek-follow-up.json"
```

The follow-up script records the tested commit and source hashes and uses
isolated local metadata, fake storage, and mocked lifecycle records. It makes
no cloud requests and starts no model or production service.
