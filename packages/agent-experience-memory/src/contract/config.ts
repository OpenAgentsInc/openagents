import { Context, Layer, Schema as S } from "effect";

/**
 * The default-off flag that gates every memory read and write (AFS-10).
 *
 * Memory is a conservative, optional addition. Version one ships with recall
 * OFF. With `enabled` false the router and the IDE behave exactly as they do
 * without this package: no bank is frozen, no record is read, no record is
 * written, and no recalled slice is added to any prompt. The flag exists so a
 * measured benefit can promote memory later, never so memory becomes a silent
 * dependency.
 */
export const MEMORY_CONFIG_SCHEMA_LITERAL = "openagents.experience_memory_config.v1" as const;

/** The default checked-in state: memory recall is OFF. */
export const MEMORY_DEFAULT_ENABLED = false as const;

/** The surfaces a memory config can bind to. Apple FM memory is strictly on-device. */
export const MemorySurface = S.Literals(["apple_fm", "codex_coding"]);
export type MemorySurface = typeof MemorySurface.Type;

/**
 * The bounded memory configuration. `maxRecallTokens` caps the recalled slice so
 * recall can never grow the prompt without bound; `maxRecords` caps how much of
 * a frozen bank the adapter considers.
 */
export const MemoryConfigShape = S.Struct({
  schema: S.Literal(MEMORY_CONFIG_SCHEMA_LITERAL),
  enabled: S.Boolean,
  surface: MemorySurface,
  maxRecallTokens: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(4000)),
  maxRecords: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1000)),
  /**
   * Local-only is a hard invariant for the Apple FM path. This field is always
   * true in version one; there is no non-local adapter. It is present so a
   * future non-local adapter needs an explicit, reviewed flip and a separate
   * owner decision, never a silent default.
   */
  localOnly: S.Boolean,
});
export type MemoryConfigShape = typeof MemoryConfigShape.Type;

const decodeConfig = S.decodeUnknownSync(MemoryConfigShape);

/** The default OFF configuration for a surface. Recall is disabled and local-only. */
export const defaultMemoryConfig = (surface: MemorySurface): MemoryConfigShape =>
  decodeConfig({
    schema: MEMORY_CONFIG_SCHEMA_LITERAL,
    enabled: MEMORY_DEFAULT_ENABLED,
    surface,
    maxRecallTokens: 512,
    maxRecords: 64,
    localOnly: true,
  });

/**
 * The config service. Its `Default` layer yields the OFF Apple FM config, so a
 * host that forgets to provide a config gets no memory rather than accidental
 * memory. An enabled config is an explicit, per-surface opt-in.
 */
export class MemoryConfig extends Context.Service<MemoryConfig, MemoryConfigShape>()(
  "agent-experience-memory/MemoryConfig",
) {
  static readonly Default = Layer.succeed(MemoryConfig, defaultMemoryConfig("apple_fm"));
  static readonly withConfig = (config: MemoryConfigShape): Layer.Layer<MemoryConfig> =>
    Layer.succeed(MemoryConfig, config);
}
