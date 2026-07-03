# Connector Sidecar Contract

`@openagentsinc/connector-sidecar` owns the provider-event ingress contract for
OpenAgents connector sidecars. The first provider is GitHub.

The package deliberately exposes only source-verified, bounded issue/PR event
envelopes, delivery idempotency keys, workspace-lane projections, and same-subject
writeback authorization helpers. It does not grant membership, payment, email,
identity, settlement, or broad repository authority, and it never returns raw
webhook bodies for model context.
