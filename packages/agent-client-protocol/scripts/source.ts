export const SCHEMA_RELEASE = "schema-v1.19.0";
export const WIRE_VERSION = 1;
export const UPSTREAM_COMMIT = "a213df5240048f96d2b23f644984bb20c188a234";

export const UPSTREAM_ASSETS = {
  "meta.json": {
    url: "https://github.com/agentclientprotocol/agent-client-protocol/releases/download/schema-v1.19.0/meta.json",
    sha256: "e0bf36f8123b2544b499174197fdc371ec49a1b4572a35114513d56492741599",
  },
  "schema.json": {
    url: "https://github.com/agentclientprotocol/agent-client-protocol/releases/download/schema-v1.19.0/schema.json",
    sha256: "92c1dfcda10dd47e99127500a3763da2b471f9ac61e12b9bf0430c32cf953796",
  },
  "meta.unstable.json": {
    url: "https://github.com/agentclientprotocol/agent-client-protocol/releases/download/schema-v1.19.0/meta.unstable.json",
    sha256: "3026898232badf413624010d1343e20bef853e6705c62d6b56387cf9de6b0543",
  },
  "schema.unstable.json": {
    url: "https://github.com/agentclientprotocol/agent-client-protocol/releases/download/schema-v1.19.0/schema.unstable.json",
    sha256: "8bdfd8347ce8bd2c8620b71bfd5460625f91c7db47a51268bb42b67014ea5b1f",
  },
  LICENSE: {
    url: `https://raw.githubusercontent.com/agentclientprotocol/agent-client-protocol/${UPSTREAM_COMMIT}/LICENSE`,
    sha256: "f250d08cee4549b22b3b4aaaf3a743473336fd280316df5d0340717e5127a221",
  },
} as const;

export const SDK_AUTHORITY = {
  package: "@agentclientprotocol/sdk",
  version: "1.2.1",
  sourceCommit: "26da1ae7ab66fae0f5e77272dee3e5d562d24aee",
  schemaLane: "unstable",
  schemaSha256: UPSTREAM_ASSETS["schema.unstable.json"].sha256,
  npmSha1: "c98952123d2b202a143ab5ec68782eec2775003a",
  npmIntegrity:
    "sha512-jwYUdOQR7tc+Zfch53VL4JJyUNK/46q03uUTYb+PjECsmnNl94XFXOfYLJ8RBpMNidXd1rpOAVgb0vqD98xImA==",
} as const;
