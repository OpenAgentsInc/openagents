import {
  SARAH_NOSTR_RECORD_MODE_ENV,
  SARAH_NOSTR_SHADOW_PUBLISH_ENV,
  type SarahNostrRecordMode,
} from "./types.ts";
import { defaultStageForRecordMode } from "./stages.ts";
import type { SarahNostrMigrationStage } from "./types.ts";

const VALID_MODES = new Set<SarahNostrRecordMode>(["khala", "shadow", "nostr"]);

export const isSarahNostrRecordMode = (
  value: unknown,
): value is SarahNostrRecordMode =>
  typeof value === "string" && VALID_MODES.has(value as SarahNostrRecordMode);

/**
 * Resolve SARAH_NOSTR_RECORD_MODE from process env.
 *
 * Precedence:
 * 1. SARAH_NOSTR_RECORD_MODE=khala|shadow|nostr (explicit)
 * 2. SARAH_NOSTR_SHADOW_PUBLISH=1 → shadow (legacy SARAH-NR-05)
 * 3. default → khala
 *
 * Invalid RECORD_MODE values fall through to the legacy flag / default so a
 * typo does not silently enable Nostr-primary.
 */
export const resolveSarahNostrRecordMode = (
  env: NodeJS.ProcessEnv = process.env,
): SarahNostrRecordMode => {
  const raw = env[SARAH_NOSTR_RECORD_MODE_ENV]?.trim().toLowerCase();
  if (raw !== undefined && raw !== "" && isSarahNostrRecordMode(raw)) {
    return raw;
  }
  if (env[SARAH_NOSTR_SHADOW_PUBLISH_ENV] === "1") {
    return "shadow";
  }
  return "khala";
};

/** True when dual-publish or Nostr-primary publish should run. */
export const shouldPublishSarahNostrFromMode = (
  mode: SarahNostrRecordMode = resolveSarahNostrRecordMode(),
): boolean => mode === "shadow" || mode === "nostr";

/** True when Khala Sync remains the writable record authority. */
export const khalaRemainsRecordAuthority = (
  mode: SarahNostrRecordMode = resolveSarahNostrRecordMode(),
): boolean => mode === "khala" || mode === "shadow";

/** True when the relay is the writable record authority. */
export const nostrIsRecordAuthority = (
  mode: SarahNostrRecordMode = resolveSarahNostrRecordMode(),
): boolean => mode === "nostr";

/**
 * Resolve the implied migration stage for the current mode, or null when
 * mode is pre-migration `khala`.
 */
export const resolveImpliedMigrationStage = (
  mode: SarahNostrRecordMode = resolveSarahNostrRecordMode(),
): SarahNostrMigrationStage | null => defaultStageForRecordMode(mode);
