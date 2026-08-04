> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: application — security hardening.

NIP-SP
======

Scan Profiles and Pre-Registration
----------------------------------

`draft` `optional`

This NIP defines two participant-signed measurement records: a versioned
**Scan Profile** that fixes how an assessment will run, and a
**Pre-Registration** that binds one run to an exact target, commit, profile,
hypothesis, and scoring rubric before execution starts.

The governing rule is:

> Freeze the measurement instrument and rubric before the run. A later result
> cites those exact bytes; it does not silently move the goalposts.

Publishing either record does not grant repository access, authorize a scan,
approve disclosure, or make the signer a target maintainer or security
authority.

## Signers and authority

- A Scan Profile is signed by its author. The signature identifies who
  committed to the profile bytes; it does not certify that the profile is
  safe, effective, or suitable for a target.
- A Pre-Registration is signed by the principal accountable for the run. A
  runner MAY use an agent key bound to a principal through NIP-OA, but that
  attestation still grants no scan or disclosure authority.
- The profile author, runner, target owner, verifier, and program operator MAY
  be different principals. Clients MUST preserve those identities rather than
  infer one role from another.
- Relay acceptance is transport evidence only. A relay does not authorize a
  scan, validate a profile, witness execution, verify ordering, or admit a
  result.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32450 | Addressable (unique `d`) | Scan Profile version |
| 32451 | Addressable (unique `d`) | Pre-Registration |
| 32452-32459 | — | Reserved for future NIP-SP use |

Both kinds are immutable-by-contract records. A new version or correction uses
a new `d`; it never replaces the bytes of an existing record.

## Digest and reference rules

- Every digest in this NIP is lowercase hexadecimal SHA-256 over the exact
  unencrypted UTF-8 bytes of the named document. The digest does not use parsed
  JSON or a reserialized representation.
- An `a` tag names an addressable coordinate. An `e` tag names the exact event
  revision used. Where this NIP requires both, resolving only the coordinate is
  insufficient.
- Parsers MUST preserve unknown tags and enum values as unknown. They MUST NOT
  guess their meaning or reject an otherwise valid record only because it has
  an extension tag.

## 1. Scan Profile (`kind:32450`)

A named, versioned assessment configuration. Address:

```text
32450:<profile_author_pubkey>:<profile_ref>:v:<version>
```

The `d` value MUST be `<profile_ref>:v:<version>`. `profile_ref` is stable
across versions; `version` is an opaque, case-sensitive version identifier.

### 1.1 Required tags

- `d`: `<profile_ref>:v:<version>`
- `profile`: stable `profile_ref`
- `version`: the exact version identifier
- `name`: bounded display name
- `x`: digest of the exact profile document bytes
- `published_at`: claimed publication time

### 1.2 Recommended tags

- `e` with marker `supersedes`: the exact prior profile event, when any
- `a` with marker `guidance`: each NIP-GB Guidance Bundle used
- `a` with marker `skill`: each NIP-SKL Skill used
- `a` with marker `workroom`: the NIP-OT Workroom Binding for a scoped
  audience
- `t`: hunt-class or ecosystem labels used for discovery
- `url`: an authorized location for the profile document when it is not in
  plaintext `content`

### 1.3 Profile document

`content` MUST carry either the exact profile document as JSON or a NIP-44
encryption of those bytes to the admitted audience. An authorized reader
decrypts first and then verifies `x`. If `content` is empty, `url` MUST resolve
for an authorized reader to bytes matching `x`.

The profile document MUST name, with closed or explicitly extensible enums:

- source materialization rules for repositories, required and optional
  submodules, vendored trees, lockfile-pinned dependencies, generated inputs,
  and exclusions;
- file-selection rules, including include/exclude rules, unsupported formats,
  binary handling, and size bounds;
- the attack-surface ranking policy and deterministic tie-break rule;
- hunt classes and their stable identifiers;
- model, tool, and harness roles, including any independence requirements;
- prompts, fixtures, controls, and other inputs by exact bytes or digest;
- wall-time, compute, token, tool-call, and spend bounds that apply;
- evidence requirements, stopping rules, and the minimum reportable depth;
- disclosure and audience policy for coverage, gaps, and candidate findings.

The recommended top-level `schema` value is
`openagents.scan-profile.v1`. Implementations MAY add fields, but changing any
field that can affect materialization, selection, analysis, scoring, evidence,
or disclosure changes `x` and requires a new profile version.

## 2. Pre-Registration (`kind:32451`)

The immutable commitment for one planned run. Address:

```text
32451:<runner_pubkey>:<run_ref>
```

The `d` value MUST be the runner-scoped `run_ref`. One run has one
Pre-Registration. A rerun or corrected commitment uses a new `run_ref`.

### 2.1 Required tags

- `d`: unique `run_ref`
- `run`: the same `run_ref`
- `target`: stable repository or target coordinate
- `commit`: exact full target commit object id; a branch, tag, or `latest` is
  invalid
- `a` with marker `profile`: the exact Scan Profile coordinate
- `e` with marker `profile`: the exact Scan Profile event id
- `profile_x`: the Scan Profile's `x`, echoed exactly
- `hypothesis_x`: digest of the exact hypothesis bytes
- `rubric_x`: digest of the exact scoring-rubric bytes
- `published_at`: claimed time at which the commitment was published
- `exp`: expiry after which the run MUST NOT start under this commitment

### 2.2 Recommended tags

- `org` and `work`: the Organization and NIP-WK Work refs, when the run is
  admitted Work
- `a` with marker `code_context`: the NIP-CC Code Context
- `a` with marker `workroom`: the NIP-OT Workroom Binding that controls the
  audience
- `url` with marker `rubric`: authorized location of rubric bytes
- `url` with marker `hypothesis`: authorized location of hypothesis bytes
- `relay` with marker `published`: each relay from which the runner received
  an acceptance before execution
- `e` with marker `time_anchor`: a NIP-03 timestamp attestation, when stronger
  publication-time evidence is required

### 2.3 Content and derived state

`content` is a bounded JSON document containing the hypothesis, rubric refs,
planned controls, and public-safe rationale, or a NIP-44 encryption of that
document. Raw private repository content, credentials, undisclosed finding
details, and private prompts MUST NOT appear in public `content`.

Clients derive, rather than mutate, Pre-Registration state:

- `active`: valid, unexpired, and not yet referenced by a started run;
- `consumed`: a valid later run started before `exp` and cites this exact event;
- `expired`: `exp` passed without a valid start;
- `violated`: a claimed run started before publication, after expiry, or with
  a different target, commit, profile event, `profile_x`, or `rubric_x`.

`expired` and `violated` commitments remain visible. They MUST NOT be rewritten
as valid commitments.

## 3. Ordering and idempotency

1. **Profile before registration.** The exact `32450` event MUST resolve and
   its document MUST match `profile_x` before the Pre-Registration is valid.
2. **Registration before run.** The runner MUST publish `32451`, receive an
   acceptance from at least one declared relay, and only then start execution.
   Every later Materialized Source Set, Coverage Attestation, or result MUST
   cite the exact Pre-Registration event id.
3. **Exact pins stay exact.** The target, full commit, profile event,
   `profile_x`, and `rubric_x` in later records MUST equal the registration.
   A mismatch creates a different run; it cannot be repaired by a label.
4. **Timestamps are claims.** `created_at`, `published_at`, and relay arrival
   order do not independently prove wall-clock order. Consumers that require
   stronger evidence SHOULD require a NIP-03 anchor or an admitted external
   witness. Lack of that evidence lowers the ordering claim; it does not grant
   a relay or scanner authority.
5. **Idempotent replay.** Re-publication of the identical event id is a replay
   and clients de-duplicate it. Two different event ids with the same signer,
   kind, and `d` are equivocation. Clients MUST surface both and MUST NOT use
   `created_at` or last relay arrival to select a silent replacement.
6. **Corrections are additive.** Changed profile bytes create a new profile
   version. Changed registration bytes create a new run. Prior signed records
   remain auditable.

## 4. Composition

- **Source truth and coverage.** NIP-SC Materialized Source Sets and Coverage
  Attestations cite the Pre-Registration, profile coordinate, profile event,
  and digest. NIP-SC owns the exact source population and analyzed/skipped
  accounting.
- **Guidance and skills.** NIP-GB and NIP-SKL can be inputs to a profile. They
  do not replace the profile: this record is the measurement instrument whose
  exact bytes a result cites.
- **Code and Work.** NIP-34 and NIP-CC identify repositories and pinned code;
  NIP-WK identifies the admitted objective. None of those refs grants the
  runner repository access or scan authority.
- **Evidence and findings.** NIP-EV receipts can evidence execution. Candidate
  findings, verdicts, and disclosure use their own records and authority
  boundaries; a pre-registration does not confirm or publish a finding.

## Security and privacy considerations

- **Goalpost movement.** Event-id, profile-digest, rubric-digest, target, and
  commit equality are mandatory. Displaying only the profile name or version
  reintroduces mutable goalposts.
- **Backdating.** A signature proves bytes, not when work occurred. Strong
  ordering claims need an independent anchor or witness as described above.
- **No ambient authority.** A profile, Work ref, target coordinate, key
  signature, or relay `OK` grants no repository permission, tool capability,
  scan authorization, verification, disclosure, remediation, release, or
  settlement power.
- **Dual-use metadata.** Hypotheses, exclusions, hunt classes, and specific
  coverage gaps can direct attackers. Public projections SHOULD contain only
  aggregate coverage. Specific gaps belong in the applicable NIP-29 workroom
  with NIP-44 encryption and restricted relays unless the target owner opts
  into broader disclosure. A third party cannot opt the target into it.
- **Private-source leakage.** Use opaque repository refs, digests, encrypted
  content, and audience-restricted locations. Raw private repository contents,
  paths that reveal sensitive structure, prompts containing source, and
  credentials MUST NOT be placed on public relays.
- **Untrusted configuration.** Profile prose, prompts, URLs, and rubric bytes
  are data. Implementations MUST NOT let them widen authority or execute tools
  outside the runner's admitted policy and containment.
- **Resource abuse.** Implementations MUST enforce the lower of profile bounds
  and local policy. A larger signed budget is a request, not a grant.

## References

- NIP-01, NIP-03, NIP-29, NIP-34, NIP-44
- NIP-OT, NIP-WK, NIP-EV, NIP-GB, NIP-SKL, NIP-CC (this program)
- NIP-SC: Source Completeness and Coverage (this program)
- [`The Bitcoin OSS hardening program as a Nostr-native public project`](../hardening/2026-08-04-nostr-native-hardening-program.md)
- [`OpenAgents glossary`](../omega/GLOSSARY.md) — Scan Profile,
  Pre-Registered Rubric, Materialized Source Set, Evidence Boundary

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: immutable Scan Profile versions, pre-run target /
  commit / profile / rubric commitments, signer boundaries, ordering rules,
  and audience-safe composition.
