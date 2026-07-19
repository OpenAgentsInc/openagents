# Sol implementation claim protocol

- Date: 2026-07-09
- Class: contract
- Dispatch: ownership and collision control only. Live issues or exact
  owner-accepted plans/work packets select work
- Owner: Sol roadmap
- Status: active coordination contract
- Authority: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
- Dispatch sources: live `roadmap:sol` issues and their comments, or an exact
  owner-accepted plan/work packet when issue policy prohibits a feature issue.
  This contract governs ownership but does not select the queue

## Purpose

Parallel agents should increase completed integration, not duplicate the same
issue or collide through a shared schema. The live GitHub issue set is the
normal cross-session claim ledger. When no valid feature issue can exist under
repository policy, the exact accepted plan or work packet and its recorded
claim state are the ledger. The root coordinator is the ledger within one
Codex collaboration session.

## Same-session claims

The root coordinator assigns every subagent that can change files:

- one issue or bounded sub-scope
- its base SHA and clean worktree
- exact paths it may change
- hot files and hot contracts it must not change independently
- required verification and handoff shape.

The coordinator remains integration owner for shared schemas, migrations,
generated catalogs, behavior-contract registries, lockfiles, package-script
keys, route tables, and other cross-lane contracts.

## Cross-session claim comment

Before you change files in an independent Codex tab/session, post this to the live
issue. If feature issues are prohibited, record the same fields in the owner
accepted plan/work packet before the implementation commit:

```text
CLAIM
actor/session: <public-safe ref>
base: <commit SHA>
worktree/branch: <public-safe identity>
scope: <bounded outcome>
paths: <expected paths>
hot files: <shared files or none>
hot contracts: <schemas/migrations/catalog versions/registries/package keys or none>
verification: <commands and receipt>
claimed_at: <UTC timestamp>
```

A material scope, path, or contract change updates the claim before mutation.
Do not claim an entire epic when the actual unit is one leaf.

## Status and release

Post `CLAIM-STATUS` with a commit, test, or concrete blocker at meaningful
boundaries. Completion posts:

```text
CLAIM-RELEASE
landed: <main SHA or not-landed disposition>
verification: <exact results>
residual: <remaining work or none>
```

A claim is stale only when both are true:

1. No status or commit evidence has appeared for 90 minutes.
2. A coordinator checks the named process/worktree and finds no active work.

Elapsed time alone never authorizes taking another agent's work. A blocked
claim stays owned until explicitly released or the process/worktree audit proves
it abandoned.

## Sol and Terra cross-lane claims

Sol owns roadmap sequence control and hot-contract integration. Terra is authorized
to pull ready low-collision leaves under
[`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md),
but that authorization does not erase a current issue claim.

- Terra claims the exact leaf before mutation. It releases the claim after the
  push.
- Sol does not need to pre-write a new task brief when the issue and Terra pull
  rules already bound the outcome.
- An active Sol P0 claim and an active Terra P1 claim may proceed concurrently
  when files and contracts are disjoint.
- A Terra leaf can require a Sol-owned schema, migration, authority boundary,
  route table, catalog version, lockfile, or other hot contract.
- If it does, Terra updates the claim. It obtains an explicit integration owner
  before it changes the contract.
- A Terra change can affect a proof rung, dependency, residual, or next-ready
  slice. In that case, Terra gives a concise receipt to Sol for roadmap reconciliation.
  The pushed change does not need to wait for the prose reconciliation.

## Collision rule

File-disjoint is not necessarily contract-disjoint. Two lanes can touch the same
wire schema, migration sequence, catalog version, behavior-contract registry,
package-script key, generated output, or public promise version.
In that case, designate one integration owner before either change lands.
Leaf agents propose changes to that owner.
They do not make an independent version change to the shared contract.
