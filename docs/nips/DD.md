> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-DD
======

Documents and Decisions
-----------------------

`draft` `optional`

This NIP defines the durable knowledge layer of the All Work system:
versioned **Documents** and recorded **Decisions**, with comments reusing
NIP-22 and file attachments reusing NIP-94/Blossom.

Two distinctions organize the NIP:

- **A Document is authored knowledge with identity and revision.** It is
  not the Work object, not an objective, and not authority: prose in a
  Document cannot grant tools, budgets, access, or release.
- **A Decision is a recorded choice.** A discussion message is not a
  Decision until recorded as one, and a Decision is not admission: acting
  on a decided choice still passes NIP-WI.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32250 | Addressable | Document (current head) |
| 32251 | Addressable (unique `d`) | Document Revision archive |
| 32252 | Addressable (unique `d`) | Decision |
| 32253-32259 | — | Reserved for future NIP-DD use |

All are authority-signed; authorship is carried in tags. Mutations flow
through NIP-WI (`document.create`, `document.revise`, `decision.record`).

## 1. Document (`kind:32250`)

The current head of one document. Address:

```text
32250:<authority_pubkey>:<document_ref>
```

### 1.1 Required tags

- `d`: stable `document_ref`
- `org`
- `title`: bounded title (or omitted for private documents)
- `revision`: current monotonic revision
- `x`: digest of the exact current body bytes
- `p` with marker `author`: the original author
- `published_at`

### 1.2 Recommended tags

- `p` with marker `editor`: subsequent contributing principals
- `a` with markers `work`, `project`, `initiative`, `team`: what the
  document is attached to
- `a` with marker `revision`: the `kind:32251` archive of the current
  revision
- `kind_label`: `spec`, `design`, `runbook`, `notes`, `retro`,
  `guide`, or deployment-declared
- `state`: `active`, `archived`
- `t`: discovery topics

`content` carries the body: public-safe Markdown, or a NIP-44 payload for
the document audience, or empty with the body off-relay behind the digest
(large documents SHOULD use a Blossom blob referenced by `x` and `url`).

### 1.3 Revisions

Every `document.revise` admission bumps `revision`, replaces the head, and
publishes a `kind:32251` archive record addressed
`32251:<authority_pubkey>:<document_ref>:rev:<n>` carrying that revision's
`x` digest, author, and time. History is therefore append-only and each
revision's exact bytes remain provable after the head moves on. An agent
that consumed a document records the `document_ref` plus `revision` in its
context manifest; a revision bump is what invalidates cached use.

## 2. Decision (`kind:32252`)

A durable record of a choice. Address:

```text
32252:<authority_pubkey>:<decision_ref>
```

### 2.1 Required tags

- `d`: unique `decision_ref`
- `org`
- `question`: bounded statement of what was being decided
- `decision`: bounded statement of what was chosen
- `p` with marker `decider`: the accountable decision maker
- `decided_at`
- `published_at`

### 2.2 Recommended tags

- `alternative`: repeated bounded statements of options not chosen
- `a` with markers `work`, `project`, `document`: affected scope
- `e` with marker `source`: the discussion message, meeting note, or
  thread the decision was distilled from
- `e`/`a` with marker `evidence`: NIP-EV refs that informed the choice
- `state`: `active`, `superseded`, `reversed`
- `a` with marker `successor`: the replacing Decision on supersession

`content` MAY carry the rationale (public-safe or encrypted). Recording
alternatives and rationale is what makes a Decision revisitable: a future
reader can tell whether the premises still hold.

### 2.3 Rules

- Reversal is a new Decision with `state=active` and the old record
  republished as `reversed` with a `successor` ref — never a deletion.
- A Decision's `decider` MUST be a principal with standing for the scope
  (checked at admission against NIP-OT membership and role policy); an
  agent can draft and propose, but the recorded decider is the
  accountable principal.
- Decisions are queryable by scope
  (`{"kinds":[32252],"#a":["<work-or-project-address>"]}`) so "why is it
  like this" has a standing answer.

## 3. Comments and attachments

- **Comments** reuse NIP-22, anchored to the Document, Decision, Issue,
  or Update address via its `a`/`e` anchors. A comment is discussion; it
  never mutates the anchored record.
- **Attachments** reuse NIP-94 file metadata and Blossom content-addressed
  storage. Each attachment ref carries its digest; access, audience, and
  retention are the attachment's own policy, and a public record MUST NOT
  embed a private attachment's bytes or bearer URL.

## Security considerations

- **Prose-as-authority.** Documents and Decisions feed agent context. The
  context compiler labels them as data; nothing written in either can
  widen tools, repositories, budgets, audiences, disclosure, release, or
  settlement.
- **Digest discipline.** A Document head without a matching archive, or a
  body that fails its `x` digest, is a Projection Issue to display, not
  content to trust.
- **Decision laundering.** Distilling a chat into a Decision is an
  interpretive act; the `source` ref keeps the original reachable so
  readers can audit the distillation.
- **Privacy.** Titles alone can leak strategy. Private knowledge publishes
  opaque heads (refs, digests, encrypted content) to restricted relays.

## References

- NIP-01, NIP-09, NIP-22, NIP-44, NIP-94, Blossom (NIP-B7)
- NIP-WK, NIP-WI, NIP-EV, NIP-OT (layer 0)
- NIP-PI, NIP-PG (this layer)
- NIP-GB (layer 3) — Guidance Bundles, the policy-bearing counterpart

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Document heads with append-only revision
  archives, Decision records with alternatives and supersession, and
  NIP-22/NIP-94 reuse for comments and attachments.
