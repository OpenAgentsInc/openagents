# Focused briefing iteration

This study continues the [failed first replay](../historical-replay-10166/README.md).
The first four fresh candidates all missed a nested ignored-cache case. Their
own tests passed, and the original briefing did not include the faulty function.
That result is retained. Issue #10166 is development data for this round.

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

The development checker retains the original seven cases and adds ten
explicit filesystem-safety cases disclosed to both arms. The unfixed source
passes 6/17. An independently reviewed reference passes 17/17. The historical
fix is only a positive control for the original requirements; the extensions
must not be described as failures of its original acceptance contract.

The focused packer passes 51 targeted Rust tests and formatting on Boat.
The two new metadata tests cover ambiguous declarations and disabled
selection components. The first corruption fixture was corrected before
trials: extending a byte range by its existing terminal newline did not
actually violate its coordinate contract.

## Accounting

The CLI reports cumulative list-price usage estimates for a session. The
runner counts its final value once, including a repair when present. These
are not verified subscription charges. Warmups, invalid infrastructure
runs, preparation, and verification are recorded separately. Failed
candidates remain in the results and cannot be called cost through acceptance.
Raw model streams, account metadata, and private conversation records remain
outside the repository.
