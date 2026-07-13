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
