# Coding quests and world changes

Status: proposed. A coding quest produces a real artifact under a fixed contract.
A success animation is downstream of verification and acceptance.

## First quest: repair the bridge planner

Create a small public Rust fixture repository or a separately scoped fixture
workspace in this repository. Pin its base commit and the test package. It must
contain no production credentials or connection to the live arena ledger.

The proposed function receives a finite grid of cells and a pair of endpoints.
It returns either an adjacent, in-bounds path over permitted cells or an explicit
unreachable result. The initial defect mishandles a boundary condition and can
include a blocked cell in a purported path. The repair must satisfy:

- Every returned coordinate is in bounds and traversable.
- Consecutive cells are adjacent under the declared movement rule.
- The endpoints match the request, and unreachable inputs are handled correctly.
- Input limits and termination bounds are respected.
- Existing valid cases continue to work.

The independent verifier uses fixed examples plus generated bounded grids whose
expected reachability comes from a separate reference implementation. Keep the
verifier and confirmation inputs outside the writable candidate worktree. Pin
the generator seed for replay, retain the test count and failures, and include
at least one unseen layout after the initial repair.

On acceptance, the referee uses the accepted planner output for one frozen world
layout. It rechecks that the path stays inside the registered bridge region and
contains only permitted coordinates, then applies a fixed gate/bridge operation.
No generated code receives a Minecraft administration connection. If the world
effect cannot be confirmed, the code can remain accepted while world integration
is pending; do not invent a green bridge result.

This fixture is new work to implement. The specification does not identify an
existing bug in a production crate or assert that a patch has been generated.

## Why this quest

The viewer can understand its before/after behavior: the route is invalid, an
agent fixes the planner, tests demonstrate the invariant, and the bridge opens.
It demonstrates coding beyond asking a model to narrate success. It is small
enough for a recording and bounded enough to evaluate independently.

Do not let a competitor repair the actual CC ledger, permissions, verifier,
signing path, or relay admission logic during the competition. Those systems
define the game. Later real repository tasks can use the same delivery contract
with maintainer review and an explicit integration destination.

## Quest definition

Each versioned quest definition must identify the following fields. These are
application requirements; finalize their serialization and fixtures before use.

| Field group | Required contents |
| --- | --- |
| Identity | Season, quest ID/version, author, definition digest |
| Objective | User-visible requirement, exclusions, accepted output types |
| Inputs | Repository identity, exact base commit, permitted files, public examples |
| Execution | Trusted executor binding, filesystem scope, allowed commands, resource limits |
| Verification | Independent checker identity, immutable checker artifact, case policy, required checks |
| Acceptance | Integrator identity, destination, expected base/preconditions, conflict policy |
| Reward | Fixed XP, uniqueness key, contributor attribution rule |
| World mapping | Bounded target region, effect schema, reconciliation procedure |
| Budget | Shared parent, attempt cap, per-stage holds, deadline, maximum parallel workers |
| Disclosure | Public summary fields and authorized recipients for code, logs, and cases |

Generation, critique, and repair all consume the same parent budget. An agent
cannot multiply available compute by splitting the work into child tasks.
Subtasks inherit narrower grants and deadlines. A reviewer has read access to
the candidate and evidence, not permission to replace acceptance rules.

## Attempt lifecycle

1. The coordinator claims the guild's quest copy and reserves its bounded budget.
2. The executor creates an isolated worktree from the exact base. Record the
   initial snapshot and expected permitted writes.
3. The worker receives a CTX task frame with objective, scope, interfaces, public
   examples, and immutable POL constraints. It proposes a patch using the
   admitted generation door.
4. Run allowed checks under the process supervisor and execution boundary.
   Capture bounded diagnostics, exit status, changed files, and artifact digests.
5. If checks fail, return relevant diagnostics for a repair, within the total
   round and cost limits. Do not reveal protected confirmation cases to search.
6. The neutral verifier runs the independent suite against the exact submitted
   artifact. The submitter's own “tests passed” message is insufficient.
7. The integrator checks the base, claim generation, write scope, and verification
   evidence, then accepts or rejects the result for the demo's candidate branch.
8. The referee awards XP once and dispatches the separate world effect. Retain
   every status even if one stage fails or remains unknown.

Use the common three axes explicitly: execution outcome, verification status,
and integration status. `completed` / `failed` / `rejected` is a valid combination.
So is `completed` / `passed` / `pending`. Only the defined accepted combination
earns the completion reward.

The recording does not require a push to this repository's `main`. Each guild
can integrate into its disposable candidate branch under a recorded test
authority. Production contributions require the repository's actual integration
rules. Game XP is not permission to merge production code.

## Protect the verifier

Candidate code, including build scripts and tests, is untrusted execution. An
unwritable checker file is not enough if the candidate process can read hidden
labels or access the verifier's credentials. Run candidate code in a separate
bounded process with only the inputs required for that case. Keep reference
answers, award keys, and the authoritative comparison outside that process.

Disable unneeded network access and prevent writes outside the permitted
workspace and output directory. Refuse an enforcement profile the host cannot
provide. Use [execution-boundary evidence](../coder/verification/2026-09-20-execution-boundary.md)
and [subprocess supervision](../coder/runtime/subprocesses.md) as existing
contracts. Incomplete snapshots make a run unverifiable.

The checker must detect changes to forbidden files, weakened or deleted tests,
out-of-scope writes, malformed output, timeout, and descendant processes that
outlive cancellation. The referee never loads candidate code as a plugin in its
own address space.

## Review and contribution

A guild reviewer can catch a real flaw before submission and can receive
attribution within the fixed quest award. Record what evidence the reviewer
examined and which issue the final patch resolved. A review that merely agrees
with a teammate does not multiply XP.

The neutral verifier is a separate trusted role. If it uses a model for a
semantic check, retain its input, model identity, policy, and uncertainty, and
keep it separate from executable checks. For the bridge fixture, the required
acceptance criteria should be deterministic.

## Follow-up quests

After the first repair, introduce one at a time:

| Quest | New behavior tested | Keep fixed |
| --- | --- | --- |
| A new bridge layout | Reuse against unseen input | API, movement rules, verifier |
| A bounded delivery-manifest parser | Semantic task selection and typed validation | Input size, allowed outputs, no network |
| Two independent repairs | Parallel claims and shared-budget reservation | Per-quest acceptance and isolated branches |
| Competing patches to one target | Fencing and integration conflict handling | One integrator and base preconditions |
| Another guild reuses a released skill | EXT discovery, trust, and transfer evaluation | Consumer grants and independent checks |

Evaluate deliberate repeats as separate trials, but award a unique quest only
once per eligible guild and season. This keeps benchmark replication from
becoming a reward exploit.
