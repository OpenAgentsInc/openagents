# Model selection and structured briefings

This experiment tests two changes separately: executing a task with Sonnet
instead of Opus, and supplying a deterministic source briefing. Four balanced
blocks compare all four combinations on the same historical issue. The
[prospective protocol](plan.md) fixes the order, models, checks, one-repair
limit, and cost and time thresholds before scored calls.

Scored calls have not started. Calibration and final registration precede execution.

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
crate tests and formatting run as well.

A prospectively recorded amendment replaced the original private-symbol
checker with these public-API tests. This admits internal refactoring and
avoids scoring by resemblance to the original patch. Imported directories
extend the original create-and-write scenario; that choice is disclosed before
model execution. Four-second positive bounds and 300 ms negative observations
are inherited from the existing test and do not establish notification latency.

The checks exercise operating-system notifications and loopback transport.
Repeated calibration measures whether this particular environment reproduces
the bug and accepts the historical fix; it does not make those tests deterministic
or prove complete correctness. Candidate patches also receive an independent
source review. Registered acceptance and any broader concerns are reported
separately.

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
