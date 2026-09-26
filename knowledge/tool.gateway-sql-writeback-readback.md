---
id: tool.gateway-sql-writeback-readback
version: 1
kind: tool
title: Inspect, apply, and verify restricted SQL gateway writebacks
summary: >-
  Use a supplied database gateway as the authority for schema, access
  permissions, and write execution; validate the generated SQL and confirm
  results through readback and the gateway audit trail. Applies when several
  systems accept inserts only through an approved wrapper.
tags: [sql, gateway, writeback, audit]
applies_when: >-
  A task provides a database gateway or wrapper with system-specific
  credentials, allowed tables, and SQL-file execution.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - production-planning
  cites:
    - SQLite, “INSERT,” SQL Language.
    - SQLite, “Transactions,” SQL Language.
evidence: []
---

## Details

Read the gateway documentation and connection configuration before issuing queries. Discover schemas through the gateway, then use read-only queries to inspect current state and constraints; do not assume table columns, keys, write permissions, or that direct database access is acceptable. Generate standalone SQL files from a canonical data model so related systems receive consistent identifiers and values. Check quoting, null handling, uniqueness, required columns, and statement boundaries before execution.

Apply each file only through the approved gateway and only to its authorized system. A successful command is not sufficient evidence that the intended state is correct: inspect the audit record, query the affected tables back through the gateway, and verify row counts plus key-level invariants and cross-system consistency. SQLite documents the semantics of `INSERT` and transactions in its SQL language reference; wrapper-specific permission and audit behavior must be taken from that wrapper's own documentation.

## How to check

Use the gateway's documented schema, query, and file-execution commands (for example, a typical wrapper exposes `schema --system`, `exec --system --sql`, and `exec --system --file`). After execution, read back the inserted rows and assert expected counts, distinct identifiers, required statuses, and matching references across systems. Confirm the audit trail records successful execution for every intended writeback. Do not infer full correctness from an aggregate count alone.

References: SQLite, “INSERT” (SQL Language); SQLite, “Transactions” (SQL Language).
