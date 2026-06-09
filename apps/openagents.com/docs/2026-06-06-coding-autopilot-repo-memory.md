# Coding On Autopilot Repo Memory

Date: 2026-06-06

Status: implemented contract note for GitHub issue #318 / `OPENAGENTS-071`.

## Purpose

Repo memory lets future Coding on Autopilot missions reuse reviewed facts about
a repository without rereading or publicly projecting private workroom context.
It is an evidence layer, not hidden routing authority.

The implementation lives in `workers/api/src/coding-autopilot-repo-memory.ts`.

## Memory Kinds

The v1 model supports:

- `accepted_fix`;
- `rejected_fix`;
- `build_command`;
- `test_command`;
- `flaky_test`;
- `denied_path`;
- `repo_convention`;
- `pr_style`;
- `dependency_note`;
- `reviewer_preference`.

## Record Shape

`CodingAutopilotRepoMemoryRecord` stores:

- memory kind;
- memory ref;
- repo ref and repo visibility;
- mission and workroom refs;
- source-authority refs;
- evidence refs;
- selector refs;
- semantic index refs;
- source state;
- confidence;
- status;
- expiration/review refs as backend timestamps;
- summary and caveat refs.

## Retrieval Rule

Ad hoc keyword routing is explicitly not allowed. Every record carries
`keywordRoutingAllowed: false`.

Retrieval must be one of:

- `typed_selector`, requiring selector refs;
- `semantic_embedding`, requiring semantic index refs;
- `manual_review`.

Future routing or retrieval consumers must use these typed selectors or
semantic retrieval contracts instead of text matching.

## Projection Rules

Public projection hides private repo refs, source-authority refs, and workroom
refs.

Customer and team projections can see safe private repo refs when they are
authorized by the surrounding product surface, but source-authority internals
remain operator-only.

Operator projection can see safe source-authority refs.

All projections use friendly time labels and reject raw timestamps, raw runner
logs, provider tokens, raw patches, source archives, private repo URLs,
customer emails, payment material, wallet material, and secrets.

## Tests

`workers/api/src/coding-autopilot-repo-memory.test.ts` covers:

- public/private repo projection splits;
- every required memory kind;
- expiration and review-needed status derivation;
- typed selector and semantic index requirements;
- `keywordRoutingAllowed: false`;
- unsafe provider, runner, private repo, customer, and raw timestamp rejection.
