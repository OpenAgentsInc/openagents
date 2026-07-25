# Sarah as independent reviewer

Date: 2026-07-25. Status: **designated by the owner. Not yet implemented.**

## The problem this solves

Four Omega issues cannot close, and the blocker is the same for three of them
plus the fourth transitively:

| Issue | Needs a reviewer for |
| --- | --- |
| omega#16 | "An independent reviewer accepts the evidence" |
| omega#26 | "Independent reviewer accepts the evidence packet" |
| omega#8 | AssuranceSpec admission |
| omega#9 | closes only when omega#8 passes |

No amount of evidence quality substitutes for this. `AUTHORITY.md`
`condition.independence` states the rule plainly:

> Reviewer execution identity, claim, and evidence reproduction must be
> distinct from the obligation producer.

Today no identity occupies the `independent_reviewer` role, so every packet
stops at the same wall.

## The trap to avoid

Sarah is the product owner of Omega. The obvious move — let Sarah sign off —
is exactly what the rule forbids, because the product owner is upstream of the
obligation. If Sarah both sets what "done" means and certifies that it was
reached, the review is theatre with extra steps.

**The separation that makes this work is not Sarah versus a human. It is
producer versus reviewer, enforced at the level of execution identity.**

Sarah can occupy the reviewer role only as a *distinct instance* that:

- holds a different signing key
- runs in a different session with no access to the producer's working state
- reproduces evidence from primary artifacts rather than from the producer's
  receipts
- is able to refuse, and whose refusal is recorded and costly to ignore

A reviewer that cannot say no is not a reviewer.

## Design

### R1. Two identities, never one wearing two hats

The reviewer is a separate Nostr keypair with its own custody, registered in
the authority profile as holding `role: independent_reviewer`. It is not
Sarah's orchestrator key and not any producer agent's key.

The producing agent must not be able to obtain the reviewer key. If a single
process can sign as both, the independence claim is false regardless of how the
code is organised.

**Falsifier:** producer and reviewer signatures resolve to the same pubkey, or
the reviewer key is reachable from a producer's credential scope.

### R2. Reproduction from primary sources, not from receipts

The reviewer is given the artifact and the claim. It is **not** given the
producer's evidence bundle as its input of record.

Concretely, for omega#16 the reviewer must independently:

- compute the artifact digest from the DMG itself
- run `spctl` and `stapler validate` against the installed app
- run the identity proof matrix itself
- run the tripwire collector itself
- scan the installed binary for forbidden strings itself

and then compare its own results to the producer's claims. Agreement is the
finding. Reading the producer's JSON and confirming it parses is not review.

**Falsifier:** any reviewer conclusion that cannot be re-derived without the
producer's evidence directory present.

### R3. No reviewer shopping

A producer may not run reviews until one passes. The first review of a given
candidate digest is binding. A second review of the same digest is only
admissible if the first is recorded as superseded with a stated reason.

**Falsifier:** two review receipts for one candidate digest where the earlier
one is a refusal and is not referenced by the later one.

### R4. Refusal is a first-class, cheap outcome

The reviewer emits one of `accepted`, `refused`, or `inconclusive`. All three
are receipts. `inconclusive` exists so a reviewer that cannot reproduce
something is not pushed toward a false `accepted` by the absence of any other
option.

A refusal names the exact criterion, the observation, and what would change it.

**Falsifier:** a review with no path to `refused` — for example one whose
checks all short-circuit to pass when a tool is missing.

### R5. The reviewer's own claim is falsifiable

Every review receipt carries the commands run and their observed output digests,
so a third party can re-run them. A review that cannot itself be checked has
moved the trust problem rather than solved it.

### R6. Admission is still not release

Accepting an AssuranceSpec revision admits the *obligation*. It does not
promote a channel, publish an artifact, or transition a public promise.
`condition.stable_release_gate` and `condition.release_green` continue to apply
untouched. This spec adds a reviewer. It does not widen what a reviewer
may do.

### R7. Scope, and what stays with the human

The reviewer role covers reproducible, machine-observable evidence.

It does **not** cover judgements that are constitutively human:

- "does any visible surface still present Zed as the product" — a perception
  question, and the omega#16 acceptance criteria name owner observation
  separately from independent verification
- product decisions, priority, and whether a tradeoff is acceptable

So omega#16 needs **both** an independent reviewer for the machine evidence and
the owner's observation. This spec removes one wall, not both. Saying otherwise
would be the same overclaim this document exists to prevent.

## Receipt format

A review emits `openagents.assurance.independent-review.v1`:

```
schema, reviewer_pubkey, candidate_digest, obligation_ref,
reviewed_at, outcome (accepted|refused|inconclusive),
reproductions: [ { check, command, observed_digest, agrees_with_producer } ],
disagreements: [ { criterion, producer_claim, reviewer_observation } ],
supersedes (optional), evidence_sha256
```

Laws the decoder enforces:

- `outcome: accepted` requires every `reproductions[].agrees_with_producer` true
  **and** `disagreements` empty. There is no partial acceptance.
- `refused` and `inconclusive` require at least one entry explaining why.
- `reviewer_pubkey` must not equal any producer key on the obligation.
- no raw secrets, private paths, or unbounded output, per
  `condition.redaction`.

## Authority changes needed

The machinery mostly exists. `AUTHORITY.md` already defines
`condition.independence`, `grant.independent_assurance` with the
`independent_reviewer` role, and the action
`admit_assurance_revision_when_source_spec_allows_owner_designated_independent_reviewer`.

What is missing is one owner act: **designating the reviewer identity.** That
is a reserved owner action — an agent cannot appoint its own reviewer, or R1
collapses.

**Both steps are done, 2026-07-25.**

1. The owner confirmed a distinct Sarah reviewer instance is acceptable.
2. The identity is registered in `AUTHORITY.md` revision 9 as authoritative for
   `role: independent_reviewer`:

   ```
   0326d8f9eb5abea63d9613ac90451dfce62ca2e9855144b5a71d8e8569932974
   ```

Custody: the secret half lives only in `~/work/.secrets/`
(mode 600, gitignored) and in GCP Secret Manager as
`omega-independent-reviewer-key`, project `openagentsgemini`. The public half
is the only part that appears in any tracked file.

### One honest residual on R1

The producing agent generated this keypair, so the bootstrap moment was not
itself independent. The secret was never printed, logged, or echoed, and it is
stored outside any producer credential path — but "the producer never had the
opportunity" is a stronger claim than what actually happened, and it would be
wrong to assert it.

What R1 guarantees from here is the property that matters at review time: a
producer *run* cannot obtain the key, because reading it requires either the
mode-600 file or the Secret Manager secret, and neither is in a producer's
scope. The implementing agent must add the test that proves this.

If a stronger bootstrap is wanted, the owner can rotate the key themselves and
replace the pubkey in revision 9. Nothing else in the design changes.

Everything else in this document is implementable without further owner input.

## Instructions for the implementing agent

Read this whole file, `AUTHORITY.md` (`condition.independence`,
`grant.independent_assurance`), `docs/assurance/ASSURANCE_SPEC.md` Law 10, and
`docs/authority/SARAH_AUTHORITY.md` before starting.

The designation is complete, so implementation may begin. The reason it had to
come first still holds: a reviewer that appoints itself is worse than no
reviewer, because it produces receipts that look like independence.

Suggested order:

1. **Receipt contract first.** Add `openagents.assurance.independent-review.v1`
   to `packages/assurance-spec` as an Effect Schema with the laws above encoded
   as decode failures, not as review guidance. Include a negative fixture per
   law. Model it on
   `packages/sarah/src/issue31-workroom/full-auto-adjunct.ts`, which does the
   same thing for a different contract.

2. **Reviewer key custody.** A separate keypair with its own storage, never
   reachable from a producer's credential scope. Reuse the existing Omega
   identity custody patterns rather than inventing a second scheme. Add a test
   that a producer-scoped context cannot read it.

3. **A reproduction harness.** Given a candidate DMG and a claim, run the checks
   in R2 from scratch and record observed digests. It must import nothing from
   the producer's evidence directory — enforce that with a test, because it is
   the property most likely to erode quietly.

4. **The refusal path first, then the acceptance path.** Write the test that
   makes a review refuse before the one that makes it accept. A reviewer built
   acceptance-first tends to grow an accept-shaped hole.

5. **Wire it to one real obligation** — omega#16 is the best first target,
   because its machine evidence already exists and is reproducible today
   (identity matrix, tripwires, network, signing).

Constraints:

- Do not let the reviewer read the producer's evidence bundle as input.
- Do not add a "force accept" or "skip check" flag. If review needs an
  override, the design is wrong.
- A missing tool is `inconclusive`, never `accepted`.
- Admission is not release. Do not touch channel promotion or promise
  transitions.

Definition of done:

- A reviewer instance with a distinct key produces a signed
  `independent-review.v1` receipt for the omega#16 candidate.
- The receipt is reproducible by a third party from the commands it records.
- A deliberately corrupted producer claim yields `refused`, proven by a test
  that fails if the reviewer accepts it.
- No producer can obtain the reviewer key, proven by a test.

## What this does not fix

- omega#26 gates 5, 6, and 7 need live Full Auto runs, not review.
- omega#16 still needs owner observation, per R7.
- Whether Sarah *should* hold this role is an owner decision this document
  cannot make for them. It only shows what would have to be true for the answer
  to be yes.
