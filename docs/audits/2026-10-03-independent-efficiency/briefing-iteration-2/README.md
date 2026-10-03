# Focused briefing iteration

This study continues the [failed first replay](../historical-replay-10166/README.md).
The first four fresh candidates all missed a nested ignored-cache case. Their
own tests passed, and the original briefing did not include the faulty function.
That result is retained. Issue #10166 is development data for this round.
Iteration work is tracked in [#10282](https://github.com/OpenAgentsInc/openagents/issues/10282).

## Result: no clear win

The [held-out panel](heldout/README.md) is complete. Both arms pass the frozen
checks in 4/4 final runs. Median CLI list-price cost is $1.1053 for control and
$0.9660 with the brief, a 12.6% reduction. Treatment is cheaper in three of
four pairs. Median recorded endpoint time is 301.2 versus 278.5 seconds, a
7.5% reduction. The result falls short of the registered 20% cost threshold.

| Held-out measure | Control | Briefing |
| --- | ---: | ---: |
| Final patches passing the registered checks | 4/4 | 4/4 |
| First attempts passing those checks | 3/4 | 4/4 |
| Median CLI list-price estimate | $1.1053 | $0.9660 |
| Median recorded endpoint time | 301.2 s | 278.5 s |
| Median tool calls | 29 | 33.5 |
| Median file reads | 8 | 10.5 |

The final control's first draft changed `read_all` but missed its `Catalog`
consumer; one repair corrected it. The briefing does not reduce median tool
calls or reads on this task. The evidence therefore does not establish reduced
discovery as the cause of the cost difference. Cache and model variation remain
possible contributors in this small panel.

Source review finds a broader quality limitation in both arms: some accepted
patches can still select different equal-rank copies or conflicting manifest
marks when input order reverses. The frozen checker covers retained-versus-live
precedence but omits those conflicts. The [independent audit](heldout-audit.json)
identifies affected candidates and source evidence. The
[post hoc counterexample tests](posthoc_equal_rank.rs) are prepared but have
not been compiled or executed; they do not change registered scores. The
historical reference sorts sources and copy candidates and avoids these gaps
by static inspection. Passing this panel is not proof of complete correctness.

The next [model and structured-briefing experiment](../briefing-model-factorial/README.md)
separates a less expensive executor from the brief's contribution. Its reserve,
packer, order, and gate are frozen before scored calls. This failed cost gate
remains part of the evidence.

## Development result

The [development panel](development/README.md) is complete. Both arms reach
4/4 accepted patches, each after one repair. Median CLI cost is $0.8631 for
control and $0.7564 with the brief, a 12.4% reduction. Treatment is cheaper
in three of four pairs. Median recorded endpoint wall time is 222.0 versus 167.7 seconds,
a 24.5% reduction. This falls short of the prospective 20% cost threshold.
The registered development acceptance gate permitted the unchanged method to
advance to the independent Gym task reported above.

Elapsed time includes source export and preflight checks. The final control
setup takes 74.0 seconds; all other scored development setups take 3.7–4.0
seconds. The runner does not time each setup component separately, so the
cause of this outlier is unknown. This variability affects the endpoint
comparison; the report also retains model and check time separately. Do not
attribute all elapsed savings to reduced model discovery. Removing the entire
setup bucket from both arms gives a secondary median wall reduction of 14.4%.
The [independent development review](development-audit.json) records both
comparisons and the failure classification.

Every initial draft fails the independent checker's path-bearing refusal
requirements. That diagnostic criterion is stricter than the common task's
explicit wording; these repairs do not all represent unsafe behavior. The
first and final controls also incorrectly permit an unknown ignored parent
containing `target/`; all four treatment drafts refuse it. Of 88 failed
first-draft case results, 86 stop on diagnostic text after refusing removal
and two stop on that unsafe admission. The shared feedback exposes the failed
requirements, and every second draft passes. This supports measuring verification and repair as part
of task completion, without establishing that the brief prevents repairs.

## Theses

1. **Prepare the exact source before asking for reasoning.** Explicit issue
   references and complete enclosing declarations can eliminate discovery
   calls without guessing a fix. Bounded source packs should expose missing
   coverage instead of clipping a function without warning.
2. **Replace tool-semantics guesses with measured facts.** A synthetic Git
   probe takes milliseconds and demonstrates which command preserves the
   ignored directory boundary. Its result is reusable evidence. It does not
   certify a candidate or authorize deleting a worktree.
3. **Make cheap verification part of the executor contract.** Identical
   formatting, independent behavioral checks, and one repair turn can catch
   plausible wrong patches. Comparing first drafts alone does not measure
   the cost of finishing a task.
4. **Preserve required instructions independently of retrieval.** A shorter
   prompt is not an improvement if it drops applicable rules. Both arms
   receive the same complete instruction block, whose files and bytes are
   checked before execution and after each model turn.

The frozen round tests the source pack and conditionally triggered Git probe
as a bundle. It does not isolate the probe's contribution. Formatting and
feedback are identical in both arms and are therefore experimental controls.
No decision model or extra reasoning model generates the briefing.

## Changes available in the prototype

`briefing-lab preview --focused` writes `focused.md`, a source evidence payload
bounded to 16 KiB. It prioritizes explicit references, uses complete small files
or containing Rust declarations, includes nearby tests and manifests, and
records omissions. The cache remains bound to the pinned Git source.

This study disables broad lexical and symbol selection with `--no-lexical
--no-symbols`. Explicit line anchors still use the syntax tree. Large files
without line anchors fall back to a labeled first-64-line excerpt; that is a
known coverage limit. The executor can read more source.

For the development case, fault-body coverage improves from 0/51 lines to
51/51 lines. The complete-small-file rule supplies this gain; it is not an
AST result. Four new source excerpts occupy 11,481 bytes, compared with
32,986 bytes across 14 old excerpts. However, the new unanchored specification
excerpt misses the safety section, and its test excerpt misses relevant Git
helpers and worktree tests. The held-out pack likewise contains partial
large-file excerpts. Coverage improvement is evidence of better retrieval
for one location, not proof that an agent can finish from the brief alone.

The [preparation script](../../../../bench/briefing-replay/prepare.py) adds the
fixed Git probe only when the task contains `git`, `ignored`, and `worktree`.
The [runner](../../../../bench/briefing-replay/replay.py) provides identical
file tools and keeps one Claude process alive through its optional repair.
Checks execute on an isolated Linux Boat sandbox. No Cargo build runs on the
owner's Mac, and no Claude chat is persisted.

## Registered comparison

The [protocol](protocol.md) defines four balanced development pairs and four
held-out pairs, acceptance requirements, cost and time thresholds, feedback,
cache warmup, and stopping rules. A practical efficiency win requires all four
held-out runs in each arm to pass, at least a 20% lower treatment median cost,
at least three cheaper pairs, and no more than a 10% increase in median total
wall time. These are small-panel engineering thresholds, not a population
superiority claim.

The held-out task is a September 26 fresh Gym assignment: make retained run
marks and resulting claims independent of input-directory order. Its source
and checker were selected before model calls. The packer was frozen before
its implementer could see that task. The historical fix and independent
checker remain outside executor roots.

Both arms use Claude Opus 5.5, medium effort, with Read, Edit, Write, Glob, and
Grep. Consequently this estimates the effect of prepared context under a
controlled executor. The original human-directed Claude sessions, with shell,
network, coordination, and deployment, are historical context and are not
matched controls.

## Preparation and calibration

The development source pack and probe took 0.299 seconds with a warm index;
the payload is 13,890 bytes. Building the index separately took 2.068 seconds.
The held-out source pack took 0.199 seconds with a warm index; it is 16,202
bytes and does not trigger the Git probe. Its cold index took 1.137 seconds.
These are initial measured samples, not latency percentiles or an arbitrary
GitHub-issue guarantee. Network issue retrieval and compilation are separate.

Ten further development preparation samples, taken while historical
dependencies compiled, ranged from 0.371 to 0.747 seconds for preview plus
probe. The cold filesystem export used for the common instruction warmup took
153.9 seconds; the first scored export took 4.0 seconds. These are benchmark
setup costs, distinct from briefing latency.

The development checker retains the original seven cases and adds ten
explicit filesystem-safety cases disclosed to both arms. The unfixed source
passes 6/17. An independently reviewed reference passes 17/17. The historical
fix is only a positive control for the original requirements; the extensions
must not be described as failures of its original acceptance contract.

The Gym base passes its 611 existing tests and one independent preservation
control; four independent assertions fail. The historical fix passes its
613 existing tests and all five independent checks. The final pinned verifier
export reproduces the base result. The initial missing dependency was fetched
before scored runs and retained as an infrastructure setup failure.

The focused packer passes 51 targeted Rust tests and formatting on Boat.
The two new metadata tests cover ambiguous declarations and disabled
selection components. The first corruption fixture was corrected before
trials: extending a byte range by its existing terminal newline did not
actually violate its coordinate contract.

## Accounting

The CLI reports cumulative list-price usage estimates for a session. The
runner counts its final value once, including a repair when present. The CLI
API-duration field is also cumulative; its ordinary duration field is per
input turn. Measured wall time remains the primary timing metric. These
are not verified subscription charges. Warmups, invalid infrastructure
runs, preparation, and verification are recorded separately. Failed
candidates remain in the results and cannot be called cost through acceptance.
The frozen runner stops its wall timer after the executor closes. Final patch
capture, scratch-directory deletion, and result serialization happen afterward.
The reported endpoint includes setup, model work, and checks, but is not the
entire benchmark process duration. The registered comparison retains that
same formula in both arms. Raw model streams, account metadata, and private
conversation records remain outside the repository.

## Further hypotheses to test independently

The [iteration theses](../iteration-theses.md) give each mechanism an isolated
metric, executor comparison, and explicit failure condition.

The development panel suggests several distinct experiments. None changes the
registered treatment during this panel.

| Component | Hypothesis | Isolated measurement |
| --- | --- | --- |
| Evidence within an explicit file | Rank declarations and document sections inside named files without admitting unrelated repository-wide matches. | Exact required-span coverage, missing helper references, bytes, and preparation time on pinned tasks. |
| Complete test setup | Supply the existing fixture constructor and relevant tests together, so the agent need not rediscover how to create a valid repository or run record. | Fixture completeness and additional discovery calls, followed by independent acceptance. |
| Public acceptance contract | Preserve explicit diagnostic and safety requirements in a compact checklist with source references. | Missed requirements and repairs; only public task/specification requirements can feed the checklist. Hidden checker contents remain evaluation evidence. |
| Runtime observations | Cache bounded synthetic observations by tool version, source identity, and probe digest. | Probe latency, cache invalidation, and a source-pack-only versus source-pack-plus-probe ablation. |
| Source coverage gate | Admit a briefing only when it contains the required complete source units; otherwise report the gap or request a targeted read. | Irrelevant bytes avoided and missed necessary context; test the abstention rule before testing agent savings. |

The development probe bundle needs an ablation before attributing a saving to
syntax selection or runtime facts. A System One reranker would be a separate
arm over the same frozen candidate pool. Its calls, latency, and cost belong
in the treatment total. It must beat the deterministic selection baseline
before replacing it.
