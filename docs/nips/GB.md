> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 3 — automation and attention.

NIP-GB
======

Guidance Bundles
----------------

`draft` `optional`

This NIP defines **Guidance Bundles**: versioned organization, domain,
team, project, repository, and workflow instructions supplied to agents in
a known precedence order.

Guidance is the policy-and-knowledge counterpart of NIP-SKL skills: a
skill is a versioned procedure; guidance is the standing instruction
context the procedure runs inside. Both share the same hard boundary:

> Natural language cannot grant capability. Guidance can instruct, warn,
> constrain, and prohibit; it cannot widen tools, budgets, repositories,
> identities, audiences, disclosure, release, or settlement. A deny in
> higher-precedence guidance fails closed against every lower layer.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32330 | Addressable | Guidance Bundle (current head) |
| 32331 | Addressable (unique `d`) | Guidance Revision archive |
| 32332-32339 | — | Reserved for future NIP-GB use |

Authority-signed; mutated through NIP-WI (`guidance.create`,
`guidance.revise`, `guidance.retire`).

## 1. Guidance Bundle (`kind:32330`)

The current head of one bundle. Address:

```text
32330:<authority_pubkey>:<guidance_ref>
```

### 1.1 Required tags

- `d`: stable `guidance_ref`
- `org`
- `scope`: the precedence layer this bundle occupies — `organization`,
  `domain`, `team`, `project`, `repository`, `workflow`, or `work`
- `name`: bounded display name
- `revision`: current monotonic revision
- `x`: digest of the exact current body bytes
- `state`: `active` or `retired`
- `published_at`

### 1.2 Recommended tags

- `a` with marker `subject`: the scoped record (Team, Project,
  repository, Work) the bundle attaches to
- `p` with marker `author` / `editor`
- `a` with marker `revision`: the `kind:32331` archive of the current
  revision
- `audience`: who may read the body
- `t`: discovery topics

`content` carries the guidance body (public-safe Markdown, NIP-44 payload,
or empty with the body off-relay behind the digest).

### 1.3 Precedence

The deployment's owning policy selects the exact chain. The recommended
default, highest first:

```text
repository invariants and admitted product contracts
  > owner policy
  > organization guidance
  > domain guidance
  > team guidance
  > project guidance
  > work-item instruction
  > skill defaults (NIP-SKL)
  > agent defaults
```

Rules:

- **Deny wins downward.** A prohibition at any layer cannot be overridden
  by permission-shaped prose at a lower layer.
- **Conflicts fail closed.** Two same-layer bundles that contradict on a
  constraint make the constrained action inadmissible until an authority
  resolves the conflict with a revision.
- **Precedence is data.** The chain in force is named by policy ref, not
  inferred from bundle names.

### 1.4 Run manifests

Every agent run records the exact guidance it consumed: each bundle's
`guidance_ref` plus `revision` (equivalently, the `kind:32331` archive
address) enters the session's context manifest (NIP-AS). This is the
reproducibility contract — behavior change without a revision change is a
defect, and an audit can re-read exactly what the agent was told.

## 2. Guidance Revision archive (`kind:32331`)

Address: `32331:<authority_pubkey>:<guidance_ref>:rev:<n>` — the same
append-only pattern as NIP-DD document revisions: each revision's exact
digest, author, and time remain provable after the head moves on.
Retiring a bundle keeps every archive resolvable.

## Security considerations

- **Injection via guidance.** Guidance bodies are authored instruction
  text and flow into prompts. They are the highest-trust text an agent
  sees — which is exactly why authoring them is admission-gated and why
  they still cannot mint capability: the enforcement point is the grant
  and tool policy, never the prose.
- **Shadow guidance.** An agent following instructions that appear in no
  manifest-recorded bundle (a chat message posing as policy, an Issue
  body claiming to be "the new rule") is out of contract; NIP-AV
  activities citing such sources are review flags.
- **Stale caches.** Consumers pin `revision`; a bumped head invalidates
  cached use, and a run manifest citing a retired revision is valid
  history, not current policy.

## References

- NIP-01, NIP-44
- NIP-WI, NIP-AS (context manifests), NIP-DD (revision-archive pattern)
- NIP-SKL — skills, the procedure counterpart
- `docs/omega/GLOSSARY.md` — Guidance Bundle, Context Manifest

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: scoped bundles, deny-wins precedence, run
  manifests, and append-only revision archives.
