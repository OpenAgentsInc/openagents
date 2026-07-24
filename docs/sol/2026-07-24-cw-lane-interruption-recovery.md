# Community workroom lane interruption: recovery record

- Class: recovery record and handoff
- Date: 2026-07-24
- Cause: agent capacity exhausted mid-lane (provider rate limit)
- Scope: the `SARAH-CW` community workroom lane in `openagents`
- Main at time of writing: `6aae1578cd`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

## 1. What happened

Several agents ran the Sarah workroom program in parallel. One reached a
provider rate limit and stopped mid-lane. The result is not lost work. It is work in three different states. A later
agent that assumes a clean start will either duplicate it or overwrite it.

This document records the exact state so recovery is mechanical.

## 2. What landed and is safe

These closed on `main` and need no recovery.

| Repository | Result |
| --- | --- |
| `nostr-effect` | every issue closed. The relay core left the Bun backend. The Node host, the Node store, and the Cloud SQL store landed. The toolchain became pnpm and Vite Plus, and the NIP-29 group policy landed |
| `omega` | `OMEGA-SW-01`, `SW-03`, `SW-04`, `SW-05`, `SW-06`, `SARAH-NR-06`, and `SARAH-CW-08` all landed |
| `openagents` | `SARAH-NR-00`, `NR-03`, `NR-04`, `NR-05`, `NR-07`, `NR-07a`, `NR-08`, `NR-09`, `CW-00`, and `CW-03` all landed |

The critical path from the Nostr-first sequence is complete. A local Node relay
is available, which was the gate on client work.

## 3. What is interrupted

Five lanes are partly done. Nothing is on `main`.

| Issue | Lane | State | Where it lives |
| --- | --- | --- | --- |
| `#9227` | `CW-02` membership | one commit, plus an unresolved merge conflict | branch `salvage/cw02-membership` |
| `#9228` | `CW-04` request and quote lane | no commit. Staged fixtures and a validator | worktree `.worktrees/oa-cw04` |
| `#9229` | `CW-05` arbitration and dispute | no commit. Staged package plus an unresolved conflict | worktree `.worktrees/oa-cw05` |
| `#9226` | `CW-06` experience points | one commit, complete, unverified | branch `codex/sarah-cw06-xp` |
| `#9231` | `CW-09` outside-developer journey | one commit, complete, unverified | branch `salvage/cw09-journey` |

The commits are anchored to refs so a worktree cleanup cannot lose them. Two of
them, `CW-06` and `CW-09`, look complete and only need verification before they
go to `main`.

## 4. The collision

`CW-02` and `CW-05` both stopped on the same unresolved conflict, in the same
two files:

- `docs/omega/2026-07-24-community-workroom-contract.md`
- `docs/ste/final-inventory.v1.json`

That is not a coincidence. `CW-00` froze the community contract in
`d9bb14675f`, and `6aae1578cd` then aligned the grant freeze with the landed
`CW-03` schema. Both later lanes edited the same frozen document concurrently
and rebased onto a moved target.

The contract document is a hot file. The generated STE inventory is a hot file
by construction, because every documentation change rewrites it.

**Rule for the recovery:** one lane at a time may edit the community contract.
Never resolve the inventory conflict by hand. Take either side, then regenerate
it and commit the regenerated output.

## 5. Recovery procedure

For each lane, in this order.

### 5.1 `CW-06` and `CW-09` first, because they are cheapest

Both are complete commits on refs. Do not re-implement them.

1. Create a fresh worktree from current `origin/main`.
2. Cherry-pick the commit.
3. Regenerate the STE inventory rather than resolving it.
4. Run `pnpm run check`.
5. Push, then comment the result on the issue and close it.

If verification fails, fix forward in the new worktree. Do not abandon the
commit and start over.

### 5.2 `CW-02`, because the conflict is small

The commit exists. The conflict is a rebase artifact, not a design problem.

1. Fresh worktree from current `origin/main`.
2. Cherry-pick `salvage/cw02-membership`.
3. For the contract document, take the version on `main` and re-apply the
   membership edit on top. `main` is the frozen contract and wins.
4. Regenerate the inventory.
5. Verify, push, close.

### 5.3 `CW-04` and `CW-05`, which need the most care

Neither has a commit, so the work exists only as uncommitted files in a
worktree. Read the file list before writing anything, or you will duplicate it.

`CW-04` has canonical and negative fixtures plus a validator under
`fixtures/sarah-lbr-request-quote/`. `CW-05` has a `community-arbitration`
package under `packages/sarah/src/` plus an edit to `NEEDS_OWNER.md`.

1. Read the existing files first.
2. Move the work to a fresh worktree from current `origin/main`.
3. Commit it before doing anything else, so the next interruption is cheap.
4. Then resolve, verify, push, close.

## 6. What this cost, and the cheap fix

The expensive part of this interruption was not the lost compute. It was that
two lanes did uncommitted work for a long time and then hit a moved target.

**Commit early inside the worktree.** An uncommitted worktree is the only
state that a rate limit can actually destroy. A commit on a branch survives
everything short of a deliberate delete. That is why the two lanes with commits
are nearly free to recover, and the two without them need care.

The second lesson is the hot file. When several lanes touch one frozen
contract, serialize that file explicitly rather than hoping the rebases
commute.

## 7. Owner-visible state

One owner action is still outstanding and is unaffected by this interruption:
the authoritative appeal Nostr public key, recorded in `NEEDS_OWNER.md`. It
blocks `#9229` from closing and blocks the v2 room from opening. It blocks
nothing in v1.
