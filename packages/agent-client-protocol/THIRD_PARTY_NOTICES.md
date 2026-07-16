# Third-party notices

## Agent Client Protocol schema

- Project: Agent Client Protocol
- Source: https://github.com/agentclientprotocol/agent-client-protocol
- Pinned release: `schema-v1.19.0`
- Pinned commit: `a213df5240048f96d2b23f644984bb20c188a234`
- License: Apache License 2.0
- Vendored license: [`upstream/schema-v1.19.0/LICENSE`](./upstream/schema-v1.19.0/LICENSE)

The vendored schema and metadata assets are unmodified official release
artifacts. Their source URLs and SHA-256 digests are recorded in
[`upstream/schema-v1.19.0/SOURCE.json`](./upstream/schema-v1.19.0/SOURCE.json).

## Agent Client Protocol TypeScript SDK

- Package: `@agentclientprotocol/sdk@1.2.1`
- Source: https://github.com/agentclientprotocol/typescript-sdk/tree/v1.2.1
- Commit: `26da1ae7ab66fae0f5e77272dee3e5d562d24aee`
- License: Apache License 2.0
- npm integrity: `sha512-jwYUdOQR7tc+Zfch53VL4JJyUNK/46q03uUTYb+PjECsmnNl94XFXOfYLJ8RBpMNidXd1rpOAVgb0vqD98xImA==`

The SDK supplies generated TypeScript identities only for the explicitly
imported unstable namespace and serves as a schema-audit comparison point.
Stable OpenAgents types are generated directly from the stable release
artifact because SDK 1.2.1 is generated from the release's unstable schema.
