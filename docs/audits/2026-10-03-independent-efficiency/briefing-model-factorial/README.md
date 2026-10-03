# Model selection and structured briefings

This experiment tests two changes separately: executing a task with Sonnet
instead of Opus, and supplying a deterministic source briefing. Four balanced
blocks compare all four combinations on the same historical issue. The
[prospective protocol](plan.md) fixes the order, models, checks, one-repair
limit, and cost and time thresholds before scored calls.

The original 16-run panel completed but is **not valid for the registered
comparison**: the final run initialized Claude CLI 2.1.288 instead of the
required 2.1.287. The coordinator retained the result and stopped. Its
[complete report](results-original/README.md) retains every patch, check,
and cost. Calibration and the original registration were published before
warmups and scored calls.

The protocol permits replacing a whole block after a confirmed infrastructure
defect. A [separate prospective registration](replacement-registration.json)
and [amendment](replacement-block-4.md) replace all four positions in block 4
with a privately pinned 2.1.287 executable. The first three blocks
and original warmups remain unchanged. The original block stays visible and
charged; no outcome from it enters the replacement comparison.

The [original source review](quality-review-original.md) also identifies an
unexecuted concern in the last candidate: its missing, extensionless rename
path heuristic can classify an ordinary renamed file as a directory. This
case is outside the frozen checker's coverage. It remains visible alongside
the version failure. The replacement is triggered by the registered version
rule, and its checker remains unchanged.
The [independent numerical audit](independent-audit-original.json) confirms
complete original accounting and rejects all five original cost gates while
the CLI binding is invalid.

## Why this follows the previous rounds

The first replay produced no accepted patch. The second round supplied full
instructions and an external verification-and-repair loop; its development
panel accepted every patch but did not reach the registered 20% cost saving.
Its unchanged held-out panel also misses the cost gate, with 12.6% lower
median estimated cost and 7.5% lower recorded endpoint time. Those observations
remain published separately, including broader quality concerns that the
frozen checker does not cover.

The new thesis is that a less expensive executor can complete a bounded issue
under the same deterministic checks. The structured brief may remove some
source discovery, but its contribution must be measured against Sonnet alone.
The four arms distinguish these explanations. A combined win does not by
itself establish that briefing helped.

## Historical task

The reserve is [issue #9989](https://github.com/OpenAgentsInc/openagents/issues/9989):
make direct catalog notifications work when a new transcript directory appears
on Linux. The source is `aeb7f9fb19e0d56c13400702894172ae472e3bcf`; the original
fix is `fd305b01df5add38e2bd0ef3f862ee85cc9950c3`, whose parent is that exact
source. The retained fresh child assignment took 312.434 seconds. Its tools,
coordination, and cross-platform duties differ from this experiment, so that
historical elapsed time is context, not a matched control.

The [display copy of the task](task/task.md) removes owner paths, a hostname,
and historical model attribution. Its provenance record hashes both versions.
All scored arms receive the same complete private task and applicable
instructions. Original solutions and independent checks stay outside their
source exports. The operational override uses file tools and isolated Linux
checks; this panel does not fulfill the original request for repeated macOS
checks or deployment.

## Frozen preparation

The packer was frozen before its implementer saw the reserve identity or
checker. It ran once on the task under its registered rules. The resulting
[brief](task/treatment.md) is 14,025 bytes, including 12,232 bytes of source.
Warm preview took 0.220 seconds; building a fresh index separately took 1.425
seconds. These are single samples with operating-system caches uncontrolled,
not latency percentiles or a guarantee for arbitrary GitHub issues.

The brief contains the watcher subscription, delivery, and relevance logic,
with bounded same-file syntax dependencies. It selects a host-policy test as
a lexical fallback, rather than the direct-notification regression requested
by the issue. It also cannot resolve every fixture or call. Those gaps remain
in the treatment. No solution, checker hint, extra model call, or handpicked
source appendix was added after the freeze.

## Acceptance and limits

The checker uses public client, host, catalog, and direct-connection APIs with
synthetic keys and temporary state. Five Linux integration cases cover an
imported day directory, an imported nested tree, unrelated files and sibling
roots, notification before the first catalog read, and an ordinary new chat
inside an existing directory. Atomic imports expose the missing directory
event without depending on the child-watcher registration race. Ordinary
crate tests and formatting run as well, with one pretrial exclusion described
below.

A prospectively recorded amendment replaced the original private-symbol
checker with these public-API tests. This admits internal refactoring and
avoids scoring by resemblance to the original patch. Imported directories
extend the original create-and-write scenario; that choice is disclosed before
model execution. Four-second positive bounds and 300 ms negative observations
are inherited from the existing test and do not establish notification latency.

The checks exercise operating-system notifications and loopback transport.
Repeated [calibration](calibration/README.md) gives the expected result in
11/11 observations for each revision: the base passes 3/5 cases and the original
fix passes 5/5. This measures whether this particular environment reproduces
the bug and accepts the historical fix; it does not make those tests deterministic
or prove complete correctness. Candidate patches also receive an independent
source review. Registered acceptance and any broader concerns are reported
separately.

## Historical test limitation

Calibration also exposed an unrelated, environment-sensitive test failure in
`tests::strict_destinations_schemas_and_private_store_permissions`. It changes
`observer.json` from mode `0600` to `0644` and expects the cached store to reject
it. The historical cache observes ctime but does not include mode in its stamp.
On this sandbox, ten immediate permission changes preserve the same observed
ctime even though the mode changes. The test fails in one full base run, then
passes three isolated runs; a single unchanged full reference gate also passes.
Those observations remain in the calibration evidence.

A prospective amendment excludes only that named test from the ordinary crate
gate in every arm. All other crate tests, formatting, and the five independent
notification checks remain unchanged. A separately named reserve verifier
records this difference; the round-2 verifier is untouched. No scored model
call precedes the amendment. This removes an unrelated source of flaky repair
feedback; it does not establish the omitted permission behavior as correct.

## Transport amendment before execution

Two calibration status reads received an HTTP 502. Each result was recovered
from the original process without relaunching it. The
[transport amendment](transport-amendment.json) binds a separate runner copy
that retries only GET responses with status 502, 503, or 504: at most three
attempts, with one- and two-second backoffs. Command launches, writes, and model
calls are never automatically retried. Sanitized retry events and elapsed time
remain in the results, and retry time stays inside the existing endpoint timer.

The [exact runner difference](transport-runner.diff) and 36 passing synthetic
tests cover the retry behavior, reporting, and unchanged coordinator. This
prospective amendment supersedes only the protocol's reference to the original
runner copy. The models, task, briefing, checks, repair rule, order, and win
thresholds remain unchanged. The original runner remains available for replay.

## Reading the result

Arm A is Opus without a brief; B is Opus with the brief; C is Sonnet without
a brief; D is Sonnet with the brief. The primary comparison is D versus A.
The same predeclared cost gate also evaluates C versus A for model selection
and D versus C for the brief's contribution on Sonnet. B versus A and D versus
B expose the other paired differences.

A cost win requires both compared arms to pass 4/4 runs, at least 20% lower
median CLI cost, lower cost in at least three of four blocks, and no more than
10% higher median recorded endpoint time. Every failure and repair remains in
the fixed panel. Each cost uses the final cumulative CLI estimate once. The
estimate is not a verified bill or subscription charge. Warmups and cold
indexing remain separate; machine and engineering costs are unmeasured.

Four repetitions of one reserved issue support a narrow engineering result.
They do not establish general model superiority, production routing policy,
or a net return on the engineering work required to build this prototype.

## What this tests in our tooling

Both executors are native Claude CLI sessions. Our experimental contribution
is the Rust source packer and the external, bounded verification-and-repair
loop. The optional source pack is the only difference within each model pair.
The shared checks let the experiment compare models at the same acceptance
boundary. No TypeSafe judgment, learned router, escalation cascade, or change
to Coder's default execution path runs in this panel.

The practical candidate is a small workflow: preserve complete instructions,
prepare bounded source evidence, use the less expensive executor, verify the
patch independently, and permit one repair with the actual failures. Treat
verification as part of its cost and behavior. This experiment cannot justify
removing those checks or sending every kind of issue to the same model.

The briefing still selects the wrong nearby test, despite receiving the exact
regression name in the task. Syntax-complete declarations help preserve local
code, but they do not establish coverage of the requested behavior. The
executor's additional reads and the checker's feedback remain useful work.
The next preparation changes should earn their place through small component
experiments before another paid executor panel.

## Next isolated experiments

These proposals follow the recorded gaps. They are untested and change none
of this panel's frozen inputs. The broader [iteration theses](../iteration-theses.md)
remain the prospective design record.

| Component | Small experiment and independent oracle | Measure before another executor trial |
| --- | --- | --- |
| Resolve the requested test | Give the index exact test names and historical path variants. Add tiny Rust fixtures with duplicate function names, import aliases, and one or two caller hops, with a separately authored expected-link table. Keep unresolved trait and macro calls explicit. | Exact-name recall, false canonical links, top-one selection, and bytes. Static reachability alone cannot establish behavioral relevance. |
| Complete fixture bindings | Compare the current declaration-wide shadowing set with call-site scope intervals. Cover `let helper = helper()`, earlier and later bindings, parameters, nested blocks, genuine shadowing, and cycles against a separate expected-resolution table. | Correct and incorrect dependencies, complete fixture bundles before and after the byte budget, and added preview time. Hold test selection fixed. |
| Reuse environment facts safely | Replay synthetic manifest and filesystem changes: nested workspaces, missing tools/includes, changed PATH precedence, updated launchers, symlink targets, modes, and another run's receipt. Compare recomputation, blind reuse, and reuse with required refreshes. | Stale positive reuse, rejected wrong-run receipts, correct cache hits, filesystem operations, and cold/warm latency. Metadata presence does not prove executable version or usability. |

Start with the binding fixtures because their expected result is narrow and
precise. Measure command and environment reuse on a scripted timeline, then
expand task-to-test resolution. Report failures as well as successes, including
same-size/same-mtime replacements that the current metadata key cannot detect.
Measure preview latency across fixed repository sizes; this panel's one warm
sample cannot establish a subsecond guarantee for arbitrary issue fetches.

After these component tests, freeze any improved packer and evaluate it on a
new task set. Keep model choice and briefing separate again. Add a TypeSafe
relevance judgment only when the deterministic candidate pool contains the
needed evidence; charge its preparation time and usage to the treatment. No
new confidence threshold or expected saving follows from this experiment.
