import { APPLE_FM_CANONICAL_HELPER_VERSION, APPLE_FM_HELPER_PROTOCOL_VERSION } from "./index.js";

/**
 * `@openagentsinc/apple-fm-runtime/node` — Node host authority (AFS-00
 * reservation).
 *
 * Node host authority lives only in this subpath so a browser or mobile bundle
 * cannot import the helper host by accident. Packet AFS-02 moves helper
 * discovery, signature and digest verification, spawn, readiness, and shutdown
 * here from the Desktop app and the nested Pylon runtime.
 *
 * AFS-00 reserves the subpath and its boundary. Concrete `node:` process,
 * child-process, and filesystem access arrives with the AFS-02 implementation.
 */
export const APPLE_FM_RUNTIME_NODE_RESERVED = true as const;

/** The helper basename the AFS-02 supervisor discovers on the Node host. */
export const APPLE_FM_HELPER_BASENAME = "foundation-bridge" as const;

/** The default loopback port the helper listens on. */
export const APPLE_FM_DEFAULT_PORT = 11435 as const;

/**
 * The frozen native helper manifest shape. AFS-02 generates the manifest bytes
 * from the single wire-version source. The generated manifest binds the
 * protocol version, the helper version, the architecture, and the digest.
 */
export interface AppleFmHelperManifest {
  readonly protocolVersion: typeof APPLE_FM_HELPER_PROTOCOL_VERSION;
  readonly helperVersion: string;
  readonly architecture: "arm64" | "x64";
  readonly sha256: string;
}

/** The expected helper version the generated manifest must carry at release. */
export const APPLE_FM_EXPECTED_HELPER_VERSION = APPLE_FM_CANONICAL_HELPER_VERSION;
