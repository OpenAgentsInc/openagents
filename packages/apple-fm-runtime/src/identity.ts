/**
 * `@openagentsinc/apple-fm-runtime` frozen identity + single version source
 * (AFS-02).
 *
 * This leaf module owns the SINGLE wire-version source. AFS-02 generates the
 * native helper manifest, the Swift `bridgeVersion`, and the Desktop staging
 * pin from these values so they cannot drift again (the AFS-00 check flagged a
 * `0.1.3` helper vs `0.1.1` Desktop staging mismatch; AFS-02 owns closing it).
 *
 * It has no dependency on any other module, so the portable wire, client,
 * supervisor, recommendation, and provider modules can import it without an
 * import cycle through the package index.
 */

/** Apple FM is the local advisory inference candidate. It is never a router. */
export const APPLE_FM_TURN_CANDIDATE = "apple_fm" as const;

export const APPLE_FM_BRIDGE_WIRE_VERSION = "openagents.apple_fm.bridge.wire.v0.2" as const;
export const APPLE_FM_HELPER_PROTOCOL_VERSION = 1 as const;
export const APPLE_FM_BACKEND_KIND = "apple_fm_bridge" as const;
export const APPLE_FM_LOCAL_PROFILE_ID = "apple-fm-local" as const;
export const APPLE_FM_DEFAULT_MODEL_ID = "apple-foundation-model" as const;
export const APPLE_FM_DEFAULT_BASE_URL = "http://127.0.0.1:11435" as const;

/**
 * Canonical helper version — the single source AFS-02 generates every other
 * copy from (the Swift `foundation-bridge` source, the native manifest, and
 * the Desktop staging pin). The AFS-02 drift check proves every copy agrees.
 */
export const APPLE_FM_CANONICAL_HELPER_VERSION = "0.1.3" as const;

/**
 * The accepted Desktop staging version. It is defined as the canonical helper
 * version, so the Desktop staging pin is DERIVED from this single source rather
 * than a second literal that can drift.
 */
export const APPLE_FM_ACCEPTED_STAGING_VERSION = APPLE_FM_CANONICAL_HELPER_VERSION;

/** Local data destination. Apple FM input stays on the device. */
export const APPLE_FM_DATA_DESTINATION = "on_device_local" as const;

/** Apple FM declares no external tool or action capability. */
export const APPLE_FM_SUPPORTS_EXTERNAL_ACTIONS = false as const;

/**
 * Compare the canonical helper version with a staging pin. AFS-02 uses this to
 * fail closed when a generated staging value does not match the single source.
 */
export const appleFmVersionSourcesAgree = (helperVersion: string, stagingVersion: string): boolean =>
  helperVersion === stagingVersion;
