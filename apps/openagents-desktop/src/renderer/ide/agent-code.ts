import {
  IdeAgentAttachmentRefSchema,
  IdeAgentAttachmentSchema,
  IdeAgentContextItemRefSchema,
  IdeAgentContextItemSchema,
  IdeAgentContextManifestSchema,
  IdeAgentManifestRefSchema,
  IdeAgentTurnRefSchema,
  decodeIdeAgentCodeCommandResult,
  decodeIdeAgentCodeSnapshot,
  emptyIdeAgentCodeSnapshot,
  projectDocumentGenerationForSource,
  type IdeAgentCodeCommand,
  type IdeAgentCodeCommandResult,
  type IdeAgentCodeSnapshot,
  type IdeAgentContextManifest,
} from "../../ide/agent-code-contract.ts"
import {
  IdeAttachmentGenerationSchema,
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  IdeGitSnapshotGenerationSchema,
  IdeGitSnapshotRefSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "../../ide/project-contract.ts"
import { sha256 } from "@noble/hashes/sha256"
import type { DesktopShellState } from "../shell.ts"

export type IdeAgentCodeRendererHost = Readonly<{
  snapshot: () => Promise<unknown>
  command: (command: IdeAgentCodeCommand) => Promise<unknown>
}>

export const unavailableIdeAgentCodeRendererHost: IdeAgentCodeRendererHost = {
  snapshot: async () => emptyIdeAgentCodeSnapshot(),
  command: async () => ({
    _tag: "Refused",
    reason: "unavailable",
    message: "Agent-code services are unavailable.",
    snapshot: emptyIdeAgentCodeSnapshot(),
  }),
}

const hex = (bytes: Uint8Array): string =>
  [...bytes].map(byte => byte.toString(16).padStart(2, "0")).join("")

// Custom Electron schemes cannot assume WebCrypto will settle. Noble's
// audited pure TypeScript SHA-256 keeps these renderer-side opaque identities
// byte-for-byte aligned with main's node:crypto recipe. Main still recomputes
// all authoritative content/revision digests before apply.
const sha256Hex = async (value: string): Promise<string> =>
  hex(sha256(new TextEncoder().encode(value)))

const safeRefSuffix = async (value: string): Promise<string> => (await sha256Hex(value)).slice(0, 32)
const contextByteBudget = 200_000
const contextTokenBudget = 50_000
const contextExcerptCharacters = 64_000

const boundedContextExcerpt = (value: string): Readonly<{ value: string; bytes: number; tokens: number; truncated: boolean }> => {
  const chunks: string[] = []
  let bytes = 0
  let characters = 0
  for (const character of value) {
    const characterBytes = new TextEncoder().encode(character).byteLength
    if (characters + character.length > contextExcerptCharacters || bytes + characterBytes > contextByteBudget) break
    chunks.push(character)
    characters += character.length
    bytes += characterBytes
  }
  const bounded = chunks.join("")
  return { value: bounded, bytes, tokens: Math.ceil(bounded.length / 4), truncated: bounded.length < value.length }
}

const selectedSession = (state: DesktopShellState) =>
  state.codingCatalog.sessions.find(session => session.sessionRef === state.codingCatalog.selectedSessionRef) ??
  state.codingCatalog.sessions.find(session => session.state === "active") ??
  state.codingCatalog.sessions[0] ?? null

const sameUnderlyingAttachment = (
  state: DesktopShellState,
  identity: Readonly<{ project: string; root: string; worktree: string; session: string; grant: string }>,
): boolean => {
  const current = state.agentCode.attachment
  if (current === null) return false
  return current.projectRef.endsWith(identity.project) && current.rootRef.endsWith(identity.root) &&
    current.worktreeRef.endsWith(identity.worktree) && current.sessionRef.endsWith(identity.session) &&
    current.grantRef === identity.grant
}

const runtimeAccountLabel = (state: DesktopShellState): string => {
  const thread = state.activeThreadId === null
    ? null
    : state.threads.find(candidate => candidate.id === state.activeThreadId) ?? null
  const observed = thread === null
    ? undefined
    : [...thread.notes].reverse().find(note =>
        note.meta?.accountRef !== undefined &&
        (note.meta.lane === undefined || note.meta.lane === state.activeLaneRef))
  // A completed local-harness turn persists the account that actually ran in
  // main-owned message metadata. That observed identity outranks a selected
  // target: implicit Claude selection can legally choose the current session
  // or a fallback Pylon home, and calling it `account.current` would make the
  // exact IDE-09 provider admission impossible (or tempt a false guess).
  if (observed?.meta?.accountRef !== undefined) return observed.meta.accountRef.slice(0, 240)
  const target = state.activeThreadId === null ? null : state.providerTargetsByThread[state.activeThreadId]
  if (target === null || target === undefined) return "account.current"
  return target.accountRef.slice(0, 240)
}

export const assembleActiveFileAgentManifest = async (
  state: DesktopShellState,
  observedAt: string,
): Promise<Readonly<{
  attachment: ReturnType<typeof IdeAgentAttachmentSchema.make>
  manifest: IdeAgentContextManifest
}> | null> => {
  const file = state.composerFileContext
  const grantRef = state.workspaceBrowser.grantRef
  if (file === null || grantRef === null) return null
  const session = selectedSession(state)
  const identity = {
    project: await safeRefSuffix(session?.projectRef ?? grantRef),
    root: await safeRefSuffix(session?.repositoryRef ?? grantRef),
    worktree: await safeRefSuffix(session?.worktreeRef ?? grantRef),
    session: await safeRefSuffix(session?.sessionRef ?? state.activeThreadId ?? grantRef),
    grant: grantRef,
  }
  const attachmentGeneration = sameUnderlyingAttachment(state, identity)
    ? state.agentCode.attachment!.attachmentGeneration
    : IdeAttachmentGenerationSchema.make((state.agentCode.attachment?.attachmentGeneration ?? 0) + 1)
  const attachmentKey = `${identity.project}.${identity.worktree}.${identity.session}.${attachmentGeneration}`
  const attachment = IdeAgentAttachmentSchema.make({
    schemaVersion: "openagents.desktop.ide-agent-code.v1",
    agentAttachmentRef: IdeAgentAttachmentRefSchema.make(`ide.agent-attachment.${await safeRefSuffix(attachmentKey)}`),
    projectRef: IdeProjectRefSchema.make(`ide.project.${identity.project}`),
    rootRef: IdeRootRefSchema.make(`ide.root.${identity.root}`),
    worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.${identity.worktree}`),
    sessionRef: IdeSessionRefSchema.make(`ide.session.${identity.session}`),
    attachmentGeneration,
    placementGeneration: IdePlacementGenerationSchema.make(1),
    grantRef,
    attachedAt: IdeTimestampSchema.make(observedAt),
    expiresAt: null,
  })
  const activeTab = state.workspaceEditor.tabs.find(tab => tab.pathRef === file.path) ?? null
  const sourceGeneration = activeTab?.generation ?? 0
  // Monaco numbers the first model incarnation from zero, while the
  // project/workspace authority deliberately uses one-based generations.
  // Preserve Monaco's exact source generation for disclosure and translate
  // it once at this boundary for the authority-facing document identity.
  const documentGeneration = projectDocumentGenerationForSource(sourceGeneration)
  // These opaque identities intentionally use the same path-derived recipe as
  // the main-owned workspace authority. The renderer never resolves them to a
  // host path and main still re-reads every revision before apply.
  const pathSuffix = await safeRefSuffix(file.path)
  const fileRef = IdeFileRefSchema.make(`ide.file.workspace.${pathSuffix}`)
  const documentRef = IdeDocumentRefSchema.make(`ide.document.workspace.${pathSuffix}`)
  const diskRevisionRef = IdeDiskRevisionRefSchema.make(`ide.disk-revision.workspace.${await safeRefSuffix(file.revisionRef)}`)
  const boundedFile = boundedContextExcerpt(file.content)
  const fileItem = IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:${file.path}:${file.revisionRef}`)}`),
    source: {
      _tag: "File",
      selectedBy: "user",
      sourceGeneration,
      fileRef,
      documentRef,
      pathRef: file.path,
      documentGeneration,
      diskRevisionRef,
    },
    disposition: { _tag: "Included", reason: "explicit_user_selection" },
    destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
    freshness: "current",
    sensitivity: "workspace",
    retention: "turn_only",
    byteEstimate: boundedFile.bytes,
    tokenEstimate: boundedFile.tokens,
    truncated: boundedFile.truncated,
    label: file.path,
    excerpt: boundedFile.value,
  })
  const supportingItems: Array<ReturnType<typeof IdeAgentContextItemSchema.make>> = []
  const unavailable = async (
    sourceClass: "active_selection" | "diagnostics" | "symbols" | "git_cochange" | "rule" | "skill" | "recent_edit" | "lexical_retrieval" | "runtime_policy",
    selectedBy: "user" | "editor" | "diagnostics" | "git" | "rule_engine" | "skill" | "retrieval" | "runtime",
    label: string,
    detail: string,
  ) => IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:${sourceClass}:${detail}`)}`),
    source: { _tag: "Unavailable", selectedBy, sourceGeneration: 1, sourceClass, detail },
    disposition: { _tag: "Omitted", reason: "unavailable", detail },
    destination: { _tag: "Withheld", reason: detail },
    freshness: "unavailable",
    sensitivity: "workspace",
    retention: "withheld",
    byteEstimate: 0,
    tokenEstimate: 0,
    truncated: false,
    label,
    excerpt: null,
  })
  const positionAt = (offset: number): Readonly<{ line: number; column: number }> => {
    const bounded = Math.max(0, Math.min(file.content.length, Math.trunc(offset)))
    let line = 1
    let lineStart = 0
    for (let index = 0; index < bounded; index += 1) {
      if (file.content.charCodeAt(index) !== 10) continue
      line += 1
      lineStart = index + 1
    }
    return { line, column: bounded - lineStart + 1 }
  }
  const selection = activeTab?.selection ?? { start: 0, end: 0 }
  if (selection.end > selection.start) {
    const selectedExcerpt = file.content.slice(selection.start, selection.end)
    const excerpt = boundedContextExcerpt(selectedExcerpt)
    supportingItems.push(IdeAgentContextItemSchema.make({
      contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:selection:${selection.start}:${selection.end}`)}`),
      source: {
        _tag: "Range", selectedBy: "user", sourceGeneration,
        fileRef, documentRef, pathRef: file.path, documentGeneration,
        range: { start: positionAt(selection.start), end: positionAt(selection.end) },
      },
      disposition: { _tag: "Included", reason: "active_selection" },
      destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
      freshness: "current", sensitivity: "workspace", retention: "turn_only",
      byteEstimate: excerpt.bytes, tokenEstimate: excerpt.tokens, truncated: excerpt.truncated,
      label: `Selection in ${file.path}`, excerpt: excerpt.value,
    }))
  } else {
    supportingItems.push(await unavailable("active_selection", "user", "Active selection", "No non-empty editor selection was attached."))
  }

  const languageItems = state.workspaceEditor.language.results
    .filter(result => activeTab !== null && result.documentRef === activeTab.documentRef &&
      result.documentGeneration === activeTab.generation && !["Stale", "Cancelled"].includes(result.state._tag))
    .flatMap(result => result.items)
  const diagnostic = languageItems.find(item => item._tag === "Diagnostic")
  if (diagnostic?._tag === "Diagnostic") {
    const excerpt = diagnostic.message.slice(0, 2_000)
    supportingItems.push(IdeAgentContextItemSchema.make({
      contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:diagnostic:${diagnostic.diagnosticRef}`)}`),
      source: {
        _tag: "Diagnostic", selectedBy: "diagnostics", sourceGeneration,
        diagnosticRef: diagnostic.diagnosticRef, fileRef, documentRef, documentGeneration,
      },
      disposition: { _tag: "Included", reason: "diagnostic_cause" },
      destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
      freshness: "current", sensitivity: "workspace", retention: "turn_only",
      byteEstimate: new TextEncoder().encode(excerpt).byteLength, tokenEstimate: Math.ceil(excerpt.length / 4),
      truncated: diagnostic.message.length > excerpt.length, label: `Diagnostic: ${diagnostic.severity}`, excerpt,
    }))
  } else {
    supportingItems.push(await unavailable("diagnostics", "diagnostics", "Project diagnostics", "No current diagnostic result matched this document generation."))
  }
  const symbol = languageItems.find(item => item._tag === "Symbol")
  if (symbol?._tag === "Symbol") {
    supportingItems.push(IdeAgentContextItemSchema.make({
      contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:symbol:${symbol.symbolRef}`)}`),
      source: {
        _tag: "Symbol", selectedBy: "editor", sourceGeneration,
        symbolRef: symbol.symbolRef, fileRef, documentRef, documentGeneration,
      },
      disposition: { _tag: "Included", reason: "symbol_match" },
      destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
      freshness: "current", sensitivity: "workspace", retention: "turn_only",
      byteEstimate: new TextEncoder().encode(symbol.name).byteLength, tokenEstimate: Math.ceil(symbol.name.length / 4),
      truncated: false, label: `Symbol: ${symbol.name}`, excerpt: symbol.name,
    }))
  } else {
    supportingItems.push(await unavailable("symbols", "editor", "Project symbols", "No current symbol result matched this document generation."))
  }

  const gitEntry = state.git.status === null ? null : [
    ...state.git.status.staged,
    ...state.git.status.unstaged,
    ...state.git.status.untracked,
  ].find(entry => entry.path === file.path) ?? null
  if (gitEntry !== null && state.git.status !== null) {
    supportingItems.push(IdeAgentContextItemSchema.make({
      contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:git:${state.git.status.statusRef}:${file.path}`)}`),
      source: {
        _tag: "GitChange", selectedBy: "git", sourceGeneration: 1, fileRef, pathRef: file.path,
        gitSnapshotRef: IdeGitSnapshotRefSchema.make(`ide.git-snapshot.${await safeRefSuffix(state.git.status.statusRef)}`),
        gitSnapshotGeneration: IdeGitSnapshotGenerationSchema.make(1),
      },
      disposition: { _tag: "Included", reason: "git_cochange" },
      destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
      freshness: "current", sensitivity: "workspace", retention: "turn_only",
      byteEstimate: 0, tokenEstimate: 0, truncated: false,
      label: `Git ${gitEntry.status}: ${file.path}`, excerpt: null,
    }))
  } else {
    supportingItems.push(await unavailable("git_cochange", "git", "Git/co-change facts", "No current Git change matched the attached file."))
  }

  supportingItems.push(
    await unavailable("rule", "rule_engine", "Project rules", "No additional project rule was attached to this turn."),
    await unavailable("skill", "skill", "Invoked skills", "No explicit skill invocation was attached to this turn."),
  )
  if (file.dirty) {
    supportingItems.push(IdeAgentContextItemSchema.make({
      contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:recent-edit:${file.revisionRef}`)}`),
      source: { _tag: "RecentEdit", selectedBy: "editor", sourceGeneration, fileRef, documentRef, documentGeneration },
      disposition: { _tag: "Included", reason: "recent_edit" },
      destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
      freshness: "current", sensitivity: "workspace", retention: "turn_only", byteEstimate: 0, tokenEstimate: 0,
      truncated: false, label: "Unsaved recent edit metadata", excerpt: null,
    }))
  } else {
    supportingItems.push(await unavailable("recent_edit", "editor", "Recent edits", "The attached file has no unsaved recent edit."))
  }
  supportingItems.push(IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:lexical:${file.path}`)}`),
    source: {
      _tag: "LexicalRetrieval", selectedBy: "retrieval", sourceGeneration: 1,
      resultRef: `lexical.path.${pathSuffix}`, queryDigest: `sha256:${await sha256Hex(file.path)}`,
    },
    disposition: { _tag: "Included", reason: "lexical_match" },
    destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
    freshness: "current", sensitivity: "workspace", retention: "turn_only", byteEstimate: 0, tokenEstimate: 0,
    truncated: false, label: "Exact path lexical match", excerpt: null,
  }))
  const semanticItem = IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:semantic-disabled`)}`),
    source: {
      _tag: "SemanticRetrieval",
      selectedBy: "retrieval",
      sourceGeneration: 1,
      resultRef: "semantic.retrieval.disabled",
      queryDigest: `sha256:${await sha256Hex("semantic retrieval disabled")}`,
    },
    disposition: {
      _tag: "Omitted",
      reason: "retrieval_disabled",
      detail: "Semantic retrieval is off; explicit file, path, lexical, language, and Git facts remain available.",
    },
    destination: { _tag: "Withheld", reason: "semantic retrieval disabled" },
    freshness: "unavailable",
    sensitivity: "workspace",
    retention: "withheld",
    byteEstimate: 0,
    tokenEstimate: 0,
    truncated: false,
    label: "Optional semantic retrieval",
    excerpt: null,
  })
  supportingItems.push(semanticItem, IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make(`ide.agent-context-item.${await safeRefSuffix(`${attachmentKey}:runtime-policy`)}`),
    source: { _tag: "RuntimePolicy", selectedBy: "runtime", sourceGeneration: 1, policyRef: "runtime.effective-turn-policy" },
    disposition: { _tag: "Included", reason: "runtime_policy" },
    destination: { _tag: "HarnessPrompt", harnessRef: state.selectedHarness },
    freshness: "current", sensitivity: "public", retention: "turn_only", byteEstimate: 0, tokenEstimate: 0,
    truncated: false, label: "Effective runtime/tool policy", excerpt: null,
  }))
  const turnKey = state.activeThreadId ?? `new:${attachmentKey}`
  const turnRef = IdeAgentTurnRefSchema.make(`ide.agent-turn.${await safeRefSuffix(turnKey)}`)
  let admittedBytes = 0
  let admittedTokens = 0
  const items = [fileItem, ...supportingItems].map(item => {
    if (item.disposition._tag !== "Included") return item
    if (admittedBytes + item.byteEstimate <= contextByteBudget && admittedTokens + item.tokenEstimate <= contextTokenBudget) {
      admittedBytes += item.byteEstimate
      admittedTokens += item.tokenEstimate
      return item
    }
    return IdeAgentContextItemSchema.make({
      ...item,
      disposition: { _tag: "Omitted", reason: "over_budget", detail: "The item exceeded the remaining disclosed context budget." },
      destination: { _tag: "Withheld", reason: "context budget exhausted" },
      retention: "withheld",
      byteEstimate: 0,
      tokenEstimate: 0,
      truncated: true,
      excerpt: null,
    })
  })
  const includedBytes = items.filter(item => item.disposition._tag === "Included")
    .reduce((total, item) => total + item.byteEstimate, 0)
  const includedTokens = items.filter(item => item.disposition._tag === "Included")
    .reduce((total, item) => total + item.tokenEstimate, 0)
  const omittedCount = items.filter(item => item.disposition._tag === "Omitted").length
  const manifest = IdeAgentContextManifestSchema.make({
    schemaVersion: "openagents.desktop.ide-agent-code.v1",
    manifestRef: IdeAgentManifestRefSchema.make(`ide.agent-manifest.${await safeRefSuffix(`${attachmentKey}:${file.path}:${observedAt}`)}`),
    attachment,
    turnRef,
    conversationThreadRef: state.activeThreadId,
    createdAt: IdeTimestampSchema.make(observedAt),
    effectiveRuntime: {
      harnessRef: state.selectedHarness,
      modelRef: state.selectedHarness === "codex" ? state.codexModel : state.claudeModel,
      providerRef: state.activeLaneRef,
      accountRef: runtimeAccountLabel(state),
      placementRef: IdePlacementRefSchema.make("ide.placement.desktop-local"),
      placementGeneration: IdePlacementGenerationSchema.make(1),
      toolPolicyRef: "tools.proposal-only",
      permissionMode: "proposal_only",
      sandboxRef: "sandbox.desktop-harness",
      memoryPolicyRef: "memory.turn-only",
      instructionPolicyRef: "instructions.effective-thread",
      semanticRetrieval: "disabled",
    },
    items,
    includedBytes,
    includedTokens,
    omittedCount,
    byteBudget: contextByteBudget,
    tokenBudget: contextTokenBudget,
    exportable: true,
    rebuildable: true,
    deletionPolicyRef: "retention.turn-only",
  })
  return { attachment, manifest }
}

export const executeAgentCodeRendererCommand = async (
  host: IdeAgentCodeRendererHost,
  command: IdeAgentCodeCommand,
): Promise<IdeAgentCodeCommandResult> => decodeIdeAgentCodeCommandResult(await host.command(command)) ?? {
  _tag: "Refused",
  reason: "unavailable",
  message: "The agent-code host returned an invalid response.",
  snapshot: emptyIdeAgentCodeSnapshot(),
}

export const loadAgentCodeRendererSnapshot = async (
  host: IdeAgentCodeRendererHost,
): Promise<IdeAgentCodeSnapshot> => decodeIdeAgentCodeSnapshot(await host.snapshot()) ?? emptyIdeAgentCodeSnapshot()
