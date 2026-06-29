# Pack C Workspace Authority Boundary

Date: 2026-06-12

## Scope

This is the Pack C file, shell, sandbox, and workspace authority record for
#4834. It is a delivery-evidence contract, not a broad terminal
implementation.

## Contract

Workspace evidence projections carry:

- workspace ref
- sandbox profile ref
- expected sandbox profile ref when a policy comparison is required
- operation kind
- command intent ref
- allowed command intent refs
- allowed path refs
- touched path refs
- approval refs when required
- timeout and cancellation refs when present
- redaction class and redaction receipt refs
- typed blocker refs

## Denials

Workspace evidence denies unsafe or incomplete operations with typed blockers:

- out-of-scope path
- missing approval
- command not allowed
- sandbox mismatch
- timeout
- cancellation
- redaction required

Denied evidence may still be retained as an audit ref. It does not satisfy
delivery, acceptance, merge, settlement, payout, or public-claim authority.

## Boundaries

Public or agent-readable workspace evidence must reject raw shell logs, raw
commands, raw prompts, local filesystem paths, private repo content, provider
payloads, credentials, wallet/payment material, and customer-private data
before projection.
