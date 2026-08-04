> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.

NIP-CC
======

Code Context and Coding Sessions
--------------------------------

`draft` `optional`

This NIP binds Work to exact code without making repository access
ambient. It defines the **Code Context** record — the typed link from Work
to repositories, revisions, and verification — and the **Coding Session**
specialization of the NIP-AS Agent Session.

Git remains the authority for Git objects and refs. NIP-34 remains the
signed collaboration wire for repositories, patches, and issues. This NIP
adds the workspace-side binding: which exact code a piece of Work is
about, what access intersection applies, and what a session actually ran
against.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32310 | Addressable | Code Context |
| 32311 | Addressable | Coding Session record |
| 32312-32319 | — | Reserved for future NIP-CC use |

Authority-signed; mutated through NIP-WI.

## 1. Code Context (`kind:32310`)

The code binding for one Work object. Address:

```text
32310:<authority_pubkey>:<work_ref>
```

### 1.1 Required tags

- `d`: the `work_ref`
- `org`
- `repo`: repeated repository refs — NIP-34 repository-announcement
  addresses (`30617:...`) or stable repository coordinates
- `revision`, `published_at`

### 1.2 Recommended tags

- `commit` with marker `pinned`: the exact base commit per repository
- `branch`: intended integration branch
- `path` with marker `focus`: focus path prefixes for the Work
- `verify`: pinned verification command ref
- `a` with marker `packet`: NIP-RC Work Packets decomposing this Work
- `e`/`a` with markers `patch`, `pr`, `issue`: NIP-34 patch/PR/issue
  refs as they appear
- `policy` with marker `access`: the access policy ref evaluated at
  admission

### 1.3 The access intersection

Effective code access for any actor on this Work is an intersection, and
the Code Context records the policy refs so the evaluation is auditable:

```text
effective access
  = owner grant
  ∩ Organization / Work Domain / Project policy
  ∩ repository-host permission
  ∩ this record's repository refs
  ∩ path and tool policy
  ∩ runtime containment
```

A repository listed here is a scope narrowing, never a grant: an actor
without underlying permission gains nothing from the listing, and search
or citation across the context never grants mutation.

## 2. Coding Session (`kind:32311`)

A companion record to a NIP-AS Agent Session for repository work.
Address:

```text
32311:<authority_pubkey>:<session_ref>
```

### 2.1 Required tags

- `d`: the same `session_ref` as the NIP-AS record
- `org`, `work`
- `a` with marker `session`: the NIP-AS Agent Session
- `repo`: the repository actually worked
- `commit` with marker `base`: the exact pinned commit the session
  started from
- `revision`, `published_at`

### 2.2 Recommended tags

- `worktree`: opaque worktree identity (never a local filesystem path)
- `branch` with markers `work` / `target`
- `containment`: the containment/sandbox profile ref the session ran
  under
- `a` with marker `claim`: the NIP-RC claim held during the session
- `verify` plus `e` with marker `verify_evidence`: the verification
  command and its NIP-EV evidence receipt
- `e`/`a` with markers `patch`, `pr`: produced NIP-34 patches or PR refs
- `diffstat`: bounded change summary (`"<files>/<insertions>/<deletions>"`)

### 2.3 Rules

- **Pin or refuse.** A coding session without a pinned base commit is
  refused at admission: "latest" is not a revision, and compatibility
  claims are meaningless without the exact base.
- **Catch-up is explicit.** Reconciling with a newer upstream revision
  publishes an updated record (`commit` marker `base` moves, prior base
  retained with marker `previous_base`) plus fresh verification evidence
  — a clean merge alone is not compatibility proof.
- **Private material stays off the wire.** Worktree identities are
  opaque; local paths, raw diffs, and shell output never appear. The
  public trace is refs, digests, and the bounded diffstat.
- **Git truth is Git's.** PR state, checks, reviews, and merges reconcile
  from the Git/NIP-34 authority. This record cites them; a session that
  "merged" per its own narration but has no Git evidence displays as
  unmerged.

### 2.4 Example

```json
{
  "kind": 32311,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "sess-2f88"],
    ["org", "org-openagents"],
    ["work", "work-9f31854f"],
    ["a", "32280:<authority-pubkey>:sess-2f88", "", "session"],
    ["repo", "30617:<repo-owner-pubkey>:openagents"],
    ["commit", "dea095c0b7be649e1d34017b2abe2b318d46e57d", "", "base"],
    ["branch", "main", "", "target"],
    ["worktree", "wt-4c11"],
    ["a", "32301:<authority-pubkey>:claim-77e0", "", "claim"],
    ["verify", "pnpm run check:fast"],
    ["e", "<evidence-receipt-id>", "", "verify_evidence"],
    ["diffstat", "9/1277/10"],
    ["revision", "3"],
    ["published_at", "1786502000"]
  ]
}
```

## Security considerations

- **Ambient-access creep.** The intersection rule is the defense: listing
  a repository in context, citing a file in search results, or naming a
  path in an Issue body never widens access.
- **Path leakage.** Opaque worktree ids and repository coordinates keep
  local topology private; a record carrying a local path fails admission.
- **Revision confusion.** All downstream claims (evidence, review,
  compatibility) inherit the pinned base; clients MUST surface base-drift
  between a session's pin and the current target branch.
- **Cross-authority repos.** A NIP-34 repository under a foreign key is a
  reference; its patches and state are that ecosystem's records, and this
  NIP's records never assert foreign repository state.

## References

- NIP-01, NIP-34
- NIP-WK, NIP-WI, NIP-EV (layer 0)
- NIP-AS, NIP-AD, NIP-RC, NIP-RV (this layer)
- `docs/omega/GLOSSARY.md` — Code Context, Coding Session, Repository
  Catch-Up, Compatibility Proof

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Code Context with the access intersection,
  Coding Session with pinned bases, explicit catch-up, and Git-truth
  boundaries.
