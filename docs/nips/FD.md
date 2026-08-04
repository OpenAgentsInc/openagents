> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 4 — hosts, outcomes, and public trust.

NIP-FD
======

Findings, Verdicts, and Disclosure
----------------------------------

`draft` `optional`

This NIP defines a responsible-disclosure wire for security findings:
producer-signed **Finding Commitments** and **Candidate Findings**,
independent **Finding Verdicts**, and authority-signed **Disclosure State**
transitions.

The records keep four questions separate:

1. Did a producer commit to exact private finding bytes before revealing
   them?
2. Did an independent verifier confirm, refute, or fail to resolve the
   candidate?
3. Was the maintainer path attempted, and what disclosure step was admitted?
4. Has the exact authorized revision been published?

A signature proves the signed bytes. It does not prove that the signer is a
producer, verifier, maintainer, or disclosure authority. Relay acceptance is
transport evidence only and grants no authority. A finding is not generic
evidence, a verdict is not owner acceptance, and disclosure is not release,
public-claim, credit, payment, or settlement authority.

## Authority model

- Finding Commitments and Candidate Findings are signed by the finding
  **producer**. The signature fixes attribution and bytes; it does not confirm
  the finding.
- Finding Verdicts are signed by the **verifier** that performed the named
  evaluation. The verifier key MUST differ from the producer key, and an
  independence-requiring policy MUST also reject keys that resolve through
  NIP-OT membership or Block NIP-OA attestation to the same controlling
  principal.
- Disclosure State records are signed by the Organization's declared
  `disclosure_authority` key. The Organization record MAY designate the All
  Work authority or a distinct key. A record from any other key is a proposal,
  not canonical disclosure state.
- A maintainer acknowledgement has maintainer authority only when its signer
  is bound to the target by a resolvable target-owned record. An out-of-band
  acknowledgement can be cited as digest-bound evidence, but clients MUST
  label it authority-attested rather than maintainer-signed.
- The disclosure authority admits transitions under a named policy revision.
  It does not inherit producer, verifier, maintainer, owner-disposition,
  promise-registry, release, or settlement authority.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32470 | Addressable (unique `d`) | Finding Commitment |
| 32471 | Addressable (unique `d` per revision) | Candidate Finding |
| 32472 | Addressable (unique `d`) | Finding Verdict |
| 32473 | Addressable (unique `d`) | Disclosure State transition |
| 32474-32479 | — | Reserved for future NIP-FD use |

## Common identifiers and references

- `finding_ref` is a stable, producer-scoped, opaque identifier. It SHOULD
  contain at least 128 bits of randomness and MUST NOT encode a target,
  weakness class, severity, or maintainer identity.
- Candidate revisions are positive dense integers scoped to
  `(producer_pubkey, finding_ref)`.
- `x` is a lowercase hexadecimal SHA-256 digest of exact bytes.
- `e` and `a` references use markers such as `commitment`, `candidate`,
  `verdict`, `maintainer_evidence`, `authorization`, `public_candidate`,
  `evidence`, and `previous`.
- `org` identifies the coordinating Organization. An embargoed record MUST
  use an opaque Organization or workroom ref if the ordinary ref would expose
  the target or campaign.
- `published_at`, `committed_at`, `verified_at`, `occurred_at`, and
  `admitted_at` are Unix timestamps. They are signed claims, not trusted
  ordering by themselves.

Unknown tags and deployment-declared enum values MUST be preserved. A client
MUST render an unknown value as unknown and MUST NOT reinterpret it as a
successful, public, or terminal state.

## 1. Finding Commitment (`kind:32470`)

A producer's hiding commitment to one exact Candidate Finding revision. Its
address is:

```text
32470:<producer_pubkey>:<finding_ref>:commit:<revision>
```

### 1.1 Required tags

- `d`: `<finding_ref>:commit:<revision>`
- `revision`: the Candidate Finding revision this commitment binds
- `x`: the commitment digest defined in 1.3
- `committed_at`

### 1.2 Recommended tags

- `org`: only when the value is safe to disclose to the relay audience
- `a` with marker `workroom`: the restricted NIP-29 workroom, only on relays
  where membership itself is not sensitive
- `e` or `a` with marker `anchor`: an authority-ordered NIP-WK Work Event or
  another policy-admitted ordering anchor
- `t`: public-safe discovery topics such as `security-finding-commitment`;
  target, weakness, and severity topics are prohibited before publication

`content` MUST be empty. A commitment event MUST NOT carry the target,
location, mechanism, severity, CWE, assumptions, evidence refs, maintainer,
audience membership, embargo time, or a deterministic unsalted fingerprint.

### 1.3 Commitment construction

The producer generates a fresh 32-byte random `opening_nonce`, retains the
exact UTF-8 `finding_bytes` used as Candidate Finding content, and computes:

```text
finding_x   = SHA256(finding_bytes)
commitment_x = SHA256(
  UTF8("openagents.finding-commitment.v1\0") ||
  opening_nonce ||
  finding_x
)
```

`x` is `commitment_x`. The nonce is private until the revision is revealed.
The Candidate Finding carries `finding_x` and, inside its protected payload,
the nonce and exact `finding_bytes`. A public reveal carries the nonce in an
`opening` tag and uses the exact `finding_bytes` as `content`.

Randomness is mandatory. A deterministic digest of a guessed target and CWE
is not a hiding commitment. Separate producers normally use separate nonces;
duplicate candidates are compared only after their authorized openings and
finding documents are available to the same audience.

### 1.4 Ordering and credit

Every Candidate Finding revision MUST cite a resolving Commitment from the
same producer. The Commitment MUST have been admitted or first observed before
the Candidate was revealed to that audience. `created_at` and `committed_at`
alone cannot prove this order because a signer can backdate them.

Where credit or duplicate-work priority depends on order, policy MUST use an
authority-ordered anchor or a relay first-seen receipt retained by the
audience. Without one, clients display `commitment_claimed`, not
`precommitment_proven`. Publication time MUST NOT replace commitment order as
the credit order.

## 2. Candidate Finding (`kind:32471`)

The producer's exact security claim. Its stable revision address is:

```text
32471:<producer_pubkey>:<finding_ref>:rev:<revision>
```

The same address has two permitted representations:

- `visibility=embargoed` or `restricted`: `content` is a NIP-44 encrypted
  `openagents.finding-envelope.v1` JSON object containing
  `opening_nonce` as lowercase hexadecimal and `finding_b64u` as unpadded
  RFC 4648 base64url of the exact `finding_bytes`.
- `visibility=public`: `content` is the exact UTF-8 `finding_bytes`, and the
  `opening` tag exposes the nonce so anyone can verify the Commitment.

The public representation is an authorized reveal of the same bytes, not a
new finding revision. It therefore keeps the same `d`, `revision`, and
`finding_x`.

### 2.1 Required tags

- `d`: `<finding_ref>:rev:<revision>`
- `revision`: a positive dense revision
- `a` with marker `commitment`: the matching `kind:32470` address
- `finding_x`: SHA-256 of the exact unencrypted `finding_bytes`
- `visibility`: `embargoed`, `restricted`, or `public`
- `published_at`: publication to this event's current audience

A public representation additionally requires:

- `opening`: the 32-byte commitment nonce as 64 lowercase hexadecimal
  characters
- `e` or `a` with marker `authorization`: the resolving
  `publication_authorized` Disclosure State for this exact revision and
  `finding_x`

### 2.2 Recommended tags

- `org`, when audience-safe
- `a` with marker `work`: the related security Work
- `a` with marker `previous`: the prior Candidate revision
- `e` or `a` with marker `verdict`: Finding Verdicts about this exact revision
- `e` or `a` with marker `evidence`: NIP-EV receipts or restricted evidence
  records
- `a` with marker `workroom`: the NIP-29 disclosure workroom
- after public authorization only, `target`, `cwe`, `severity`, and `t` tags
  copied from the finding document for discovery

Protected Candidate events SHOULD expose only `d`, `revision`, the opaque
Commitment ref, `finding_x`, `visibility`, and audience-safe routing tags.
Implementations MUST NOT copy protected target or weakness fields into clear
tags.

### 2.3 Finding document

`finding_bytes` MUST decode as a JSON object with schema
`openagents.finding.v1` and these fields:

- `target`: an exact target ref and revision, including a source-tree or
  artifact digest when available
- `mechanism`: the bounded claim explaining how the weakness can occur
- `location`: exact or bounded affected components; it MAY be omitted only
  when revealing it would exceed the admitted audience
- `severity`: `critical`, `high`, `medium`, `low`, `informational`, or
  `unknown`, explicitly labeled as a producer proposal, plus its scoring
  system or rationale
- `cwe`: a nonempty array of CWE identifiers, `unknown`, or `not_applicable`
- `evidence_boundary`: what evidence exists, what an authorized verifier can
  obtain, what is withheld, and why
- `assumptions`: a possibly empty array of conditions required for the claim
- `impact`: a bounded consequence statement
- `reproduction_boundary`: safe reproduction conditions and prohibited
  actions

Evidence bytes, exploit material, credentials, private source, customer data,
and maintainer contact details stay outside the finding document unless the
exact disclosure audience is authorized to receive them. The document cites
digest-bound evidence instead.

### 2.4 Revision and reveal rules

- Revision `1` cites commitment `1`. Revision `n` MUST cite revision `n-1`
  and a Commitment for `n` that preceded its reveal. A missing revision is an
  explicit history gap.
- A material change to target, mechanism, affected location, impact,
  evidence boundary, assumptions, or reproduction boundary creates a new
  revision and a new prior Commitment. It MUST NOT replace the committed
  plaintext under an existing revision.
- Correcting severity or CWE after a verdict also creates a revision. Prior
  Candidates and Verdicts remain historical facts.
- An embargoed or restricted Candidate MUST be encrypted even on a restricted
  relay. It MUST be sent only through the admitted NIP-29 audience and a relay
  whose policy admits that audience. NIP-44 protects content, not event
  metadata.
- Embargoed Candidate ciphertext MUST NOT be published to a public relay.
  Public relays receive only the minimal Commitment and, after explicit
  authorization, the public representation.
- A public representation MUST reproduce the exact committed finding bytes.
  New public prose requires a new committed revision.

## 3. Finding Verdict (`kind:32472`)

An independent verifier's judgment about one exact Candidate Finding
revision. Its address is:

```text
32472:<verifier_pubkey>:<finding_ref>:verdict:<verdict_ref>
```

### 3.1 Required tags

- `d`: `<finding_ref>:verdict:<verdict_ref>`, unique for the verifier
- `e` or `a` with marker `candidate`: the exact Candidate event or address
- `subject_revision` and `finding_x`: the evaluated revision and plaintext
  digest
- `verdict`: `confirmed`, `refuted`, or `inconclusive`
- `policy`: the evaluation policy and revision
- `verified_at`

### 3.2 Recommended tags

- `org`, `a` with marker `work`, and `a` with marker `workroom`
- `e` or `a` with marker `evidence`: each NIP-EV receipt or other item
  evaluated
- `x`: digest of the verifier's full report
- `scope`: what portion of the Candidate was evaluated
- `reason`: a bounded reason code, especially for `refuted` or
  `inconclusive`
- `env`: exact environment or target revision used
- `a` or `e` with marker `supersedes`: a prior Verdict corrected by this one

`content` carries a public-safe summary or a NIP-44 encrypted report for the
same disclosure audience as the Candidate. It MUST NOT reveal more finding
detail than the current Disclosure State admits.

### 3.3 Verdict rules

- The verifier pubkey MUST differ from the Candidate producer pubkey. A
  deployment claiming principal independence MUST resolve identity bindings
  and reject common control. Merely rotating a key does not create an
  independent verifier.
- `confirmed` requires executed evaluation under `policy` and resolvable
  evidence for the claimed scope. Model confidence or agreement is diagnostic
  signal, not confirmation.
- `refuted` means the named revision failed the named evaluation. It does not
  delete the Candidate, prove that all variants are safe, or authorize public
  disclosure.
- `inconclusive` preserves uncertainty. It MUST NOT be folded into confirmed
  or refuted.
- Verdicts are append-only. A correction publishes a new Verdict with a
  `supersedes` ref. Conflicting Verdicts remain visible; policy may request
  another verifier but MUST NOT silently choose the favorable result.
- A Verdict is security-domain judgment. It can contribute evidence to a
  NIP-EV Verification Receipt, but it is not that receipt and never supplies a
  NIP-EV Owner Disposition.

## 4. Disclosure State (`kind:32473`)

An append-only, disclosure-authority-signed transition for one Candidate
revision. Its address is:

```text
32473:<disclosure_authority_pubkey>:<finding_ref>:rev:<revision>:state:<seq>
```

### 4.1 Required tags

- `d`: `<finding_ref>:rev:<revision>:state:<seq>`
- `org`
- `finding_ref`, `subject_revision`, and `finding_x`
- `seq`: a positive dense sequence scoped to the Candidate revision
- `from` and `to`: Disclosure State values
- `e` or `a` with marker `candidate`: the exact Candidate revision
- `policy`: disclosure policy revision
- `p` with marker `actor`: the principal whose admitted action caused the
  transition
- `occurred_at` and `admitted_at`

For `seq > 1`, an `e` or `a` ref with marker `previous` is also required.

### 4.2 Recommended tags

- `a` with marker `workroom` and `a` with marker `work`
- `e` or `a` with marker `verdict`: the Verdicts considered
- `e` or `a` with marker `maintainer_evidence`: contact delivery,
  acknowledgement, fix, or unreachability evidence
- `p` with marker `maintainer`: only when safe for the event audience
- `maintainer_status`: `attempted`, `delivered`, `acknowledged`, `declined`,
  `unreachable`, or `unknown`
- `embargo_expires_at`: a planning deadline only
- `reason`: a bounded transition reason
- `scope`: the exact audience or public fields authorized
- `e` with marker `public_candidate`: required on `to=published`

Before publication, Disclosure State events stay on the restricted relay and
use encrypted `content` for protected rationale. A public `published` event
MUST contain only public-safe tags and content.

### 4.3 States

- `embargoed`: the exact Candidate revision exists only for its admitted
  disclosure audience
- `reported_to_maintainer`: a documented maintainer contact path was attempted
- `acknowledged`: maintainer acknowledgement evidence resolves
- `fix_available`: a maintainer or coordinating authority has bound evidence
  of an available fix to the Candidate revision
- `publication_authorized`: an explicit human or role-admitted decision
  authorizes a named public payload and scope
- `published`: the exact authorized public Candidate event resolves
- `withdrawn`: the producer or disclosure authority ended this Candidate
  revision before publication, with a reason

`published` and `withdrawn` are terminal for that Candidate revision. A
correction or newly discovered detail uses a new committed Candidate revision
and its own sequence beginning at `embargoed`.

### 4.4 Permitted transitions

| From | Permitted `to` values |
| --- | --- |
| none | `embargoed` |
| `embargoed` | `reported_to_maintainer`, `withdrawn` |
| `reported_to_maintainer` | `acknowledged`, `fix_available`, `publication_authorized`, `withdrawn` |
| `acknowledged` | `fix_available`, `publication_authorized`, `withdrawn` |
| `fix_available` | `publication_authorized`, `withdrawn` |
| `publication_authorized` | `published`, `withdrawn` |
| `published` | none |
| `withdrawn` | none |

A transition outside this table is noncanonical. Repeating the same state
requires a new Candidate revision only when protected bytes changed; ordinary
progress notes belong in Work or evidence records.

### 4.5 Transition gates

- `reported_to_maintainer` requires a digest-bound contact attempt. The
  contact destination and message body remain private.
- `acknowledged` requires resolving acknowledgement evidence. Relay delivery
  alone is not acknowledgement.
- `fix_available` requires an exact fix revision or artifact digest. It does
  not mean the fix is correct, released, or accepted.
- `publication_authorized` requires a prior
  `reported_to_maintainer` state, the exact `finding_x`, an explicit actor,
  and a public scope. An unreachable or unresponsive maintainer can satisfy
  the attempted-path predicate only when the policy admits the documented
  attempt; silence is never acknowledgement.
- The public Candidate event MUST cite the `publication_authorized` event.
  The following `published` transition MUST cite that public Candidate event
  and verify its Commitment opening.
- A target or maintainer can opt into earlier or broader disclosure through a
  signed, policy-admitted decision. That opt-in does not publish bytes by
  itself and cannot disclose third-party secrets or expand another
  principal's authority.

### 4.6 Embargo rule

`embargo_expires_at` is a deadline for human and policy review. It is never an
automatic transition. Reaching it MUST NOT decrypt, republish, submit, enqueue,
or authorize a Candidate. NIP-40 expiration, a scheduler, relay retention, or
client wall-clock logic cannot create `publication_authorized` or `published`.
If the deadline passes with no decision, the Candidate remains in its current
state and clients display `embargo_overdue`.

## 5. Lifecycle and version integrity

- Each Candidate revision has one gap-free Disclosure State sequence.
  Clients MUST surface missing or conflicting `seq` values and stop their fold
  at the last unambiguous transition.
- `from` MUST equal the preceding event's `to`, and `previous` MUST resolve to
  that event. Relay arrival and event `created_at` do not repair a gap.
- Every Verdict and Disclosure State binds both `subject_revision` and
  `finding_x`. A later Candidate revision makes neither record current for the
  new bytes.
- Deletion requests do not retract a Commitment, Verdict, or public reveal.
  Clients MAY honor NIP-09 for local display while preserving the fact that
  previously disclosed bytes cannot be made secret again.
- A compromised authority can sign false transitions. Clients pin the
  disclosure authority from NIP-OT, honor recorded rotations, and retain the
  append-only sequence for audit.

## 6. Composition

- **NIP-WK:** Security Work can cite Commitments and restricted Candidates.
  Authority-ordered Work Events can anchor commitment order. Work state does
  not change Finding or Disclosure State.
- **NIP-EV:** Evidence Receipts bind reports, reproductions, fixes, and contact
  evidence. Independent Verification Receipts and Owner Dispositions remain
  separate. A Finding Verdict or Disclosure State cannot stand in for either.
- **NIP-OT, NIP-AD, and Block NIP-OA:** These records bind Organization keys,
  grants, roles, and controlling principals used by the authority and
  independence checks.
- **NIP-29, NIP-44, and NIP-59:** Workrooms define restricted audiences;
  encryption and metadata-minimizing delivery protect embargoed material.
  Encryption never grants membership or disclosure authority.
- **NIP-PP and release systems:** A published finding can become evidence for
  a promise transition, incident, release, or public claim only through that
  system's own authority record.
- **NIP-SI:** A confirmed finding can motivate a Security Invariant, and a
  Regression Watch observation can motivate a new Commitment. Neither link
  changes the other record's disposition.

## Security and privacy considerations

- **Metadata is disclosure.** NIP-44 does not hide event kind, signer, time,
  size, tags, relay, or audience traffic. Protected records use opaque refs,
  padding where the delivery profile admits it, restricted relays, and the
  smallest audience.
- **Commitment guessing.** Fresh 32-byte nonces and no clear target tags
  prevent dictionary comparison against low-entropy finding descriptions.
  Nonces MUST come from a cryptographically secure generator and MUST NOT be
  reused.
- **Public-relay leakage.** Embargoed ciphertext, maintainer contact state,
  exact targets, weakness classes, and evidence locations stay off public
  relays. A public relay receives a minimal Commitment and explicitly
  authorized public records only.
- **False identity separation.** Distinct keys can share one operator.
  Policies that claim independent verification resolve principal bindings and
  record the independence basis.
- **Unsafe reproduction.** A verifier's grant, target scope, and reproduction
  boundary constrain testing. A Candidate is not authorization to exploit,
  access data, disrupt a service, or exceed a maintainer's grant.
- **Retraction illusion.** Once plaintext reaches a public relay, deletion,
  withdrawal, expiration, or a later Verdict cannot make it private. Clients
  preserve corrections without pretending the earlier disclosure vanished.
- **Authority laundering.** A relay badge, encrypted message, maintainer
  silence, model score, confirmed Verdict, fix commit, or elapsed timer cannot
  manufacture a Disclosure State transition.

## References

- NIP-01, NIP-09, NIP-29, NIP-40, NIP-44, NIP-59
- NIP-WK, NIP-EV, NIP-OT, NIP-AD, NIP-PP (this program)
- NIP-SI: Security Invariants and Regression Watch (this program)
- Block NIP-OA: agent and principal attestation
- `docs/hardening/2026-08-04-nostr-native-hardening-program.md` — program
  design, disclosure boundary, roadmap, and falsifiers

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: hiding Finding Commitments, encrypted and
  revision-bound Candidate Findings, independent Verdicts, and a dense
  maintainer-first Disclosure State machine with no timer-driven publication.
