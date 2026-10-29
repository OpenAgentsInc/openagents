# Block NIP-FI projection and policy

`v1alpha1/identity.proto` remains the original unverified wire projection.
This slice changes no field, namespace, descriptor or signed-original contract.
Parsing a transported assertion or a `verified` flag grants no authority.

The selected implementation policy is
[`fi-rs256-policy-v1`](../../../../docs/protocol/2026-10-28-fi-rs256-policy-v1.md).
Its current Coax types represent local candidate/configuration/provider facts,
not a new interchange API. Keep them out of public discovery and logs.
No schema for remotely asserted TLS/authentication or snapshot verification is
introduced. Future protocol changes must version and document their actual
consumer, preserve original compact JWS bytes and retain verifier ownership.

Full RSA/JWS parsing and verification, actual authenticated key-source adapters,
AssertionPolicyId construction and fresh NIP-42 pairing remain explicit gates.
The existing alpha DTO is not their implementation.
