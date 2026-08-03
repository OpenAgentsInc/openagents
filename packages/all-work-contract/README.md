# OpenAgents All Work contract

`@openagentsinc/all-work-contract` owns the encoded boundary for All Work reads,
the OpenAgents-owned planning authority, and the shared Work command admission
authority. It does not replace native Effect and Rust domain models.

The reviewed source is
[`definition/all-work-v1.contract.json`](./definition/all-work-v1.contract.json).
It uses the restricted OpenAgents Contract Profile described in
[`docs/sol/2026-08-02-effect-rust-unified-contract-models-analysis.md`](../../docs/sol/2026-08-02-effect-rust-unified-contract-models-analysis.md).
The generator emits these committed artifacts:

- Effect Schema codecs and TypeScript types in `src/generated.ts`;
- Rust `serde` types and structural validators in
  `generated/rust/all_work_v1.rs`;
- canonical JSON Schema in
  `generated/json-schema/all-work-v1.schema.json`;
- positive, negative, absent/null, integer, unknown-field, and compatibility
  fixtures in `fixtures/`; and
- artifact digests, protocol methods, compatibility posture, and named
  handwritten semantic checks in `generated/compatibility.json`. Its
  per-type `implementationStatus` distinguishes live request-processor types
  from structural-only future SDK shapes.

Generated code owns structure only. `src/semantic.ts` keeps cross-record rules
explicit: an Issue projection uses the same Work identity and revision, v1 is
an explicit rollback negotiation, Work reads require `omega-effectd.v2`, and
successive projections from one source cannot regress revision or change a
cursor without a revision advance. The boundary also defines typed Work-index
subscription request/event envelopes without claiming a production transport.
Neither layer grants admission, delegation, verification, acceptance, release,
settlement, or public-claim authority.

## Planning authority

`PlanningGraph` is the generated cross-runtime read model. It carries typed
planning resources, same-identity Work/Issue snapshots, planning and label
links, public-safe text records, Release Scope Links, Source Coordinates,
Projection Issues, freshness, completeness, a revision, an event cursor, and a
reconciliation digest. Release Planning Record resources are planning metadata.
They are never canonical Releases, Release Candidates, approvals, or
publication authority.

The Effect-owned implementation is handwritten beside the generated shape:

- `src/planning-authority.ts` owns native create, update, triage, relation,
  comment, and planning commands. It requires optimistic revision and an
  idempotency key, records a zero-GitHub-write receipt, and refuses native
  mutation of imported read-only Work.
- `src/planning-file-store.ts` validates the versioned state and atomically
  replaces the owner-local JSON record. A multi-process host supplies its normal
  single-writer lease; stale revisions fail closed.
- `src/github-bootstrap.ts` reconciles bounded, public-safe GitHub pages into
  stable Work identities. GitHub supplies source observations only. Missing
  pages become explicit gaps; a complete later observation can mark a retained
  last-known-good row unavailable without deleting its identity.
- `bootstrap/v0.2.0-github-source.json` is the digestible final bootstrap
  corpus: 28 open rows, six closed foundation rows, 42 planning resources, and
  46 typed relations from the accepted dogfood snapshot. Importing the same
  corpus again is a no-op.

The command processor does not write to GitHub. A successful GitHub read grants
no command, claim, delegation, verification, owner-disposition, release, or
public-claim authority.

## Repository Work Claim authority

The generated boundary also defines Work Packet, Repository Work Claim, audit,
read, and execute records. `src/repository-claim-authority.ts` owns their
Effect command processor, optimistic revision, idempotency, collision checks,
90-minute-plus-audit takeover law, generation fencing, historical GitHub
projection, and atomic owner-local state. `repository.claim.read` and
`repository.claim.execute` are live v2 reference-process methods. Native claim
receipts always report zero GitHub writes.

## Signed Workroom projections

The generated boundary includes causal signed Workroom activity, audience and
privacy labels, supersession/revocation targets, and a persist-before-publish
outbox. `src/signed-workroom-nostr.ts` recomputes the deterministic NIP-01 event
ID and verifies its BIP-340 Schnorr signature. Direct projection admission
requires `principal:nostr:<signer-pubkey>`; other actor classes require a future
purpose-bound grant adapter and fail closed now.

`src/signed-workroom-authority.ts` admits the verified projection only for the
named Effective Principal and capability, known causal parents, and an
advancing generation. Its receipt explicitly says that relay acceptance is not
authority and that the projection does not admit an external effect. See
`docs/omega/2026-08-03-signed-workroom-projection.md`.

The `workroom.activity.deliver` method records unique attempts only for the
relay targets already persisted with the signed event. Accepted, rejected, and
unreachable facts advance the same optimistic ledger revision. Partial success
remains visible through the exact accepted-relay set, failed targets remain
retryable, and a delivery receipt still admits no external effect or authority.

Canonical JSON uses `openagents-canonical-json-v1`: UTF-8, object keys sorted
by Unicode code point, array order retained, safe integers only, absent fields
omitted, present null encoded as `null`, and no insignificant whitespace. The
Effect and Rust conformance tests compare the same committed byte vector.
The Effect export `encodeAllWorkCanonicalJson` and Rust export
`canonical_json_bytes` enforce that encoding; the Effect encoder also enforces
the boundary byte limit.

## Work command admission

The generated boundary defines `work.command.execute` for assignment,
delegation, revocation, agent-session activity and control, artifacts,
verification, and Owner Disposition. The handwritten
`src/work-command-authority.ts` processor requires an explicit Organization,
Effective Principal, capability, expected revision, generation, and
idempotency key. It requires a human Assignee before delegation, keeps that
assignment separate from the Agent Delegate, and refuses to unassign the
accountable human while a grant remains active. Revocation must clear the
delegate and fence its Session generation before unassignment can proceed. The
processor retains provider-event and loss refs and returns exact
zero-GitHub-write receipts. Repository claim and lease refs are a conditional
pair; collision and takeover authority remains with the native Repository Work
Claim service. See the
[authority record](../../docs/omega/2026-08-03-work-command-admission-authority.md).
`src/work-command-file-store.ts` persists one atomic, private state record per
digest-addressed canonical Work identity; the host remains responsible for its
single-writer lease.

## Internal Work writer cutover

`work.cutover.read` and `work.cutover.execute` expose the canonical,
generation-fenced writer ledger. It starts in `legacy_github` shadow mode and
cannot activate from import, startup, tests, or a rendered screen. Activation
requires the exact reconciled source digest and cursor, an authorized Effective
Principal, the dedicated capability, optimistic revision and generation, an
explicit receipt reference, and a generated request whose GitHub-write count is
structurally zero.

While `native_omega` is active, every native event must advance the retained
high-water cursor. Rollback refuses unless its reconciliation cursor exactly
covers that high-water mark. The Effect authority owns idempotency and atomic
owner-local persistence. This landed boundary does not activate the cutover;
the packaged two-client journey remains a separate gate.

`src/internal-github-write-policy.ts` is the shared refusal boundary for
retained internal GitHub issue and claim writers. Owner-local processes resolve
the durable ledger through `OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT`; remote or
hosted writers receive the same state through
`OPENAGENTS_INTERNAL_WORK_WRITER`. Missing signals retain preactivation
`legacy_github` behavior, malformed or conflicting signals fail closed, and
`native_omega` routes canonical Work to Omega before a GitHub mutation seam can
run. Strict bug ingress and exact completion/release callback comments remain
transport boundaries, not Work or claim authority. Landing this fence does not
activate `native_omega`.

## Strict public bug candidates

`strict_bug.candidate.read` and `strict_bug.candidate.execute` provide the
typed ingress and triage boundary for the final public GitHub strict-bug form.
A preverified GitHub transport adapter must supply the exact issue source,
delivery identity, and signature-verification evidence. The authority accepts
only issues in `OpenAgentsInc/openagents` or `OpenAgentsInc/omega`, rejects
secret-shaped or private-path content, deduplicates source and delivery refs,
derives the ingress idempotency key from the delivery ref, and persists every
report as an untrusted `pending` candidate.

Admission is a separate owner-local triage command. A non-rejected disposition
must link the candidate to canonical Work, but that link does not grant Work
command authority. Both ingress and triage return idempotent receipts whose
GitHub-write count is structurally zero. This boundary does not install or
claim a production GitHub webhook adapter; it is the canonical destination
that such a signature-verifying adapter must call.

## Generate and verify

```bash
pnpm --dir packages/all-work-contract generate
pnpm --dir packages/all-work-contract check:generated
pnpm --dir packages/all-work-contract typecheck
pnpm --dir packages/all-work-contract test
cargo test -p openagents-all-work-contract
```

`check:generated` runs offline, regenerates into a temporary directory, checks
the complete generated-file inventory, and byte-compares every artifact. A
wire change starts in the definition and produces a reviewable generated diff.

## Cross-repository consumption

Omega consumes the generated Rust file, fixtures, and compatibility manifest
as one digest-bound artifact. Omega-specific adapters map native state to these
DTOs. A copied or vendored generated file is valid only when its definition and
artifact digests match the manifest; handwritten Rust mirrors are not allowed.

The first immutable consumer is Omega commit
`6e3f67c6006b0e98eb57047971777eece2fd0f20`. It pins OpenAgents commit
`1ea08b1429cbd888875fef195f9b94bef666e70e` and Rust artifact SHA-256
`298aa826cb7bdf182742251d53c9ab6a436ba8e386fd292a22701a7dec40cefb` in
`crates/omega_effectd/all-work-contract/SOURCE.json`. Omega's Rust supervisor
also has an opt-in cross-repository test that starts the pinned TypeScript
process and negotiates `omega-effectd.v2` before it decodes the typed Work
Index response.

The additive `planning.graph.read` method is the generated seam for the
canonical planning projection. The OpenAgents `omega-effectd` reference process
opens the durable Effect authority and serves this method after v2 negotiation.
An Omega consumer must pin the definition and Rust artifact digests together.
It must not decode the development fixture as live authority.
