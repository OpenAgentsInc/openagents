# `@openagentsinc/assurance-spec`

Deterministic proposal, parsing, structural validation, serialization, and
adequacy assessment for the proposed AssuranceSpec `0.1` companion format.

This first implementation turns an executable ProductSpec into exact assurance
coverage scaffolding: exact ProductSpec identity, one proposed obligation per
criterion, exact source-claim snapshots, and explicit `needs_design`
diagnostics for every unresolved proof-design field. Optional repository
inventory is pinned to committed Git `HEAD`.

It does **not** use a model, infer semantics from filenames, map tests to
criteria, choose tools or environments, run tests, admit proof design, create
evidence, verify results, authorize release, or change public promises.
Repository test-looking paths and package scripts are unbound proposal context,
not proof.

## CLI

```bash
bun packages/assurance-spec/src/cli.ts propose \
  docs/mvp/openagents-codex-workroom-mvp.product-spec.md \
  --repo . \
  --out docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md \
  --force

bun packages/assurance-spec/src/cli.ts validate \
  docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md

bun packages/assurance-spec/src/cli.ts coverage \
  docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md
```

`propose` succeeds when it creates a structurally valid proposal even if every
obligation still needs design. `coverage` reports adequacy separately.

### Agent tooling (AT-1)

The CLI also ships the deterministic agent surface designed in
[`../../docs/assurance/AGENT_TOOLING.md`](../../docs/assurance/AGENT_TOOLING.md):

```bash
# Stateless dual-digest sessions (the full pin is returned to the caller)
bun packages/assurance-spec/src/cli.ts session begin <file.assurance-spec.md> [--root <dir>] [--json]
bun packages/assurance-spec/src/cli.ts session check <file.assurance-spec.md> \
  (--against <session.json> | --spec-digest <hex> --subject-digest <hex>) [--root <dir>] [--json]

# Read-only reports (never a verdict, never a blended score)
bun packages/assurance-spec/src/cli.ts obligations <file> [--criterion <id>] [--status ready|needs_design] [--technique <t>] [--json]
bun packages/assurance-spec/src/cli.ts obligation <file> <obligation-id> [--json]
bun packages/assurance-spec/src/cli.ts ledgers <file> [--json]
bun packages/assurance-spec/src/cli.ts checklist <file> [--criterion <id>] [--json]
bun packages/assurance-spec/src/cli.ts claim <file> [--claim "<text>"] [--json]
bun packages/assurance-spec/src/cli.ts inventory <repo-dir> [--out <file.json>] [--json]

# Read-only stdio MCP server (JSON-RPC 2.0, protocol 2024-11-05), confined to --root
bun packages/assurance-spec/src/cli.ts mcp --root .
```

Exit codes are the API: **0** success, **1** operation failure, **2** usage
error, **3** stale session. Every command takes `--json`.

The MCP server exposes the §3.1 read-only tool table
(`begin_assurance_session`, `check_assurance_session`, `list_assurance_specs`,
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

Without `--repo`, proposal generation remains valid and emits a typed
`repository_not_supplied` diagnostic. With `--repo`, inventory reads the
committed tree and tracked dirty state only. It does not read ignored or
untracked files, run package scripts, inspect remotes, or serialize absolute
paths.

## Library

```ts
import {
  inventoryRepository,
  proposeAssuranceSpec,
  validateAssuranceSpec,
} from "@openagentsinc/assurance-spec"
```

`proposeAssuranceSpec` is pure. Filesystem and Git access remain isolated in
`inventoryRepository` and the CLI.

A future UI should decode this document and project its structural and
adequacy states. It must not maintain a hardcoded parallel assurance plan or
turn `proposed`, `needs_design`, or `not run` into pass state.

The architecture and authority boundaries live in
[`../../docs/assurance/ASSURANCE_SPEC.md`](../../docs/assurance/ASSURANCE_SPEC.md).
