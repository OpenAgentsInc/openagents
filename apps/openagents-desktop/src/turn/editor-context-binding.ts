import { Schema as S } from "effect"

import {
  brandedTurnRef,
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  ContextItem,
  ContextManifestRef,
  ContextSourceKind,
  MAX_TURN_CONTEXT_CHARS,
  ProjectRef,
  TurnThreadRef,
  WorkContextEnvelope,
  WorktreeRef,
} from "@openagentsinc/agent-runtime-schema"

/**
 * AFS-05 editor-context binding.
 *
 * This is the single, host-owned bridge that lets the Editor agent rail feed its
 * IDE-08 context (active file, selection, explicit attachments, root, worktree,
 * generations, and local lexical/symbol facts) into the SAME shared kernel
 * `TurnService` that chat uses, rather than a parallel authority path.
 *
 * The renderer can only DESCRIBE its editor context. It hands main a bounded
 * binding; the host validates it against the authoritative editor identity and
 * builds the effective `WorkContextEnvelope`. A binding for another project,
 * root, worktree, or generation is refused, so a stale context can never reach a
 * turn and a stale candidate can never apply.
 *
 * The module is pure Effect Schema and pure logic: no Electron, no Node host API,
 * no IDE-08/IDE-09 contract import. It stays inside the AFS turn vocabulary so it
 * remains unit-testable and safe to import from the typed turn IPC surface.
 */
export const EDITOR_CONTEXT_BINDING_SCHEMA_LITERAL = "openagents.editor_context_binding.v1" as const

/** A workspace root reference. The frozen envelope binds project and worktree; the host also binds root here for refusal. */
export const EditorContextRootRef = brandedTurnRef("EditorContextRootRef")
export type EditorContextRootRef = typeof EditorContextRootRef.Type

/**
 * The authoritative identity of the editor a binding claims. A turn's context is
 * admitted only when the binding identity matches the host's current editor
 * identity exactly. Generation is monotonic; any difference is refused.
 */
export const EditorContextIdentity = S.Struct({
  projectRef: ProjectRef,
  rootRef: EditorContextRootRef,
  worktreeRef: WorktreeRef,
  generation: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
})
export interface EditorContextIdentity extends S.Schema.Type<typeof EditorContextIdentity> {}

/**
 * A single editor context item the renderer describes. It carries references and
 * truth flags only; never raw file bytes. `kind` names the source class, so
 * `local_lexical` and `local_symbol` items work with no remote embedding
 * provider.
 */
export const EditorContextItemInput = S.Struct({
  kind: ContextSourceKind,
  itemRef: brandedTurnRef("ContextItemRef"),
  derived: S.Boolean,
  byteLength: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  truncated: S.Boolean,
  redacted: S.Boolean,
})
export interface EditorContextItemInput extends S.Schema.Type<typeof EditorContextItemInput> {}

/** The bounded editor-context binding the renderer submits with an editor turn. */
export const EditorContextBinding = S.Struct({
  schema: S.Literal(EDITOR_CONTEXT_BINDING_SCHEMA_LITERAL),
  threadRef: TurnThreadRef,
  identity: EditorContextIdentity,
  items: S.Array(EditorContextItemInput).check(S.isMaxLength(512)),
  byteLimit: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(MAX_TURN_CONTEXT_CHARS),
  ),
})
export interface EditorContextBinding extends S.Schema.Type<typeof EditorContextBinding> {}

export const decodeEditorContextBinding = S.decodeUnknownSync(EditorContextBinding)

/**
 * Why the host refused an editor-context binding. Every reason keeps the turn
 * honest: the context is withheld, never silently substituted.
 */
export type EditorContextRefusalReason =
  | "no_active_editor"
  | "project_mismatch"
  | "root_mismatch"
  | "worktree_mismatch"
  | "stale_generation"

export interface EditorWorkContextBuilt {
  readonly ok: true
  readonly envelope: WorkContextEnvelope
  /** True when no item depends on a remote semantic index (local lexical/symbol only). */
  readonly noRemoteIndexDependency: boolean
}

export interface EditorWorkContextRefused {
  readonly ok: false
  readonly reason: EditorContextRefusalReason
}

export type EditorWorkContextResult = EditorWorkContextBuilt | EditorWorkContextRefused

const manifestRefFor = (binding: EditorContextBinding): ContextManifestRef =>
  ContextManifestRef.make(
    `context.editor.${binding.threadRef}.g${binding.identity.generation}`.slice(0, 256),
  )

/**
 * Build the effective editor `WorkContextEnvelope` for one turn, or refuse it.
 *
 * The host, not the renderer, owns this decision. A binding is admitted only
 * when its claimed identity matches the authoritative `expected` editor identity
 * for the current project, root, worktree, and generation. Any mismatch — a
 * different project/root/worktree, or a generation the host no longer holds — is
 * refused so the turn never carries context from another editor and a stale
 * candidate can never apply.
 */
export const buildEditorWorkContext = (
  binding: EditorContextBinding,
  expected: EditorContextIdentity | null,
  now: () => string = () => new Date().toISOString(),
): EditorWorkContextResult => {
  if (expected === null) return { ok: false, reason: "no_active_editor" }
  if (binding.identity.projectRef !== expected.projectRef) return { ok: false, reason: "project_mismatch" }
  if (binding.identity.rootRef !== expected.rootRef) return { ok: false, reason: "root_mismatch" }
  if (binding.identity.worktreeRef !== expected.worktreeRef) return { ok: false, reason: "worktree_mismatch" }
  if (binding.identity.generation !== expected.generation) return { ok: false, reason: "stale_generation" }

  const items = binding.items.map((item) =>
    ContextItem.make({
      kind: item.kind,
      itemRef: item.itemRef,
      derived: item.derived,
      byteLength: item.byteLength,
      truncated: item.truncated,
      redacted: item.redacted,
    }),
  )
  const totalByteLength = binding.items.reduce((sum, item) => sum + item.byteLength, 0)
  const truncated = binding.items.some((item) => item.truncated) || totalByteLength > binding.byteLimit
  const redacted = binding.items.some((item) => item.redacted)
  const noRemoteIndexDependency = binding.items.every((item) => item.kind !== "semantic_remote")

  const envelope = WorkContextEnvelope.make({
    schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
    manifestRef: manifestRefFor(binding),
    threadRef: binding.threadRef,
    projectRef: binding.identity.projectRef,
    worktreeRef: binding.identity.worktreeRef,
    generation: { state: "known", value: binding.identity.generation },
    createdAt: now(),
    items,
    totalByteLength,
    byteLimit: binding.byteLimit,
    truncated,
    redacted,
  })
  return { ok: true, envelope, noRemoteIndexDependency }
}

/**
 * The host-owned editor-context registry. Electron main sets the current editor
 * binding and the authoritative editor identity; the shared kernel's
 * `ContextSource` reads them for the bound thread. It is a small mutable seam so
 * the SAME `TurnService` carries editor context without a second authority path.
 */
export interface EditorContextRegistry {
  readonly get: (threadRef: string) => EditorContextBinding | null
  readonly set: (binding: EditorContextBinding) => void
  readonly clear: (threadRef: string) => void
  readonly expectation: () => EditorContextIdentity | null
  readonly setExpectation: (identity: EditorContextIdentity | null) => void
}

export const makeEditorContextRegistry = (): EditorContextRegistry => {
  const bindings = new Map<string, EditorContextBinding>()
  let expectation: EditorContextIdentity | null = null
  return {
    get: (threadRef) => bindings.get(threadRef) ?? null,
    set: (binding) => {
      bindings.set(binding.threadRef, binding)
    },
    clear: (threadRef) => {
      bindings.delete(threadRef)
    },
    expectation: () => expectation,
    setExpectation: (identity) => {
      expectation = identity
    },
  }
}
