# Artanis Codex Agent Task Execution Policy

- **Date:** 2026-06-27
- **Scope:** Issue #6367, under epic #6359.

Artanis does not get an arbitrary-shell execution tool. Owner-local coding work
must flow through the typed Khala -> Pylon -> Codex `codex_agent_task` path so
every action has an assignment closeout, exact downstream token rows, and an
owner-private/public-safe trace split.

The dispatch tool requires a public-safe verification command before it can
create live work. The Worker execution seam also rejects direct calls that omit
verification or cannot resolve the pinned commit for a verified workspace. That
prevents fixture-only or unverifiable repository work from entering the Artanis
burndown path.

Non-spend code may be merged only after the assignment closeout reports green
verification/proof. Spend, settlement, wallet, deployment, runtime promotion,
and other risky actions remain behind the Artanis approval-gate ledger.
