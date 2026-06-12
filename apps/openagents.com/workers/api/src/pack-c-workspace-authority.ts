import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

export const PACK_C_WORKSPACE_AUTHORITY_VERSION =
  'pack-c-workspace-authority:v1' as const

const PACK_C_WORKSPACE_AUTHORITY_COLLECTION =
  'pack_c_workspace_authority_public'

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PACK_C_WORKSPACE_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /raw[-_ ]shell/i,
  /raw[-_ ]command/i,
  /raw[-_ ]prompt/i,
  /raw[-_ ]log/i,
  /private[-_ ]repo/i,
  /private[-_ ]content/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
]

class PackCWorkspaceAuthorityError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'PackCWorkspaceAuthorityError'
  }
}

export type PackCWorkspaceOperationKind =
  | 'file_edit'
  | 'file_read'
  | 'file_write'
  | 'shell_exec'
  | 'verification'

export type PackCWorkspaceRedactionClass =
  | 'customer'
  | 'operator'
  | 'private'
  | 'public'
  | 'team'

export type PackCWorkspaceAuthorityInput = Readonly<{
  allowedCommandIntentRefs: ReadonlyArray<string>
  allowedPathRefs: ReadonlyArray<string>
  approvalRefs?: ReadonlyArray<string> | undefined
  cancellationRef?: string | null | undefined
  commandIntentRef: string
  evidenceRef: string
  expectedSandboxProfileRef?: string | undefined
  generatedAt: string
  operationKind: PackCWorkspaceOperationKind
  redactionClass: PackCWorkspaceRedactionClass
  redactionReceiptRefs?: ReadonlyArray<string> | undefined
  requiresApproval: boolean
  sandboxProfileRef: string
  timeoutRef?: string | null | undefined
  touchedPathRefs?: ReadonlyArray<string> | undefined
  workspaceRef: string
}>

export type PackCWorkspaceAuthorityProjection = Readonly<{
  allowedCommandIntentRefs: ReadonlyArray<string>
  allowedPathRefs: ReadonlyArray<string>
  approvalRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  cancellationRef: string | null
  commandIntentRef: string
  evidenceRef: string
  expectedSandboxProfileRef: string | null
  generatedAt: string
  operationKind: PackCWorkspaceOperationKind
  redactionClass: PackCWorkspaceRedactionClass
  redactionReceiptRefs: ReadonlyArray<string>
  requiresApproval: boolean
  sandboxProfileRef: string
  status: 'allowed' | 'denied'
  timeoutRef: string | null
  touchedPathRefs: ReadonlyArray<string>
  workspaceAuthorityVersion: typeof PACK_C_WORKSPACE_AUTHORITY_VERSION
  workspaceRef: string
}>

const assertNoPrivateWorkspaceMaterial = (
  value: unknown,
  context: string,
): void => {
  assertNoProviderSecretMaterial(value, context)

  const text = typeof value === 'string' ? value : JSON.stringify(value)

  if (PACK_C_WORKSPACE_PRIVATE_MARKERS.some(marker => marker.test(text))) {
    throw new PackCWorkspaceAuthorityError(
      `${context} contains raw shell, private repo, local path, or prompt material.`,
    )
  }
}

const safeRef = (field: string, value: string): string => {
  const trimmed = value.trim()
  assertNoPrivateWorkspaceMaterial(trimmed, field)

  if (!SAFE_REF_PATTERN.test(trimmed)) {
    throw new PackCWorkspaceAuthorityError(
      `${field} must be a stable Pack C workspace ref.`,
    )
  }

  return trimmed
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const safeOptionalRef = (
  field: string,
  value: string | null | undefined,
): string | null =>
  value === null || value === undefined ? null : safeRef(field, value)

const scopeBlockers = (
  evidenceRef: string,
  allowedPathRefs: ReadonlyArray<string>,
  touchedPathRefs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const allowed = new Set(allowedPathRefs)

  return touchedPathRefs
    .filter(pathRef => !allowed.has(pathRef))
    .map(
      pathRef =>
        `pack-c-workspace-authority-blocker:${evidenceRef}:out-of-scope:${pathRef}`,
    )
}

const blockers = (
  input: PackCWorkspaceAuthorityInput,
  refs: Readonly<{
    allowedCommandIntentRefs: ReadonlyArray<string>
    allowedPathRefs: ReadonlyArray<string>
    approvalRefs: ReadonlyArray<string>
    commandIntentRef: string
    evidenceRef: string
    redactionReceiptRefs: ReadonlyArray<string>
    touchedPathRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...scopeBlockers(
    refs.evidenceRef,
    refs.allowedPathRefs,
    refs.touchedPathRefs,
  ),
  ...(input.requiresApproval && refs.approvalRefs.length === 0
    ? [
        `pack-c-workspace-authority-blocker:${refs.evidenceRef}:missing-approval`,
      ]
    : []),
  ...(refs.allowedCommandIntentRefs.includes(refs.commandIntentRef)
    ? []
    : [
        `pack-c-workspace-authority-blocker:${refs.evidenceRef}:command-not-allowed`,
      ]),
  ...(input.expectedSandboxProfileRef !== undefined &&
  input.expectedSandboxProfileRef.trim() !== input.sandboxProfileRef.trim()
    ? [
        `pack-c-workspace-authority-blocker:${refs.evidenceRef}:sandbox-mismatch`,
      ]
    : []),
  ...(input.timeoutRef === null || input.timeoutRef === undefined
    ? []
    : [`pack-c-workspace-authority-blocker:${refs.evidenceRef}:timeout`]),
  ...(input.cancellationRef === null || input.cancellationRef === undefined
    ? []
    : [`pack-c-workspace-authority-blocker:${refs.evidenceRef}:cancelled`]),
  ...(input.redactionClass === 'public' &&
  refs.redactionReceiptRefs.length === 0
    ? [
        `pack-c-workspace-authority-blocker:${refs.evidenceRef}:redaction-required`,
      ]
    : []),
]

export const projectPackCWorkspaceAuthority = (
  input: PackCWorkspaceAuthorityInput,
): PackCWorkspaceAuthorityProjection => {
  assertNoPrivateWorkspaceMaterial(input, 'pack-c-workspace-authority.input')

  const evidenceRef = safeRef(
    'pack-c-workspace-authority.evidenceRef',
    input.evidenceRef,
  )
  const allowedCommandIntentRefs = safeRefs(
    'pack-c-workspace-authority.allowedCommandIntentRefs',
    input.allowedCommandIntentRefs,
  )
  const allowedPathRefs = safeRefs(
    'pack-c-workspace-authority.allowedPathRefs',
    input.allowedPathRefs,
  )
  const approvalRefs = safeRefs(
    'pack-c-workspace-authority.approvalRefs',
    input.approvalRefs,
  )
  const commandIntentRef = safeRef(
    'pack-c-workspace-authority.commandIntentRef',
    input.commandIntentRef,
  )
  const redactionReceiptRefs = safeRefs(
    'pack-c-workspace-authority.redactionReceiptRefs',
    input.redactionReceiptRefs,
  )
  const touchedPathRefs = safeRefs(
    'pack-c-workspace-authority.touchedPathRefs',
    input.touchedPathRefs,
  )
  const blockerRefs = blockers(input, {
    allowedCommandIntentRefs,
    allowedPathRefs,
    approvalRefs,
    commandIntentRef,
    evidenceRef,
    redactionReceiptRefs,
    touchedPathRefs,
  })
  const projection: PackCWorkspaceAuthorityProjection = {
    allowedCommandIntentRefs,
    allowedPathRefs,
    approvalRefs,
    blockerRefs,
    cancellationRef: safeOptionalRef(
      'pack-c-workspace-authority.cancellationRef',
      input.cancellationRef,
    ),
    commandIntentRef,
    evidenceRef,
    expectedSandboxProfileRef:
      input.expectedSandboxProfileRef === undefined
        ? null
        : safeRef(
            'pack-c-workspace-authority.expectedSandboxProfileRef',
            input.expectedSandboxProfileRef,
          ),
    generatedAt: input.generatedAt,
    operationKind: input.operationKind,
    redactionClass: input.redactionClass,
    redactionReceiptRefs,
    requiresApproval: input.requiresApproval,
    sandboxProfileRef: safeRef(
      'pack-c-workspace-authority.sandboxProfileRef',
      input.sandboxProfileRef,
    ),
    status: blockerRefs.length === 0 ? 'allowed' : 'denied',
    timeoutRef: safeOptionalRef(
      'pack-c-workspace-authority.timeoutRef',
      input.timeoutRef,
    ),
    touchedPathRefs,
    workspaceAuthorityVersion: PACK_C_WORKSPACE_AUTHORITY_VERSION,
    workspaceRef: safeRef(
      'pack-c-workspace-authority.workspaceRef',
      input.workspaceRef,
    ),
  }

  assertNoPrivateWorkspaceMaterial(
    projection,
    PACK_C_WORKSPACE_AUTHORITY_COLLECTION,
  )

  return projection
}
