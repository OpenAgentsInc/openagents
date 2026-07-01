export type KhalaCodeDesktopJsonRpcId = number | string

export type KhalaCodeDesktopCodexApprovalMethod =
  | "item/commandExecution/requestApproval"
  | "item/fileChange/requestApproval"
  | "item/permissions/requestApproval"

export type KhalaCodeDesktopCodexApprovalAction =
  | "accept"
  | "acceptForSession"
  | "acceptWithExecpolicyAmendment"
  | "applyNetworkPolicyAmendment"
  | "cancel"
  | "decline"
  | "grantPermissions"
  | "grantPermissionsForSession"
  | "grantPermissionsWithStrictReview"

export type KhalaCodeDesktopCodexNetworkPolicyAmendment = {
  readonly action: "allow" | "deny" | string
  readonly host: string
}

export type KhalaCodeDesktopCodexPermissionProfile = {
  readonly fileSystem?: {
    readonly entries?: readonly unknown[]
    readonly globScanMaxDepth?: number
    readonly read?: readonly string[] | null
    readonly write?: readonly string[] | null
  }
  readonly network?: {
    readonly enabled?: boolean | null
  }
}

export type KhalaCodeDesktopCodexApprovalResponseInput = {
  readonly action: KhalaCodeDesktopCodexApprovalAction
  readonly execpolicyAmendment?: readonly string[]
  readonly method: KhalaCodeDesktopCodexApprovalMethod
  readonly networkPolicyAmendment?: KhalaCodeDesktopCodexNetworkPolicyAmendment
  readonly permissions?: KhalaCodeDesktopCodexPermissionProfile
}

export type KhalaCodeDesktopCodexApprovalProjection = {
  readonly additionalPermissions?: unknown
  readonly availableDecisions?: readonly unknown[]
  readonly command?: string
  readonly cwd?: string
  readonly grantRoot?: string
  readonly method: KhalaCodeDesktopCodexApprovalMethod
  readonly networkApprovalContext?: unknown
  readonly permissions?: KhalaCodeDesktopCodexPermissionProfile
  readonly proposedExecpolicyAmendment?: readonly string[]
  readonly proposedNetworkPolicyAmendments?: readonly KhalaCodeDesktopCodexNetworkPolicyAmendment[]
  readonly reason?: string
  readonly requestId: KhalaCodeDesktopJsonRpcId
}

export const isKhalaCodeDesktopCodexApprovalMethod = (
  method: string,
): method is KhalaCodeDesktopCodexApprovalMethod =>
  method === "item/commandExecution/requestApproval" ||
  method === "item/fileChange/requestApproval" ||
  method === "item/permissions/requestApproval"

const emptyPermissions = (): KhalaCodeDesktopCodexPermissionProfile => ({})

const permissionResponse = (
  input: KhalaCodeDesktopCodexApprovalResponseInput,
): unknown => {
  switch (input.action) {
    case "accept":
    case "grantPermissions":
      return {
        permissions: input.permissions ?? emptyPermissions(),
        scope: "turn",
      }
    case "acceptForSession":
    case "grantPermissionsForSession":
      return {
        permissions: input.permissions ?? emptyPermissions(),
        scope: "session",
      }
    case "grantPermissionsWithStrictReview":
      return {
        permissions: input.permissions ?? emptyPermissions(),
        scope: "turn",
        strictAutoReview: true,
      }
    case "decline":
    case "cancel":
      return {
        permissions: emptyPermissions(),
        scope: "turn",
      }
    default:
      throw new Error(`${input.action} is not valid for Codex permission approval.`)
  }
}

export const khalaCodeDesktopCodexApprovalResponsePayload = (
  input: KhalaCodeDesktopCodexApprovalResponseInput,
): unknown => {
  switch (input.method) {
    case "item/commandExecution/requestApproval":
      switch (input.action) {
        case "accept":
        case "acceptForSession":
        case "decline":
        case "cancel":
          return { decision: input.action }
        case "acceptWithExecpolicyAmendment":
          if (input.execpolicyAmendment === undefined) {
            throw new Error("acceptWithExecpolicyAmendment requires an execpolicy amendment.")
          }
          return {
            decision: {
              acceptWithExecpolicyAmendment: {
                execpolicy_amendment: [...input.execpolicyAmendment],
              },
            },
          }
        case "applyNetworkPolicyAmendment":
          if (input.networkPolicyAmendment === undefined) {
            throw new Error("applyNetworkPolicyAmendment requires a network policy amendment.")
          }
          return {
            decision: {
              applyNetworkPolicyAmendment: {
                network_policy_amendment: input.networkPolicyAmendment,
              },
            },
          }
        default:
          throw new Error(`${input.action} is not valid for Codex command approval.`)
      }
    case "item/fileChange/requestApproval":
      switch (input.action) {
        case "accept":
        case "acceptForSession":
        case "decline":
        case "cancel":
          return { decision: input.action }
        default:
          throw new Error(`${input.action} is not valid for Codex file-change approval.`)
      }
    case "item/permissions/requestApproval":
      return permissionResponse(input)
  }
}
