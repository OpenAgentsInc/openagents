import type {
  ContextManifestRef,
  ProjectRef,
  WorktreeRef,
} from "@openagentsinc/agent-runtime-schema";

/**
 * `@openagentsinc/ide-runtime` — reserved root package (AFS-00).
 *
 * Packet AFS-05 extracts the portable IDE project, root, worktree, attachment,
 * generation, context, cursor, and proposal schemas and pure services here from
 * the current Desktop IDE contracts. AFS-00 only reserves the package, its
 * manifest, its export map, and its import boundary.
 *
 * Ownership: portable IDE schemas and pure services. It must not own platform
 * adapters (file, Git, DAP, PTY, language process, Electron, or Monaco). The
 * cross-surface context envelope and its references stay in
 * `@openagentsinc/agent-runtime-schema`; this package owns the detailed IDE
 * context and proposal schemas when AFS-05 extracts them.
 */
export const IDE_RUNTIME_PACKAGE = "@openagentsinc/ide-runtime" as const;

/** AFS-00 reservation marker. AFS-05 replaces this with real IDE schemas. */
export const IDE_RUNTIME_RESERVED = true as const;

/**
 * The cross-surface IDE reference vocabulary this package will extend. The
 * detailed IDE context and proposal schemas are added by AFS-05; these
 * references stay anchored to the shared schema package.
 */
export type IdeRuntimeContextManifestRef = ContextManifestRef;
export type IdeRuntimeProjectRef = ProjectRef;
export type IdeRuntimeWorktreeRef = WorktreeRef;

export * from "./ide-review-projection.js";
export * from "./ide-review-projector.js";
export * from "./ide-review-client-cache.js";
