# IDE-12 Effect source control

Date: 2026-07-20
Issue: [#9040](https://github.com/OpenAgentsInc/openagents/issues/9040)
State: implemented candidate. Independent review is not complete.

## Result

Desktop now uses one Effect source-control graph for repository reads and Git
changes. The main process owns this graph. The renderer sends decoded commands
and shows decoded snapshots. It does not run Git or call a provider.

The graph keeps these states separate:

| State | Proof |
| --- | --- |
| changed | An exact HEAD, index, and worktree snapshot shows a change. |
| reviewed | A review fact identifies the reviewed version. |
| committed | The expected tree is in the exact local commit. |
| pushed | The remote branch OID is the exact local HEAD OID. |
| pull request | Decoded provider facts identify the repository, head, base, commits, checks, reviews, merge state, and freshness. |
| merged | The provider or remote state proves the merge. |
| owner accepted | An independent owner record supplies this fact. It is false in the candidate receipt. |
| released | Release evidence supplies this fact. It is false in the candidate receipt. |

## Authority and versions

`source-control-contract.ts` defines the repository, worktree, binding, status,
version, selection, operation, receipt, recovery, provider, and delivery
schemas. `source-control-service.ts` supplies the Effect service. It serializes
operations, checks the expected version, stores bounded receipts, and publishes
typed events. `source-control-git-adapter.ts` is the decoded Git and GitHub CLI
adapter. `source-control-host.ts` owns the service lifetime.

Each change command includes the exact repository and worktree binding, the
expected HEAD, index, and worktree versions, an operation reference, an actor,
and an approval reference when policy requires it. The adapter reads the live
Git preimage immediately before a change. It refuses a stale preimage. A process
exit code does not prove a postcondition.

The production main process and preload use the canonical source-control IPC
channels. The old Git host can supply compatible read data. It cannot perform a
renderer-requested change.

## Supported operations

The implementation supplies these operations:

- refresh and exact status for tracked, untracked, ignored, binary, large,
  rename, conflict, submodule, LFS, detached-HEAD, and worktree facts.
- full-file and partial hunk or line stage and unstage.
- destructive discard with an exact preview, a required approval for an agent,
  a private recovery patch, and recovery after a service restart.
- commit, amend, branch, tag, switch, merge, rebase, cherry-pick, revert,
  continue, and abort.
- fetch, pull, and push with prompt-disabled execution, exact refspec policy,
  non-fast-forward facts, and an observed remote-OID postcondition.
- worktree create, inspect, repair, and safe remove with ownership, occupancy,
  dirty, changed, and unpushed facts.
- version-bound history and blame.
- decoded pull-request, commit, review, check, mergeability, merge, and
  freshness facts.

The adapter rejects option-shaped refs and remotes. It does not follow a
symlink to inspect private content. It withholds ignored, private, and
secret-shaped paths from content projection and refuses an unsafe stage or
discard request.

## Concurrency and recovery

All human, agent, and external Git changes converge through refresh and the
same expected-version protocol. Parallel worktrees keep distinct bindings and
ownership. A worktree removal requires the exact preview and refuses an active,
dirty, changed, unpushed, unmanaged, or stale target.

Candidate worktrees do not merge themselves. Comparison and fan-in remain an
explicit review and merge action. The delivery record never converts an agent
statement into review, acceptance, or release proof.

The discard recovery store uses a mode `0700` directory and mode `0600` files.
The stored patch survives a service restart. A successful recovery deletes the
stored patch. Merge, rebase, cherry-pick, and revert operations expose their
conflict state and use explicit continue or abort commands.

## Verification evidence

The deterministic real-Git corpus covers partial changes, stale content,
discard and restart recovery, hooks, signing, option injection, detached HEAD,
branch and rewrite flows, conflicts, worktrees, local bare remotes, fetch,
rebase pull, exact push, non-fast-forward refusal, history, blame, and decoded
provider facts.

The benchmark receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-12-source-control.json`.
It records p50, p95, and p99 results for small, medium, and large status refresh,
stage and unstage, and 100-entry history. It also records heap, handle, child
process, private-path, ignored-path, and credential projection facts.

The packaged macOS arm64 journey receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-12-source-control-packaged.json`.
It binds the packaged artifact digest and proves visible stage, commit, exact
remote-OID push, discard, persistent recovery, distinct delivery facts,
keyboard focus, and private-path withholding. Its screenshot and redacted trace
are beside the receipt.

The acceptance evaluator is
`apps/openagents-desktop/scripts/ide-source-control-acceptance.ts`. It decodes
both receipts, checks candidate ancestry, recomputes the packaged artifact
digest, runs the IDE boundary check, runs `git diff --check`, and rejects a
private path or secret-shaped value.

Run the complete packet with:

```sh
pnpm --dir apps/openagents-desktop run verify:ide-12
pnpm run check:ste
```

## Review and release state

The candidate reviewer disposition is `unreviewed`. The owner disposition is
`unreviewed`. The AssuranceSpec lifecycle is `proposed`. This record does not
claim Windows or Linux packaged evidence. IDE-13 owns portable project
placement. IDE-17 owns the broader parallel-agent comparison surface.

Rollback uses the exact parent of the candidate source commit. A rollback must
not delete a user worktree or a recovery patch. The final issue record supplies
the exact candidate, main, artifact, evidence, and rollback references after
the change lands.
