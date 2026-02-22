# Adapters (Serialization/Parsing Contract)

Adapters format provider requests and parse provider responses.

## Normative Rules

1. Adapters serialize/parse only.
2. Adapters do not validate tool params.
3. Adapters do not execute tools.
4. Adapters do not own retry policy.

## Why

This keeps safety, policy, and receipts centralized in runtime/control execution layers.

## Ownership Boundary

- Adapter responsibility: provider shape translation.
- Runtime responsibility: schema validation, execution policy, retries, receipts.
