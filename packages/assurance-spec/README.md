# `@openagentsinc/assurance-spec`

The repository-installable agent skills live in [`skills/`](./skills/):
`assurancespec-authoring` creates and refines deterministic proposals, while
`assurancespec-work` executes reviewed obligations without acquiring admission,
verification, completion, or release authority. npm-based installation remains
gated on the starter-kit/publication milestone. The files are published here
now for direct repository consumption.

Deterministic proposal, parsing, structural validation, serialization, and
adequacy assessment for the proposed AssuranceSpec `0.1` companion format.

This first implementation turns an executable ProductSpec into exact assurance
coverage scaffolding: exact ProductSpec identity, one proposed obligation per
criterion, exact source-claim snapshots, and explicit `needs_design`
diagnostics for every unresolved proof-design field. Optional repository
inventory is pinned to committed Git `HEAD`.

Current limitation: `propose`, semantic-planner preparation, and
`session begin` use the legacy executable-profile extractor, which recognizes
stable top-level Markdown criterion bullets. They do not yet consume the validated
upstream-style `productspec-acceptance-criteria` item list even though
`@openagentsinc/product-spec` parses and validates that list. Do not rewrite or
downgrade a structured ProductSpec to work around this. The IDE reconciliation
records the bounded mechanical bridge for Desktop revision 7 in that proposed
AssuranceSpec and `docs/assurance/README.md`. Direct structured-item support is
a tooling follow-up, not an assurance or admission shortcut.

Observer's semantic-planning boundary is also available as an injected Effect
program. The caller supplies an explicit accepted ProductSpec identity pin.
the request builder checks its path, revision, document digest, and ordered
criterion ids against the exact Markdown. A provider-neutral planner returns a
typed disposition for every criterion. Deterministic compilation rejects
missing, duplicate, stale, drifted, malformed, self-verifying, or label-only
seam designs, and copies claim snapshots/digests only from the checked request.
The result is always `proposed`: review annotation and admission are separate.

It does **not** use a model, infer semantics from filenames, map tests to
criteria, choose tools or environments, run tests, admit proof design, create
evidence, verify results, authorize release, or change public promises.
Repository test-looking paths and package scripts are unbound proposal context,
not proof.

## CLI

```bash
node --import tsx packages/assurance-spec/src/cli.ts propose \
  docs/mvp/openagents-codex-workroom-mvp.product-spec.md \
  --repo . \
  --out docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md \
  --force

node --import tsx packages/assurance-spec/src/cli.ts validate \
  docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md

node --import tsx packages/assurance-spec/src/cli.ts coverage \
  docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md

# Provider-free boundary smoke. accepted-subject.json is an explicit identity
# pin, not an identity derived or accepted by this command.
node --import tsx packages/assurance-spec/src/cli.ts observer propose \
  docs/mvp/openagents-codex-workroom-mvp.product-spec.md \
  --accepted-subject accepted-subject.json \
  --planner fixture \
  --out /tmp/observer-proposal.assurance-spec.md
```

`propose` succeeds when it creates a structurally valid proposal even if every
obligation still needs design. `coverage` reports adequacy separately.
`observer propose --planner fixture` exercises the same injected planner
request/response compiler used by agents and intentionally leaves every
criterion `needs_design`. It grants no review, admission, or execution
authority. Library callers inject their provider implementation through
`runSemanticPlannerProposal`. Model calls do not occur inside parsing or
compilation.

For a ProductSpec whose Acceptance Criteria section uses only a
`productspec-acceptance-criteria` block, the current `propose` command fails
closed with `product_spec_not_executable` / `missing_acceptance_criteria`.
Structural ProductSpec validity does not waive that limitation, and callers
must not interpret it as permission to invent IDs, alter exact subject bytes,
or admit a hand-built proposal.

### Agent tooling (AT-1)

The CLI also ships the deterministic agent surface designed in
[`../../docs/assurance/AGENT_TOOLING.md`](../../docs/assurance/AGENT_TOOLING.md):

```bash
# Stateless dual-digest sessions (the full pin is returned to the caller)
node --import tsx packages/assurance-spec/src/cli.ts session begin <file.assurance-spec.md> [--root <dir>] [--json]
node --import tsx packages/assurance-spec/src/cli.ts session check <file.assurance-spec.md> \
  (--against <session.json> | --spec-digest <hex> --subject-digest <hex>) [--root <dir>] [--json]

# Read-only reports (never a verdict, never a blended score)
node --import tsx packages/assurance-spec/src/cli.ts obligations <file> [--criterion <id>] [--status ready|needs_design] [--technique <t>] [--json]
node --import tsx packages/assurance-spec/src/cli.ts obligation <file> <obligation-id> [--json]
node --import tsx packages/assurance-spec/src/cli.ts ledgers <file> [--json]
node --import tsx packages/assurance-spec/src/cli.ts checklist <file> [--criterion <id>] [--json]
node --import tsx packages/assurance-spec/src/cli.ts claim <file> [--claim "<text>"] [--json]
node --import tsx packages/assurance-spec/src/cli.ts inventory <repo-dir> [--out <file.json>] [--json]

# Agent Run 0.1 interop: cross-checked, but only self-reported evidence
node --import tsx packages/assurance-spec/src/cli.ts agent-run ingest <file.agent-run.json> [--root <dir>] [--json]

# Read-only stdio MCP server (JSON-RPC 2.0, protocol 2024-11-05), confined to --root
node --import tsx packages/assurance-spec/src/cli.ts mcp --root .
```

Exit codes are the API: **0** success, **1** operation failure, **2** usage
error, **3** stale session. Every command takes `--json`.

The MCP server exposes the §3.1 read-only tool table
(`ingest_agent_run`, `begin_assurance_session`, `check_assurance_session`, `list_assurance_specs`,
`get_assurance_spec`, `validate_assurance_spec`, `get_subject_binding`,
`get_obligations`, `get_obligation`, `get_seams`, `get_environments`,
`get_gates`, `get_coverage_ledgers`, `get_evidence_checklist`,
`check_completion_claim`, `get_typed_gaps`, `get_repository_inventory`) over a
hand-rolled zero-dependency stdio JSON-RPC loop. There are deliberately no
mutating tools: no admit, approve, verify, plan, design, or propose over MCP.
Until receipts exist, `observation` is `not_run` everywhere, the reachable
frontier is `not_computed`, and missing Environment Profiles are typed gaps
(`environment_profile_missing`) — the tools say exactly that instead of
rounding up. Sessions are stateless: `begin` returns the full dual-digest pin
(no `intent_digest` yet — the field is declared, never faked) and `check`
recomputes both digests and classifies `unchanged` / `assurance_spec_changed`
/ `subject_changed` / `both_changed` / `invalid_current` with a typed
`recommended_action`.

Agent Run ingest is deliberately weaker than receipt ingestion. It validates
the upstream 0.1 shape, rejects duplicate item IDs, confines and validates the
pinned ProductSpec, checks its revision and every cited `AC-*` / `EVAL-*` /
`SM-*` ID, and checks the byte digest when `content_hash` is present. A missing
optional hash is returned as `missing_product_spec_content_hash`. The output is
always `proof_rung: "self_report"`, explicitly records `producer == claimant`,
preserves item results as `claimed_items`, and grants no observation,
verification, or independent-producer authority.

Without `--repo`, proposal generation remains valid and emits a typed
`repository_not_supplied` diagnostic. With `--repo`, inventory reads the
committed tree and tracked dirty state only. It does not read ignored or
untracked files, run package scripts, inspect remotes, or serialize absolute
paths.

## Document format (AS-L1 completion, #8760)

- **Custom sections round-trip.** A `## custom-<kebab-name>` heading (the id
  itself is the heading in this bounded profile) is preserved byte-stable
  after the nine mandatory sections instead of failing `unsupported_section`.
  Malformed custom ids fail `invalid_custom_section_id`. Custom sections
  before a mandatory section fail `invalid_section_order`. Documents without
  custom sections serialize exactly as before.
- **Unknown frontmatter round-trips.** Flat `key: value` lines outside the
  bounded profile are preserved verbatim, in authored order, after the known
  keys. They are never interpreted.
- **Thin/empty-section warnings.** A mandatory section whose narrative
  (structured block excluded) is empty warns `empty_required_section`. Fewer
  than `THIN_SECTION_WORD_COUNT` meaningful words warns
  `thin_required_section`. Warnings never affect validity — they are cheap
  honesty about skeleton documents, and the deterministically generated MVP
  proposal rightly warns on every section until a human writes real reasoning.
- **Referential integrity at parse time.** Duplicate/dangling/uncovered
  criterion, environment, and gate references are computed in the same parse
  pass. `parseAssuranceSpec` throws the first violation and
  `validateAssuranceSpec` reports the complete set.

## Conformance corpus (`conformance/`)

`conformance/valid/` holds canonical documents (seeded from the checked-in MVP
proposal plus minimal fixtures) that must validate and round-trip byte-stable.
`conformance/invalid/` holds one fixture per implemented stable error code,
named `<code-kebab>[--variant].assurance-spec.md`. The sweep in
`test/conformance.test.ts` derives the expected code from the filename and
mechanically enforces that every registered code
(`ASSURANCE_STRUCTURAL_ERROR_CODES`) has a fixture and every fixture names a
registered code. `conformance/review/` does the same for review annotations.

Codes are API. Fixtures are frozen bytes — never regenerate them to make a
serializer change pass. Any change that can make a previously valid document
invalid must bump `assurance_spec_format_version`, freeze the current corpus
under a per-version directory, and seed the new version's corpus
(ASSURANCE_SPEC.md §13, the version pin is asserted in the parity tests).

## Review annotations (`.assurance-review.json`)

Portable review annotations (ASSURANCE_SPEC.md §8.1) are format-only in AS-1:
`validateAssuranceReviewAnnotation` / `parseAssuranceReviewAnnotation` /
`serializeAssuranceReviewAnnotation` handle the shape, and
`bindAssuranceReviewAnnotation` enforces exact subject binding — same
`assurance_spec_id`, same `assurance_revision`, byte-exact
`assurance_spec_digest`, and every graded target (`document` / `section` /
`obligation` / `gate`) resolving inside the document. The twelve recommended
axes (`ASSURANCE_REVIEW_AXES`) are a closed vocabulary. An annotation is a
portable opinion: review tooling, aggregation, and admission are deliberately
out of scope, and a valid bound review grants no authority.

## Library

```ts
import {
  bindAssuranceReviewAnnotation,
  compileSemanticPlannerProposal,
  prepareSemanticPlannerInput,
  runSemanticPlannerProposal,
  ingestAgentRun,
  inventoryRepository,
  proposeAssuranceSpec,
  validateAssuranceReviewAnnotation,
  validateAssuranceSpec,
} from "@openagentsinc/assurance-spec";
```

`proposeAssuranceSpec` is pure. Filesystem and Git access remain isolated in
`inventoryRepository` and the CLI.

`prepareSemanticPlannerInput` runtime-decodes the accepted subject and optional
inventory, checks the subject against exact ProductSpec bytes, freezes source
snapshots/digests, and emits a digest-bound typed request.
`runSemanticPlannerProposal` accepts an injected provider-neutral Effect
planner. `compileSemanticPlannerProposal` runtime-decodes and deterministically
compiles its output only after exact input, subject, and complete-disposition
checks. Extra planner fields cannot overwrite frozen source claims.

A future UI should decode this document and project its structural and
adequacy states. It must not maintain a hardcoded parallel assurance plan or
turn `proposed`, `needs_design`, or `not run` into pass state.

The architecture and authority boundaries live in
[`../../docs/assurance/ASSURANCE_SPEC.md`](../../docs/assurance/ASSURANCE_SPEC.md).

## Starter kit and owned-runner gate

`starter-kit/` is a one-commit adoption example: ProductSpec, proposed
AssuranceSpec, the short agent contract, and a typed
`assurance/owned-runner.json`. Run it locally or on OpenAgents-owned compute:

```sh
assurance-spec owned-runner assurance/owned-runner.json --root . --json
```

Structural validation and stale committed session pins block. The three
ledgers remain informational. The runner never computes or gates on a blended
percentage. Its schema requires `github_hosted_ci: false`. No GitHub Actions
workflow is shipped because the OpenAgents repository forbids hosted CI.

Distribution readiness is independently reproducible:

```sh
pnpm --dir packages/assurance-spec run pack:public -- --out /tmp/assurance-pack
pnpm --dir packages/assurance-spec run verify:distribution
```

The packager rewrites monorepo-only dependency protocols to concrete versions,
packs ProductSpec before AssuranceSpec, records SHA-256 tarball receipts, then
the verifier installs the exact tarballs offline in a clean temporary checkout
and runs the starter gate. Registry publication and no-auth clean-consumer
proofs are recorded separately in the adopting repository's immutable
distribution and registry receipts. Package prose is not publication
authority. The repository's own adoption lives at
`assurance/owned-runner.json` with a committed current MVP session pin. It runs
the same structural blocking and informational-ledger policy as the kit.
