> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 4 — hosts, outcomes, and public trust.

NIP-SI
======

Security Invariants and Regression Watch
----------------------------------------

`draft` `optional`

This NIP defines durable security controls as three signed record families:
versioned **Security Invariants**, producer-signed **Artifact Provenance
Witnesses**, and recurring **Regression Watches** with explicit freshness,
stopping, and loss accounting.

A source scan reports what one tool observed once. A Security Invariant names
a property that must keep holding across exact source, configuration, build,
artifact, and runtime boundaries. A Provenance Witness binds a measurement to
those exact inputs and outputs. A Regression Watch repeats the check as the
target changes and records every observed, missed, lost, and stopped slot.

These records preserve the trust boundaries of NIP-EV:

- an Invariant is a policy definition, not proof that the property holds;
- a Witness is producer-signed evidence, not independent verification;
- an observed Watch result, including an observed regression, is not an owner
  disposition, finding verdict, release decision, or public claim; and
- relay acceptance is transport evidence only and grants no authority.

## Authority and signer model

- Security Invariant revisions and Regression Watch records are canonical
  only when signed by the Organization's NIP-OT-declared
  `security_invariant_authority` and `regression_watch_authority` keys. One key
  MAY hold both roles or the All Work authority MAY be declared for either.
- Artifact Provenance Witnesses are signed by the principal that produced the
  artifact measurement. Its signature identifies the producer and binds the
  record; it does not make the producer an authority or verifier.
- A Watch authority admits a producer's Witness into an observation event.
  Admission means that the observation belongs to the canonical watch stream.
  It does not verify the Witness or its conclusion.
- Independent verification uses NIP-EV. The verifier pubkey MUST differ from
  every producer key whose Witness it evaluates. A policy claiming principal
  independence MUST also reject keys that resolve through NIP-OT membership
  or Block NIP-OA attestation to the same controlling principal.
- Authority keys do not inherit maintainer, Work-owner, promise-registry,
  release, acceptance, or settlement authority.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32480 | Addressable (unique `d` per revision) | Security Invariant |
| 32481 | Addressable (unique `d`) | Artifact Provenance Witness |
| 32482 | Addressable (unique `d`) | Regression Watch definition or event |
| 32483-32489 | — | Reserved for future NIP-SI use |

## Common identifiers and tags

- `invariant_id`: a stable Organization-scoped identifier
- `watch_ref`: a stable Organization-scoped Regression Watch identifier
- `revision`: a positive dense revision scoped to its Invariant or Watch
- `seq`: a positive dense event sequence scoped to a Watch
- `x`: a lowercase hexadecimal SHA-256 digest of exact bytes
- `org`: the coordinating Organization
- `target`: a target record, repository, package, device, or artifact family
- `target_revision`: the exact source or release revision evaluated
- `e` and `a` refs with markers including `invariant`, `definition`,
  `witness`, `evidence`, `verification`, `previous`, `work`, and `successor`
- `published_at`, `produced_at`, `occurred_at`, and `admitted_at`: Unix
  timestamps, which remain signed claims

Parsers MUST preserve unknown tags, invariant families, witness classes,
result values, and Watch events. Unknown values never mean that an Invariant
holds, a check is fresh, or a Watch ended successfully.

## 1. Security Invariant (`kind:32480`)

A named, versioned property and its falsification contract. Its address is:

```text
32480:<invariant_authority_pubkey>:<invariant_id>:rev:<revision>
```

Each revision is individually addressable and immutable. Consumers derive the
current revision from the highest gap-free, authority-signed revision chain,
not from relay arrival order.

### 1.1 Required tags

- `d`: `<invariant_id>:rev:<revision>`
- `invariant`: stable `invariant_id`
- `org`
- `revision`: positive dense revision
- `family`: an invariant family from 1.4
- `state`: `draft`, `active`, `superseded`, or `retired`
- `x`: SHA-256 of the exact UTF-8 invariant document in `content`
- `published_at`

For `revision > 1`, an `e` or `a` reference with marker `previous` is also
required.

### 1.2 Recommended tags

- `title`: bounded public-safe name
- `target`: the target class or exact target ref
- repeated `layer`: `source`, `configuration`, `build`, `artifact`, or
  `runtime`
- repeated `witness_class`: required witness classes
- repeated `falsifier`: bounded machine-readable falsifier identifiers
- `consequence`: bounded failure consequence
- `a` with marker `work`: Work that authored or changed the Invariant
- `a` with marker `successor`: replacement Invariant on `superseded`
- `reason`: required in practice for weakening, superseding, or retiring an
  active Invariant
- `t`: discovery topics

### 1.3 Invariant document

`content` MUST be the exact UTF-8 JSON document whose digest appears in `x`.
It has schema `openagents.security-invariant.v1` and these required fields:

- `name`: a stable human-readable property name
- `statement`: the positive property that must hold
- `scope`: exact targets and applicability selectors
- `layers`: one entry for each of `source`, `configuration`, `build`,
  `artifact`, and `runtime`; an inapplicable layer carries
  `not_applicable` and a rationale instead of being omitted
- `failure_consequence`: the security consequence if the property fails
- `assumptions`: conditions under which the Invariant applies
- `required_witnesses`: the required witness classes, measurements, binding
  fields, and pass predicates for each applicable layer
- `falsifiers`: explicit observations that are sufficient to show violation,
  including safe reproduction limits
- `evaluation`: an exact procedure, executable artifact digest, or
  versioned policy ref used to evaluate the witnesses
- `independence`: which observations require an independent NIP-EV verifier

A useful Invariant is falsifiable. “The product is secure” is not an
Invariant. “Every release artifact's linked entropy symbol resolves to the
board RNG implementation named by the source and build manifests” is an
Invariant when it names the symbol measurement, exact bindings, expected
result, and falsifiers.

### 1.4 Baseline families

The baseline vocabulary is:

- `entropy_provenance`
- `nonce_generation`
- `security_downgrade`
- `security_code_reachability`
- `build_source_divergence`
- `parser_memory_safety`
- `signature_validation`
- `update_downgrade_path`
- `side_channel`
- `dependency_substitution`

Deployments MAY add families. A family is a discovery label, not an
evaluation result or severity score.

### 1.5 States and revision rules

- `draft`: authored but not yet required by the target's policy
- `active`: currently required by the named target policy
- `superseded`: replaced by an explicit successor Invariant or revision
- `retired`: no longer required, with a reason and residual-risk statement

Revision `1` begins as `draft` or `active`. A later revision MUST cite its
immediate predecessor and increment by exactly one. Permitted state progress
is `draft -> active|superseded|retired` and
`active -> active|superseded|retired`. `superseded` and `retired` are terminal
for that revision chain; a replacement uses its cited successor chain.

Changing a statement, scope, layer applicability, witness binding, pass
predicate, falsifier, evaluation procedure, or independence requirement
creates a new revision. Existing Witnesses and Watch observations remain facts
about their exact Invariant revision and MUST NOT be presented as current for
the new revision.

Weakening an active pass predicate or removing a falsifier is a security-policy
change. The new revision MUST carry a reason and an owner or target-policy
decision ref where that policy requires one. A failing observation MUST NOT be
made to pass by quietly revising the Invariant.

An observed violation does not mutate the Invariant's state. The property can
remain active while the target is known to violate it.

## 2. Artifact Provenance Witness (`kind:32481`)

A producer-signed measurement that binds one built artifact to exact source,
build definition, effective configuration, and artifact bytes. Its address is:

```text
32481:<producer_pubkey>:<witness_ref>
```

The producer can be a builder, test runner, hardware lab, maintainer, or agent
acting under an admitted grant. The Witness says what that producer observed;
it is not self-verification.

### 2.1 Required tags

- `d`: unique `witness_ref`
- `org`
- `e` or `a` with marker `invariant`: the exact Security Invariant revision
- `target` and `target_revision`
- `witness_class`: one of `source_to_build`, `configuration_binding`,
  `symbol_presence`, `symbol_resolution`, `artifact_measurement`,
  `runtime_binding`, `dependency_closure`, or deployment-declared
- `source_x`: digest of the exact source-tree manifest or source archive bytes
- `build_x`: digest of the exact build-input manifest described in 2.3
- `artifact_x`: digest of the exact built or shipped artifact bytes
- `config_x`: digest of the exact effective-configuration manifest
- `x`: digest of the exact measurement output bytes
- `produced_at`

### 2.2 Recommended tags

- `source_ref`: commit, tree, archive, or Materialized Source Set ref used for
  discovery; it supplements and never replaces `source_x`
- `artifact_ref`: release, package, firmware, image, or binary identity
- `toolchain_x`: digest of the compiler, linker, build tools, and lock data
  manifest
- `symbols_x`: digest of exact symbol-table or linkage output
- `dependency_x`: digest of the resolved dependency closure
- `runtime_x`: digest of runtime measurement bytes when required
- `env`: platform, board, architecture, container, or execution-environment
  identity
- `e` or `a` with marker `evidence`: NIP-EV Evidence Receipt for the complete
  output bytes
- `a` with marker `work` and `a` with marker `session`
- `url`: an audience-authorized location for exact bytes
- `result`: `observed`, `not_observed`, `inconclusive`, or
  deployment-declared; the Invariant predicate determines whether it passes

### 2.3 Exact binding rules

`build_x` MUST digest a manifest that identifies the build recipe, command
graph, toolchain, dependency closure, environment selectors, and input digests.
`config_x` MUST digest the resolved effective configuration, including defaults
and generated configuration. A manifest that says “default config” or “current
toolchain” is not exact.

If a field has no input for this build, its manifest records an explicit
`not_applicable` entry and rationale; absence is not represented by omitting a
required digest. Symlinks, submodules, generated inputs, vendored code, build
flags, link scripts, and post-processing steps belong in the appropriate
manifest whenever they can affect the artifact.

The Witness body is the exact measurement output named by `x`, or a NIP-44
envelope containing it. A `symbol_resolution` Witness, for example, includes
the exact symbol query and output that demonstrate which implementation won
the link. The body MAY cite external bytes by digest when size or sensitivity
prevents inline content.

Before using a Witness, a client MUST resolve or obtain the needed manifests
and measurement bytes, recompute their digests, and confirm that the Invariant
requires the presented witness class and binding fields. A repository commit
label, release version, CI status, SBOM name, or artifact URL alone is
insufficient.

### 2.4 Witness limits and correction

- The Witness binds the producer's claimed mapping for one artifact. It does
  not prove the build is reproducible, complete, safe, released, or accepted.
- A signature over `artifact_x` does not prove possession of the artifact.
  Policies MAY require challenge-response, independent rebuild, transparency
  inclusion, or physical-device measurement through separate evidence.
- A producer MUST NOT verify its own Witness. NIP-EV Verification Receipts use
  a different producer key and, where required, a different controlling
  principal.
- Witnesses are append-only. A mistake creates a new Witness with a
  `supersedes` ref; the prior event remains visible.
- A Witness about Invariant revision `n` is stale for revision `n+1` until the
  new revision's required-witness contract explicitly admits it by unchanged
  digest and semantics.

## 3. Regression Watch (`kind:32482`)

A Regression Watch uses two variants of kind `32482`:

- a versioned **definition** that says what, when, and against which exact
  Invariant revisions to check; and
- a dense append-only **event** stream that records lifecycle changes and the
  disposition of each scheduled check slot.

Both variants are Watch-authority-signed. Producer-signed measurement bytes
remain Artifact Provenance Witnesses or NIP-EV Evidence Receipts cited by an
observation event.

### 3.1 Definition address and required tags

```text
32482:<watch_authority_pubkey>:<watch_ref>:def:<revision>
```

Required tags:

- `d`: `<watch_ref>:def:<revision>`
- `record`: `definition`
- `watch`: stable `watch_ref`
- `org`, `revision`
- repeated `e` or `a` refs with marker `invariant`: every exact Invariant
  revision checked
- `target`: target or target family
- `trigger`: `target_revision`, `interval`, or `both`
- `freshness_seconds`: maximum age of an admitted observation before its
  execution freshness expires
- `grace_seconds`: bounded delay after a due slot before the slot is missed
- `loss_after_misses`: positive count of consecutive missed slots that meets
  the loss predicate
- `starts_at`
- `x`: digest of the exact Watch definition document in `content`
- `published_at`

`cadence_seconds` is also required for `trigger=interval` or `both`. Revision
`> 1` requires an `e` or `a` ref with marker `previous`.

### 3.2 Definition document and recommended tags

The `openagents.regression-watch.v1` definition document includes:

- exact target-selection and target-revision discovery rules
- the evaluation procedure and executable or policy digest
- required Witness classes and their producer admission policy
- slot numbering and due-time calculation
- timeout, retry, and duplicate-observation policy
- safe runtime and reproduction limits
- notification and escalation refs, which are routing rules and not authority
- explicit stop reasons and required stop evidence
- privacy and relay audience

Recommended tags include `title`, `cadence_seconds`, `a` with marker `work`,
`a` with marker `workroom`, repeated `p` with marker `runner`, `policy`,
`due_at` for the first slot, and `t` topics.

Revision numbers are dense. Changing target selection, any Invariant revision,
procedure, witness requirement, trigger, cadence, freshness, grace, loss
threshold, stop rule, runner admission, or audience creates a new definition
revision. An authority event `definition_revised` activates it at a named
slot; earlier events keep their former definition revision.

### 3.3 Event address and required tags

```text
32482:<watch_authority_pubkey>:<watch_ref>:evt:<seq>
```

Required tags:

- `d`: `<watch_ref>:evt:<seq>`
- `record`: `event`
- `watch`, `org`
- `seq`: positive dense Watch event sequence
- `event`: a value from 3.4
- `a` with marker `definition`: the active exact definition revision
- `p` with marker `actor`: the principal whose admitted action caused the
  event
- `occurred_at` and `admitted_at`

For `seq > 1`, an `e` or `a` ref with marker `previous` is required. Events
that dispose a scheduled check slot also require `slot`, `due_at`, and
`target_revision`.

### 3.4 Event kinds

- `created`: starts the Watch in `active` state under definition revision `1`
- `definition_revised`: activates the cited next dense definition revision at
  a named first slot
- `check_observed`: admits one runner observation and its Witness refs
- `check_missed`: records that a slot passed its due time plus grace without an
  admitted observation
- `paused`: explicitly suspends creation of new slots
- `resumed`: resumes from `paused` under a named definition and next slot
- `loss_declared`: records that the definition's loss predicate was met
- `recovered`: resumes from `lost` and names the next slot
- `stopped`: explicitly terminates the Watch with reason and residual coverage
- `superseded`: terminates this Watch in favor of a cited successor

Deployments MAY add event kinds. Unknown events remain in sequence and do not
silently change lifecycle state.

### 3.5 Observation tags and results

A `check_observed` event requires:

- `slot`, `due_at`, `started_at`, and `completed_at`
- `target_revision`
- repeated `e` or `a` refs with marker `invariant`: the exact evaluated
  revisions
- repeated `e` or `a` refs with marker `witness`: all required Artifact
  Provenance Witnesses
- `p` with marker `runner`: the executing principal
- `result`: `holds`, `violated`, `inconclusive`, or `not_run`

Recommended tags are `e` or `a` refs with markers `evidence` and
`verification`, `reason`, `env`, `source_x`, `build_x`, `artifact_x`, and
`config_x` copied from the bound Witness set for indexable comparison.

`holds` means the admitted observation found that all named predicates held
for the bound target revision and available Witnesses. It remains an
observation until independently verified where policy requires that rung.
`violated` means at least one named falsifier or failing predicate was
observed. `inconclusive` means the check ran without a dispositive result.
`not_run` records an admitted attempt that produced no evaluation; it does not
make the slot current or successful.

One slot has one terminal scheduling disposition: `check_observed` or
`check_missed`. A late observation after `check_missed` uses a new event with
`late_for_slot`; it does not erase the missed event, restore historical
freshness, or renumber later slots. Duplicate runner results cite the same slot
as additional evidence and do not create a second terminal disposition.

### 3.6 Lifecycle states

Clients fold the dense authority event stream into:

- `active`: slots are expected under the current definition
- `paused`: new interval slots are suspended by explicit authority action
- `lost`: the loss predicate was met and `loss_declared` was recorded
- `stopped`: explicit terminal stop with no successor
- `superseded`: explicit terminal replacement by another Watch

Permitted transitions are:

| From | Event | To |
| --- | --- | --- |
| none | `created` | `active` |
| `active` | `paused` | `paused` |
| `paused` | `resumed` | `active` |
| `active` or `paused` | `loss_declared` | `lost` |
| `lost` | `recovered` | `active` |
| `active`, `paused`, or `lost` | `stopped` | `stopped` |
| `active`, `paused`, or `lost` | `superseded` | `superseded` |

`check_observed`, `check_missed`, and `definition_revised` do not by themselves
change lifecycle state. `stopped` and `superseded` are terminal.

### 3.7 Freshness rules

Freshness describes observation age and target coverage, not whether the
Invariant is true.

- An observation is `fresh` only when it is the latest admitted
  `check_observed` for the current definition, `result` is `holds`,
  `violated`, or `inconclusive`, all required refs resolve, and
  `completed_at + freshness_seconds` has not passed.
- A `not_run` or missing observation is never fresh.
- For `trigger=target_revision` or `both`, discovery of a newer applicable
  target revision makes the prior observation `target_stale` immediately,
  even within the time window.
- Passing `due_at + grace_seconds` without a terminal slot disposition makes
  that slot `overdue`. Clients derive and display overdue status even before
  the authority emits `check_missed`.
- Clock uncertainty, unresolved target discovery, missing Witness bytes, a
  changed Invariant revision, or a definition gap is displayed separately and
  MUST NOT be rounded into fresh.
- Pausing does not freeze a green badge. The latest observation ages normally,
  and the client displays the pause and any stale coverage.

Freshness can therefore change as time or target state changes without a new
event. This derived display fact cannot write, stop, verify, or dispose the
Watch.

### 3.8 Stopping and loss rules

- A Watch stops only through an authority-signed `stopped` or `superseded`
  event. A deadline, elapsed cadence, runner exit, missing relay event,
  repository archival, or satisfied stop condition does not stop it
  automatically.
- `stopped` requires `reason`, `last_slot`, `last_target_revision`, the final
  freshness state, and a bounded residual-coverage statement. It SHOULD cite
  stop evidence and a successor when coverage continues elsewhere.
- Every expected slot is either observed or remains visibly pending, overdue,
  or missed. A stopped Watch does not erase open or missed slots.
- `loss_after_misses` is deterministic over consecutive missed slots. Once the
  threshold is met, clients display `loss_predicate_met` even if the authority
  has not emitted `loss_declared`. The missing declaration is itself a control
  gap.
- `loss_declared` names the first lost slot, affected target revisions, known
  cause, and recovery owner. Loss is not pause or stop; expected revision
  coverage remains outstanding until recovery or explicit termination.
- `recovered` names the next slot and does not rewrite missed history. A
  backfill cites each missed slot and is labeled late.

### 3.9 Sequence integrity

Watch events are dense and append-only. `seq=n` MUST cite `n-1`, and the
active definition revision MUST resolve. A missing or conflicting sequence is
an explicit Watch-history gap. Clients stop canonical folding at the last
unambiguous event and MUST NOT use timestamps or relay order to bridge it.

Definition revisions are independently dense and append-only. Deleting a
definition or event does not renumber the stream. NIP-09 can affect local
display but cannot make a historical observation, miss, loss, or stop never
have happened.

## 4. Result and disposition boundaries

An observed regression is a `check_observed` event with `result=violated`
against an exact target and Invariant revision. It is a signed, admitted
observation. It does not by itself:

- confirm a NIP-FD Candidate Finding or publish its details;
- create a NIP-EV Verification Receipt or Owner Disposition;
- change, weaken, retire, or supersede the Security Invariant;
- open, accept, close, or release Work;
- transition a NIP-PP Product Promise; or
- authorize deployment, rollback, public claim, credit, payment, or
  settlement.

Those systems MAY cite the observation as evidence and act through their own
keys, policies, and records. A fast downward promise transition can cite one
credible failing observation under NIP-PP, but the Watch does not sign that
transition.

## 5. Composition

- **NIP-EV:** Witness measurement bytes can have Evidence Receipts.
  Independent Verification Receipts evaluate them under a named policy;
  Owner Disposition remains separate.
- **NIP-WK:** Work authors Invariants, operates Watches, handles losses, and
  responds to observations. Work state never changes Invariant or Watch state
  without the required SI authority event.
- **NIP-OT, NIP-AD, and Block NIP-OA:** These records declare authority keys,
  runner grants, producer roles, and principal identity used for independence
  checks.
- **NIP-SC and NIP-SP:** Materialized source sets and preregistered scan
  profiles can supply exact source scope and evaluation-policy refs. Coverage
  does not prove an Invariant holds.
- **NIP-FD:** A `violated` observation can motivate a private Finding
  Commitment. Candidate and disclosure details follow NIP-FD and do not enter
  public Watch records during embargo.
- **NIP-PP:** A Watch observation can be evidence for a separate promise
  transition. The promise-registry authority remains the claim authority.

## Security and privacy considerations

- **Digest completeness.** A provenance chain is only as exact as its
  manifests. Omitting generated inputs, defaults, link steps, vendored source,
  dependencies, or post-processing can make different artifacts appear to
  share provenance. Verifiers inspect manifest completeness as well as hashes.
- **Producer self-attestation.** A builder can sign a perfectly formed false
  Witness. Independent verification, reproducible builds, distinct hardware,
  or other AssuranceSpec requirements supply stronger rungs; key distinction
  alone may not establish principal independence.
- **Stale green state.** Clients calculate freshness from exact definition and
  target revisions. They never carry a `holds` result across a new target, a
  changed Invariant, an expired freshness window, or unresolved Witness refs.
- **Missing-check concealment.** Dense sequences and explicit slot accounting
  preserve missed and late checks. Pause, stop, supersession, and recovery do
  not rewrite losses.
- **Policy weakening.** A new Invariant or Watch revision cannot retroactively
  change the meaning of an old observation. Weakening predicates requires an
  explicit revision and recorded reason.
- **Dual-use detail.** Source gaps, symbol names, failing predicates, artifact
  locations, and exact targets can help an attacker. Protected Invariants,
  Witness bodies, and Watch events use restricted NIP-29 audiences, NIP-44
  encrypted content, opaque refs, and restricted relays. Aggregate public
  coverage MUST NOT expose specific unexamined paths.
- **Digest correlation.** Even a digest can reveal that two audiences share a
  private source or artifact. Deployments keep sensitive digests on restricted
  relays and publish aggregate counts or separately authorized public refs.
- **Unsafe checks.** A Watch definition is not execution authority. Runners
  remain constrained by grants, target authorization, resource limits, and
  reproduction boundaries.
- **Relay and scheduler authority.** Relay admission, event expiration,
  retries, scheduler time, or runner status cannot verify a Witness, dispose a
  missed slot, stop a Watch, or create an owner decision.

## References

- NIP-01, NIP-09, NIP-29, NIP-44
- NIP-WK, NIP-EV, NIP-OT, NIP-AD, NIP-PP (this program)
- NIP-FD: Findings, Verdicts, and Disclosure (this program)
- NIP-SC: Source Completeness and Coverage (proposed hardening program)
- NIP-SP: Scan Profiles and Pre-Registration (proposed hardening program)
- Block NIP-OA: agent and principal attestation
- `docs/hardening/2026-08-04-nostr-native-hardening-program.md` — invariant
  families, artifact-witness requirement, roadmap, and falsifiers

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: versioned falsifiable Security Invariants, exact
  source/build/artifact/config Provenance Witnesses, and dense recurring
  Regression Watches with explicit freshness, stopping, and loss rules.
