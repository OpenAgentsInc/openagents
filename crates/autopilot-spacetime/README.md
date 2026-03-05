# autopilot-spacetime

Core schema and reducer primitives for OpenAgents Spacetime sync migration.

Current scope:

1. Canonical core table/index metadata.
2. Canonical SQL DDL snapshot for core schema.
3. Reducer primitives and deterministic stream sequencing.
4. Subscription query planning and index coverage checks.
5. Scope/stream-grant authorization helpers for subscription and reducer operations.
6. Typed client primitives for protocol negotiation, subscribe/reconnect/resume, and reducer calls.
7. Contract tests for schema/auth/client semantics.
