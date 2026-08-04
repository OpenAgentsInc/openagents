> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: application — security hardening.

NIP-SC
======

Source Completeness and Coverage
--------------------------------

`draft` `optional`

This NIP defines the source-truth and coverage ledger for security assessment:
the **Materialized Source Set** that states what was present and readable, the
**Coverage Attestation** that states what was analyzed and skipped, and the
**Divergence Note** that preserves disagreement between comparable runs.

The governing rule is:

> A result without a resolvable Materialized Source Set is an anecdote. Clients
> MUST label and render it as such; it is not scan-result or coverage evidence.

Completeness is a signed claim with exact counts and reasons. It is not inferred
from a productive-looking report, a repository URL, or the number of findings.

## Signers and authority

- A Materialized Source Set is signed by the principal that materialized and
  inventoried the source for the run.
- A Coverage Attestation is signed by the principal accountable for the scan
  report. The signer can be the same runner, but the record is still a claim,
  not independent verification.
- A Divergence Note is signed by the observer that compared two attestations.
  Any principal can publish one; the signature identifies the observer and
  grants no finding, triage, target-owner, or disclosure authority.
- NIP-EV verification remains separate and requires its own signer and policy.
  Relay acceptance, scan completion, model agreement, and a target's silence
  are not verification or acceptance.
- A relay transports these records. It does not authorize scanning, decide
  completeness, confirm findings, resolve divergence, or control disclosure.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32460 | Addressable (unique `d`) | Materialized Source Set |
| 32461 | Addressable (unique `d`) | Coverage Attestation |
| 32462 | Addressable (unique `d`) | Divergence Note |
| 32463-32469 | — | Reserved for future NIP-SC use |

All three kinds are immutable-by-contract. Corrections and reruns use new
identifiers and explicit predecessor refs.

## Common comparison key

Coverage is comparable only when these four values are equal byte-for-byte:

```text
(target, commit, profile coordinate, profile_x)
```

`target` is a stable repository or target coordinate. `commit` is the full
target commit object id. The profile coordinate and digest identify the exact
NIP-SP measurement instrument. A display name, branch, shortened commit, or
profile version label is not a comparison key.

Every digest in this NIP is lowercase hexadecimal SHA-256 over the exact
unencrypted UTF-8 bytes of the named manifest. An authorized reader decrypts
before checking the digest. Parsers preserve unknown tags and enum values as
unknown and never guess their meaning.

## 1. Materialized Source Set (`kind:32460`)

The exact source population that was present, readable, and eligible for one
run, including known absences. Address:

```text
32460:<materializer_pubkey>:<run_ref>
```

The `d` value MUST be the materializer-scoped `run_ref`. If materialization
changes, it is a different run with a new `run_ref`.

### 1.1 Required tags

- `d`: unique `run_ref`
- `run`: the same `run_ref`
- `target`: stable repository or target coordinate
- `commit`: exact full target commit object id
- `e` with marker `preregistration`: exact NIP-SP Pre-Registration event
- `a` with marker `profile`: exact NIP-SP Scan Profile coordinate
- `e` with marker `profile`: exact Scan Profile event id
- `profile_x`: exact profile digest echoed from the Pre-Registration
- `x`: digest of the exact Materialized Source Set manifest
- `source_state`: `complete`, `partial`, or `degraded`
- `readable_files` and `readable_bytes`: total readable source population
- `analyzed_files` and `analyzed_bytes`: files and bytes supplied to analysis
- `skipped_files` and `skipped_bytes`: readable files and bytes not supplied
  to analysis
- `materialized_at`: claimed completion time of materialization

For every nonzero skipped class, the event MUST include:

```text
["skip_reason", "<code>", "<file_count>", "<byte_count>"]
```

The reason counts MUST sum exactly to `skipped_files` and `skipped_bytes`.
Recommended codes are `profile_excluded`, `unsupported_format`, `binary`,
`generated`, `size_limit`, `budget_exhausted`, `tool_failure`,
`policy_restricted`, `dependency_unavailable`, and `other`.

### 1.2 Recommended tags

- `repo`: each repository announcement address or stable coordinate
- `submodules_declared` and `submodules_populated`
- `vendored_trees_declared` and `vendored_trees_populated`
- `lockfiles_declared` and `lockfiles_resolved`
- `source_reason`: each missing or untrusted source class; recommended codes
  are `submodule_missing`, `submodule_unreadable`, `vendored_missing`,
  `lockfile_missing`, `dependency_unresolved`, `generated_input_missing`,
  `file_unreadable`, `repository_unavailable`, `credential_denied`,
  `materializer_failure`, `manifest_incomplete`, and `other`
- `env_x`: digest of the materializer and harness environment manifest
- `org`, `work`, and `a` with markers `session`, `code_context`, and
  `workroom`, when applicable
- `e` with marker `supersedes`: an explicitly corrected prior source-set
  event; the correction uses a new `run_ref`

### 1.3 Source-state semantics

- `complete`: every source unit required by the pinned profile was present and
  readable, and the manifest inventories every readable file.
- `partial`: one or more required units were absent, unreadable, or excluded
  outside the pinned profile's declared rules, but the materializer can state
  the known boundary and exact counts for what remained.
- `degraded`: a materializer, inventory, dependency, or evidence failure makes
  the boundary or counts unreliable. A degraded set MUST carry at least one
  `source_reason`.

An intentionally excluded file can coexist with `source_state=complete` only
when the exact pinned profile declared that exclusion and the file remains in
the manifest as `skipped` with `profile_excluded`. Undeclared absence is never
complete.

### 1.4 Manifest

`content` MUST carry either the exact source-set manifest as JSON or a NIP-44
encryption of those bytes to the admitted audience. If `content` is empty, an
authorized `url` MUST resolve to bytes matching `x`.

The recommended `schema` is `openagents.materialized-source-set.v1`. The
manifest records:

- each repository, full commit, and tree object;
- every declared submodule with expected commit and
  `populated`, `missing`, or `unreadable` state;
- vendored-tree, lockfile, dependency-closure, and generated-input digests and
  their resolution state;
- every readable file by path or audience-safe opaque path ref, byte count,
  content digest, `analyzed` or `skipped` disposition, and skip reason;
- known missing source units and machine-readable reasons;
- the inventory, materializer, and harness versions or digests needed to
  reproduce the boundary.

The manifest is metadata. It MUST NOT contain raw private repository file
contents, credentials, secrets, terminal output, or undisclosed finding text.

## 2. Coverage Attestation (`kind:32461`)

The durable statement that one target, commit, and profile was examined to a
declared depth over one exact Materialized Source Set. Address:

```text
32461:<attestor_pubkey>:<run_ref>
```

### 2.1 Required tags

- `d`: unique `run_ref`
- `run`: the same `run_ref`
- `target`, `commit`, `a` profile, `e` profile, and `profile_x`: exact values
  copied from the Materialized Source Set and Pre-Registration
- `e` with marker `preregistration`: exact NIP-SP Pre-Registration event
- `e` with marker `source_set`: exact Materialized Source Set event
- `source_x`: the source set's manifest digest, echoed exactly
- `run_state`: `completed`, `aborted`, or `failed`
- `completeness`: `complete`, `partial`, or `degraded`
- `depth`: `inventory_only`, `automated_scan`, `prioritized_review`,
  `manual_review`, `adversarial_validation`, or `mixed`
- `analyzed_files`, `analyzed_bytes`, `skipped_files`, and `skipped_bytes`:
  values copied exactly from the source set
- every `skip_reason` tuple copied exactly from the source set
- `result_x`: digest of the exact result-index manifest, including the empty
  result set
- `completed_at`: claimed end time

When `run_state` is not `completed` or `completeness` is not `complete`, at
least one `reason` tag is required. Recommended codes are
`source_incomplete`, `selection_incomplete`, `budget_exhausted`,
`tool_failure`, `evidence_missing`, `run_aborted`, `policy_stopped`, and
`other`.

### 2.2 Recommended tags

- `finding_set_x`: digest of the lexicographically sorted exact candidate-
  finding or finding-commitment refs; the empty set has an explicit digest
- `e`/`a` with marker `result`: each result record included in `result_x`
- `e`/`a` with marker `evidence`: NIP-EV Evidence Receipts
- `a` with markers `work`, `session`, and `workroom`
- `e` with marker `supersedes`: a prior attestation corrected by a new run
- `reason`: machine-readable qualification, including on a complete run when
  a bounded limitation still matters

### 2.3 Completeness semantics

- `complete`: all profile-required source was materialized, every selected
  file was examined to the declared depth, all skips were profile-declared and
  counted, and every profile-required evidence item exists.
- `partial`: the signer knows and quantifies one or more missing source,
  selection, depth, or evidence classes. Exact counts and reasons remain
  reliable for the material that was examined.
- `degraded`: source truth, execution, tooling, or evidence failed in a way
  that makes the boundary, counts, or claimed depth unreliable.

`complete` is valid only over a `source_state=complete` Materialized Source
Set. `partial` and `degraded` do not mean failure to produce useful evidence;
they prohibit a completeness claim the run did not earn.

The depth values describe work performed; they are not automatically ordered
proof rungs. A manual review is not independent verification, an automated
scan can produce strong executed evidence, and finding truth remains governed
by its evidence and verdict records.

### 2.4 Result composition law

Every event or off-relay report presented as a scan result MUST cite, by exact
event id, a valid `32460` Materialized Source Set and SHOULD cite its `32461`
Coverage Attestation. The target, commit, profile pin, run, and source digest
MUST agree across the chain.

If the source-set event is absent, unresolvable, digest-invalid, equivocated,
or inconsistent, clients MUST:

- label the purported result `anecdote`;
- exclude it from coverage totals and completeness comparisons; and
- avoid completeness, absence-of-findings, or target-safety language.

Anecdotes can remain visible as leads. They do not become scan results because
they have many findings, a trusted signer, or relay acceptance.

## 3. Divergence Note (`kind:32462`)

An observer-signed comparison of two Coverage Attestations with an identical
comparison key. Address:

```text
32462:<observer_pubkey>:div:<comparison_digest>
```

The observer orders the two attestation event ids lexicographically and
computes `comparison_digest` as SHA-256 over the exact comparison-manifest
bytes, which include both ordered ids. The `d` value MUST be
`div:<comparison_digest>`.

### 3.1 Required tags

- `d`: `div:<comparison_digest>`
- `target`, `commit`, `a` profile, and `profile_x`: the shared exact
  comparison key
- two `e` tags with markers `left` and `right`: exact Coverage Attestation
  event ids in lexical order
- `difference`: repeated difference class, using `completeness`,
  `source_set`, `analyzed_count`, `skipped_count`, `skip_reasons`, `depth`,
  `run_state`, `result_set`, `finding_set`, `evidence`, or `other`
- `classification`: `coverage`, `result`, or `mixed`
- `x`: the same `comparison_digest`, binding the exact comparison manifest
- `observed_at`: claimed observation time

### 3.2 Recommended tags and content

- `reason`: bounded machine-readable explanation for each material difference
- `e`/`a` with marker `evidence`: comparison evidence
- `e` with marker `supersedes`: a prior erroneous note; a corrected comparison
  has different bytes and therefore a different `d`
- `a` with markers `work` and `workroom`

`content` is the exact JSON comparison manifest or a NIP-44 encryption of it.
The recommended `schema` is `openagents.coverage-divergence.v1`. It includes
the two attestation ids, their `source_x`, completeness, counts, skip reasons,
depth, `result_x`, optional `finding_set_x`, and a bounded statement of each
difference. It MUST NOT copy raw source, private findings, or embargoed
evidence.

A Divergence Note is a lead. It is not a Candidate Finding, Finding Verdict,
verification, target-owner disposition, or disclosure decision. Equal finding
counts do not imply equal finding sets; clients compare `finding_set_x` or the
exact result refs when available.

## 4. Ordering and idempotency

1. **Pre-registration first.** A source set or attestation is valid only when
   its exact NIP-SP Pre-Registration resolves, was unexpired when the run
   started, and has identical target, commit, profile event, `profile_x`, and
   rubric pin.
2. **Source truth before result.** The Materialized Source Set MUST be finalized
   before the Coverage Attestation or any scan result that cites it. Later
   discovery of omitted source creates a new run; it does not rewrite the old
   boundary.
3. **Counts reconcile.** `readable_files = analyzed_files + skipped_files` and
   `readable_bytes = analyzed_bytes + skipped_bytes`. Skip-reason totals match
   skipped totals. Attestation counts and tuples equal the source set exactly.
   A mismatch makes the attestation invalid, not complete with a warning.
4. **Exact references order the graph.** Event ids and predecessor refs define
   derivation. `created_at`, claimed timestamps, and relay arrival order do not
   select a replacement or prove execution order by themselves.
5. **Idempotent replay.** Identical event ids are de-duplicated. Different
   event ids with the same signer, kind, and `d` are equivocation; clients
   surface all versions and exclude the ambiguous record from completeness
   aggregation until explicitly resolved.
6. **Divergence is stable.** Recomputing the same comparison manifest produces
   the same Divergence Note address for one observer. Changing either input or
   the comparison bytes creates a new address and retains the prior note.

## 5. Composition

- **NIP-SP.** The Pre-Registration supplies the frozen target, commit, profile,
  hypothesis, rubric, and expiry. These records echo the exact pins.
- **Work and code.** NIP-WK, NIP-CC, and NIP-RC can connect a run to admitted
  Work, code context, sessions, and repository claims. Those records do not
  make coverage complete or authorize access.
- **Evidence.** NIP-EV Evidence Receipts can bind manifests and reports.
  Independent Verification Receipts evaluate them under a named policy. A
  runner-signed attestation cannot verify itself.
- **Findings and disclosure.** Candidate findings, verdicts, commitments, and
  disclosure keep their own state and audience. A finding set can be compared
  here by digest without publishing its contents.
- **Projection.** Public coverage projections SHOULD aggregate counts by
  ecosystem, class, and period. They MUST preserve partial/degraded labels and
  MUST NOT turn absence of a public gap into evidence of complete coverage.

## Security and privacy considerations

- **False completeness.** A clone, repository ref, clean harness exit, or
  nonempty finding list does not prove source completeness. Clients verify the
  manifest digest, exact pins, counts, reasons, and source-state rules.
- **Dual-use gap disclosure.** Exact skipped paths, unexamined security-critical
  areas, and finding-set differences can guide attackers. Specific gaps belong
  in the applicable NIP-29 workroom with NIP-44 encryption and restricted
  relays. Public views use aggregate counts. Only the target owner can opt the
  target into broader gap disclosure; another observer cannot lower that
  boundary.
- **No raw private source.** Public events MUST NOT contain raw private
  repository contents, private file paths, source excerpts, prompts containing
  source, credentials, terminal output, or embargoed findings. Use opaque refs,
  digests, restricted relays, and encrypted manifests. Even file names and
  per-class counts can be sensitive and require audience review.
- **Digest probing.** Digests of small or predictable private values can be
  guessed. Private manifests SHOULD use audience-restricted storage and MAY
  use committed, domain-separated salted digests where equality outside that
  audience is not required. The salt stays inside encrypted content.
- **Scanner and relay overclaim.** A scanner signature proves its claim; it
  does not make the scanner independent, correct, or authorized. A relay has no
  scan, finding, verification, target-owner, disclosure, release, or settlement
  authority.
- **Divergence abuse.** A note can be mistaken, malicious, or based on private
  information. Clients show the observer, exact inputs, difference classes,
  and audience; they never render divergence itself as a vulnerability.
- **Result laundering.** Referencing an invalid, unavailable, or unrelated
  source set does not satisfy the result-composition law. The complete chain
  must resolve and match.

## References

- NIP-01, NIP-29, NIP-34, NIP-44
- NIP-SP: Scan Profiles and Pre-Registration (this program)
- NIP-OT, NIP-WK, NIP-EV, NIP-CC, NIP-RC (this program)
- [`The Bitcoin OSS hardening program as a Nostr-native public project`](../hardening/2026-08-04-nostr-native-hardening-program.md)
- [`OpenAgents glossary`](../omega/GLOSSARY.md) — Materialized Source Set,
  Evidence Completeness, Coverage Ledger, Divergence Signal

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: exact materialized-source manifests, analyzed and
  skipped accounting, complete / partial / degraded attestations, anecdote
  handling, and target + commit + profile divergence notes.
