/**
 * IDR-02 existence-only candidate discovery.
 *
 * Discovery enumerates known identity-candidate locations by EXISTENCE and
 * METADATA ONLY. It runs the audit "Phase 0: discovery without secret reads":
 *
 * - It uses `lstat` for each path, so it never follows a symbolic link and never
 *   opens a file for reading. It reads NO secret bytes.
 * - It refuses an unexpected symbolic link by default (`link_refused`).
 * - It reports a weak-permission file as a custody blocker (`weak_permissions`).
 * - It records only a PUBLIC source label, the filesystem type, the permission
 *   mode, a coarse size class, and a modification-time label. It never records
 *   the raw private path or any file content.
 *
 * Discovery can NEVER create or overwrite a file. It has no write path and no
 * `readFile` path. The candidate probe surface is `lstat` only, injected as a
 * narrow `CandidateStatProbe`, so a caller can prove existence-only inspection.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { lstatSync } from "node:fs";
import { Effect, Schema as S } from "effect";

/** The filesystem type of a candidate, from an existence-only `lstat`. */
export const CandidateFsType = S.Literals([
  "absent",
  "regular_file",
  "symbolic_link",
  "directory",
  "other",
  "metadata_unavailable",
]);
export type CandidateFsType = typeof CandidateFsType.Type;

/**
 * A fail-closed candidate blocker. Neither value admits a candidate; each is a
 * refusal recorded during existence-only inspection. `link_refused` is the
 * default refusal of a symbolic link; `weak_permissions` is the custody blocker
 * for a file that grants group or other access.
 */
export const CandidateBlocker = S.Literals(["link_refused", "weak_permissions"]);
export type CandidateBlocker = typeof CandidateBlocker.Type;

/** A coarse size class from `lstat` size. It never reflects file content. */
export const CandidateSizeClass = S.Literals(["empty", "small", "medium", "large", "unknown"]);
export type CandidateSizeClass = typeof CandidateSizeClass.Type;

/**
 * One existence-only candidate diagnostic. It is PUBLIC-SAFE: it carries a source
 * label, not the raw private path, and it never carries file content. `present`
 * and `admissible` are separate: a symbolic link or a weak-permission file is
 * present but not admissible.
 */
export const CandidateDiagnostic = S.Struct({
  /** A stable PUBLIC label for the source, never the raw private path. */
  sourceLabel: S.String.check(S.isMinLength(1)),
  /** The filesystem type, from `lstat`. */
  fsType: CandidateFsType,
  /** Whether an entry exists at the path. */
  present: S.Boolean,
  /**
   * Whether the candidate is admissible for an authorized read later. It is true
   * only for a present regular file with no blocker. A symbolic link or a
   * weak-permission file is never admissible.
   */
  admissible: S.Boolean,
  /** The POSIX permission mode as an octal string, or `null` on Windows or when absent. */
  permissionMode: S.NullOr(S.String),
  /** A coarse size class from `lstat` size, never from content. */
  sizeClass: CandidateSizeClass,
  /** An RFC 3339 modification-time label, or `null` when absent. */
  modifiedAtIso: S.NullOr(S.String),
  /** The fail-closed blocker, or `null` when the candidate has none. */
  blocker: S.NullOr(CandidateBlocker),
});
export type CandidateDiagnostic = typeof CandidateDiagnostic.Type;

/**
 * The narrow existence-only probe surface discovery is allowed to use. It has
 * `lstat` only. It has no `readFile`, no `open`, and no write method, so a
 * caller can prove by construction that discovery reads no secret bytes.
 */
export interface CandidateStatProbe {
  readonly lstatSync: (absolutePath: string) => {
    readonly isSymbolicLink: () => boolean;
    readonly isFile: () => boolean;
    readonly isDirectory: () => boolean;
    readonly size: number;
    readonly mode: number;
    readonly mtimeMs: number;
  };
}

/** The default probe. It binds Node `lstat` only; it exposes no read or write. */
export const nodeStatProbe: CandidateStatProbe = { lstatSync };

/** A candidate location to inspect: a PUBLIC label plus the private absolute path. */
export interface CandidateSpec {
  readonly sourceLabel: string;
  readonly absolutePath: string;
}

/** Options for a single inspection. Both fields default to the running platform. */
export interface InspectOptions {
  readonly probe?: CandidateStatProbe;
  readonly platform?: NodeJS.Platform;
}

const sizeClassOf = (size: number): CandidateSizeClass => {
  if (!Number.isFinite(size) || size < 0) return "unknown";
  if (size === 0) return "empty";
  if (size <= 256) return "small";
  if (size <= 4096) return "medium";
  return "large";
};

const permissionModeString = (mode: number, platform: NodeJS.Platform): string | null =>
  platform === "win32" ? null : (mode & 0o777).toString(8).padStart(3, "0");

/** True when a POSIX mode grants any group or other access. */
const grantsGroupOrOther = (mode: number): boolean => (mode & 0o077) !== 0;

/**
 * Inspect ONE candidate location by existence and metadata only. It calls
 * `lstat` exactly once and never reads file content. A symbolic link is refused
 * (`link_refused`). A regular file that grants group or other access is a
 * custody blocker (`weak_permissions`). A missing path is `absent`, never an
 * error, so an open path can stop cleanly with no candidate.
 */
export function inspectCandidatePath(
  spec: CandidateSpec,
  options: InspectOptions = {},
): CandidateDiagnostic {
  const probe = options.probe ?? nodeStatProbe;
  const platform = options.platform ?? process.platform;

  let stat: ReturnType<CandidateStatProbe["lstatSync"]>;
  try {
    stat = probe.lstatSync(spec.absolutePath);
  } catch (error) {
    const code = (error as { code?: unknown }).code;
    if (code === "ENOENT" || code === "ENOTDIR") {
      return {
        sourceLabel: spec.sourceLabel,
        fsType: "absent",
        present: false,
        admissible: false,
        permissionMode: null,
        sizeClass: "unknown",
        modifiedAtIso: null,
        blocker: null,
      };
    }
    // Any other error (for example a permission error on a parent directory)
    // leaves the metadata unknown. It is present-unknown and never admissible.
    return {
      sourceLabel: spec.sourceLabel,
      fsType: "metadata_unavailable",
      present: false,
      admissible: false,
      permissionMode: null,
      sizeClass: "unknown",
      modifiedAtIso: null,
      blocker: null,
    };
  }

  const modifiedAtIso = Number.isFinite(stat.mtimeMs) ? new Date(stat.mtimeMs).toISOString() : null;

  if (stat.isSymbolicLink()) {
    return {
      sourceLabel: spec.sourceLabel,
      fsType: "symbolic_link",
      present: true,
      admissible: false,
      permissionMode: permissionModeString(stat.mode, platform),
      sizeClass: sizeClassOf(stat.size),
      modifiedAtIso,
      blocker: "link_refused",
    };
  }

  if (stat.isDirectory()) {
    return {
      sourceLabel: spec.sourceLabel,
      fsType: "directory",
      present: true,
      admissible: false,
      permissionMode: permissionModeString(stat.mode, platform),
      sizeClass: "unknown",
      modifiedAtIso,
      blocker: null,
    };
  }

  if (!stat.isFile()) {
    return {
      sourceLabel: spec.sourceLabel,
      fsType: "other",
      present: true,
      admissible: false,
      permissionMode: permissionModeString(stat.mode, platform),
      sizeClass: sizeClassOf(stat.size),
      modifiedAtIso,
      blocker: null,
    };
  }

  const weak = platform !== "win32" && grantsGroupOrOther(stat.mode);
  return {
    sourceLabel: spec.sourceLabel,
    fsType: "regular_file",
    present: true,
    admissible: !weak,
    permissionMode: permissionModeString(stat.mode, platform),
    sizeClass: sizeClassOf(stat.size),
    modifiedAtIso,
    blocker: weak ? "weak_permissions" : null,
  };
}

/**
 * An existence-only identity-candidate source. `discover` returns one diagnostic
 * per known candidate location. It never reads secret bytes and never mutates
 * the filesystem.
 */
export interface IdentityCandidateSource {
  readonly discover: () => Effect.Effect<ReadonlyArray<CandidateDiagnostic>>;
}

/**
 * A filesystem candidate source over a fixed list of candidate specs. It inspects
 * each path with `lstat` only, so it creates nothing, overwrites nothing, and
 * reads no secret bytes. Callers pass PUBLIC labels; the raw private paths never
 * leave the diagnostics.
 */
export function nodeFsCandidateSource(
  specs: ReadonlyArray<CandidateSpec>,
  options: InspectOptions = {},
): IdentityCandidateSource {
  return {
    discover: () => Effect.sync(() => specs.map((spec) => inspectCandidatePath(spec, options))),
  };
}

/**
 * A deterministic in-memory candidate source. It returns the given diagnostics
 * unchanged and touches no filesystem. Tests use it to drive the open flow
 * across the acceptance-matrix cases without a real file.
 */
export function inMemoryCandidateSource(
  diagnostics: ReadonlyArray<CandidateDiagnostic>,
): IdentityCandidateSource {
  return { discover: () => Effect.sync(() => diagnostics) };
}

/** Partition diagnostics into the admissible set and the fail-closed blockers. */
export function summarizeDiagnostics(diagnostics: ReadonlyArray<CandidateDiagnostic>): {
  readonly admissible: ReadonlyArray<CandidateDiagnostic>;
  readonly blockers: ReadonlyArray<CandidateBlocker>;
} {
  const admissible = diagnostics.filter((diagnostic) => diagnostic.admissible);
  const blockers: CandidateBlocker[] = [];
  for (const diagnostic of diagnostics) {
    if (diagnostic.blocker !== null && !blockers.includes(diagnostic.blocker)) {
      blockers.push(diagnostic.blocker);
    }
  }
  return { admissible, blockers };
}
