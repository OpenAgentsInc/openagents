import { Schema as S } from "effect";

/**
 * Bootstrap recipe: the files an adapter ships into a fresh session workspace
 * and the commands it runs there before accepting a prompt (install deps, drop
 * bridge files). Declared so a sandbox provider can compute a stable identity
 * and bake the result into a reusable snapshot/checkpoint. Adapters with no
 * bootstrap needs omit `getBootstrap` on the adapter entirely.
 *
 * HARN-01 fixes the shape; HARN-07 wires it to the managed-sandbox
 * `onFirstCreate`/snapshot-identity path.
 */
export const HarnessBootstrapFile = S.Struct({
  path: S.NonEmptyString,
  content: S.String,
  /** Optional POSIX mode (e.g. 0o755) for executable bridge files. */
  mode: S.optionalKey(S.Number),
});
export interface HarnessBootstrapFile extends S.Schema.Type<typeof HarnessBootstrapFile> {}

export const HarnessBootstrapCommand = S.Struct({
  command: S.NonEmptyString,
  /** Working directory for the command, relative to the session workspace root. */
  cwd: S.optionalKey(S.String),
});
export interface HarnessBootstrapCommand extends S.Schema.Type<typeof HarnessBootstrapCommand> {}

export const HarnessBootstrap = S.Struct({
  /**
   * Stable identity for snapshot reuse. Two sessions with the same identity may
   * resume from the same baked snapshot; the recipe application hook runs once
   * per identity.
   */
  identity: S.NonEmptyString,
  files: S.optionalKey(S.Array(HarnessBootstrapFile)),
  commands: S.optionalKey(S.Array(HarnessBootstrapCommand)),
});
export interface HarnessBootstrap extends S.Schema.Type<typeof HarnessBootstrap> {}
