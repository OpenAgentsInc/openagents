# Decision API contracts

Product scope, HTTP contracts, and the implemented bounded relay decision
worker contract. See the [service guide](../service/decision-worker.md) for its
admission and deployment limits.

[Documentation index](../README.md)

| Document | Topic |
| --- | --- |
| [classification-http](classification-http.md) | Classify through the gateway |
| [decision-api](decision-api.md) | OpenAgents Decision API |
| [relay-decision-contract](relay-decision-contract.md) | The relay decision contract |

The machine-readable HTTP contract is [openapi.yaml](openapi.yaml).

This is the standalone decision gateway's contract. The public API front at
`https://api.openagents.com` has a [combined reference](https://api.openagents.com/v1/openapi.json)
that includes only `/v1/systemone` from this contract. The front's inference
catalog does not have the standalone gateway's door-catalog response shape.
Classification, jobs, and account-service operations are not made public by
composition. See the [public route inventory](../../sdk/public-routes.md).

The [public slice](../../sdk/openapi.systemone.json) is generated from
`openapi.yaml` with `python3 scripts/api/systemone_openapi.py` (PyYAML).
Use `--check` for a deterministic manual drift check. Refusal code examples name
known codes without forbidding backend-specific codes. Request identity,
attempts, workspace membership, policy-gated anonymous access, quota settlement,
and response metadata remain the owning gateway's responsibility.
