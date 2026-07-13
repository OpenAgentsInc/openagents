import { createHash } from "node:crypto"

import type {
  CapabilitySecretVault,
  CapabilityTargetAdapter,
  PortableCapabilityLease,
  PortableTargetClass,
  SecretMaterial,
} from "@openagentsinc/portable-session-contract"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const FORBIDDEN_RESULT =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|"(?:token|accessToken|authContent|authorization|password|secret|credential|material|path|hostname|processId|pid)"\s*:/iu

export type PortableGrantAuthorityKind = "provider" | "github"

export type PortableGrantAuthorityBinding = Readonly<{
  grantRef: string
  ownerUserId: string
  kind: PortableGrantAuthorityKind
  providerAccountRef?: string | undefined
  runnerSessionId?: string | undefined
}>

export class PortableCapabilityRuntimeAdapterError extends Error {
  readonly _tag = "PortableCapabilityRuntimeAdapterError"
  override readonly name = "PortableCapabilityRuntimeAdapterError"

  constructor(
    readonly code:
      | "invalid_scope"
      | "authority_unavailable"
      | "authority_refused"
      | "material_unavailable"
      | "target_refused"
      | "unsafe_result",
    message: string,
  ) {
    super(message)
  }
}

type Fetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

export type HttpPortableCapabilityGrantVaultConfig = Readonly<{
  baseUrl: string
  serviceBearer: string
  fetch?: Fetch | undefined
  bindings?: ReadonlyArray<PortableGrantAuthorityBinding> | undefined
}>

const assertRef = (value: string, field: string): string => {
  if (!SAFE_REF.test(value)) {
    throw new PortableCapabilityRuntimeAdapterError(
      "invalid_scope",
      `${field} is not a public-safe ref`,
    )
  }
  return value
}

const publicSafe = <A>(value: A): A => {
  if (FORBIDDEN_RESULT.test(JSON.stringify(value))) {
    throw new PortableCapabilityRuntimeAdapterError(
      "unsafe_result",
      "capability result contains forbidden private material",
    )
  }
  return value
}

const jsonRecord = async (response: Response): Promise<Record<string, unknown>> => {
  if (!response.ok) {
    throw new PortableCapabilityRuntimeAdapterError(
      "authority_refused",
      `capability authority refused with status ${response.status}`,
    )
  }
  const value: unknown = await response.json()
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new PortableCapabilityRuntimeAdapterError(
      "authority_refused",
      "capability authority returned an invalid envelope",
    )
  }
  return value as Record<string, unknown>
}

const nestedRecord = (
  value: unknown,
  field: string,
): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new PortableCapabilityRuntimeAdapterError(
      "authority_refused",
      `capability authority omitted ${field}`,
    )
  }
  return value as Record<string, unknown>
}

export class HttpPortableCapabilityGrantVault implements CapabilitySecretVault {
  private readonly fetch: Fetch
  private readonly bindings = new Map<string, PortableGrantAuthorityBinding>()

  constructor(private readonly config: HttpPortableCapabilityGrantVaultConfig) {
    if (!config.baseUrl.startsWith("https://") || config.serviceBearer.length < 8) {
      throw new PortableCapabilityRuntimeAdapterError(
        "invalid_scope",
        "capability authority configuration is invalid",
      )
    }
    this.fetch = config.fetch ?? globalThis.fetch
    for (const binding of config.bindings ?? []) this.register(binding)
  }

  register(binding: PortableGrantAuthorityBinding): void {
    assertRef(binding.grantRef, "grantRef")
    assertRef(binding.ownerUserId, "ownerUserId")
    if (binding.providerAccountRef !== undefined) {
      assertRef(binding.providerAccountRef, "providerAccountRef")
    }
    if (binding.runnerSessionId !== undefined) {
      assertRef(binding.runnerSessionId, "runnerSessionId")
    }
    const previous = this.bindings.get(binding.grantRef)
    if (previous !== undefined && JSON.stringify(previous) !== JSON.stringify(binding)) {
      throw new PortableCapabilityRuntimeAdapterError(
        "invalid_scope",
        "grant binding conflicts with existing scope",
      )
    }
    this.bindings.set(binding.grantRef, { ...binding })
  }

  async reissue(input: Readonly<{
    sourceGrantRef: string
    destinationGrantRef: string
    runnerSessionId?: string | undefined
    requestedAction?: string | undefined
  }>): Promise<PortableGrantAuthorityBinding> {
    const source = this.requireBinding(input.sourceGrantRef)
    assertRef(input.destinationGrantRef, "destinationGrantRef")
    if (input.destinationGrantRef === source.grantRef) {
      throw new PortableCapabilityRuntimeAdapterError(
        "invalid_scope",
        "destination grant must be distinct",
      )
    }
    const runnerSessionId = input.runnerSessionId ?? source.runnerSessionId
    const envelope = await this.post(
      `/api/portable-capability-grants/${source.kind}/reissue`,
      {
        ownerUserId: source.ownerUserId,
        sourceGrantRef: source.grantRef,
        destinationGrantRef: input.destinationGrantRef,
        ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
        ...(input.requestedAction === undefined
          ? {}
          : { requestedAction: input.requestedAction }),
      },
    )
    const grant = nestedRecord(envelope.grant, "grant")
    if (
      grant.grantRef !== input.destinationGrantRef ||
      grant.status !== "issued" ||
      envelope.material !== "excluded"
    ) {
      throw new PortableCapabilityRuntimeAdapterError(
        "authority_refused",
        "capability authority returned a mismatched reissue",
      )
    }
    const binding = {
      ...source,
      grantRef: input.destinationGrantRef,
      ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
    }
    this.register(binding)
    return binding
  }

  async withSourceGrantMaterial<A>(input: {
    readonly sourceGrantRef: string
    readonly leaseRef: string
    readonly use: (material: SecretMaterial) => Promise<A>
  }): Promise<A> {
    assertRef(input.leaseRef, "leaseRef")
    const binding = this.requireBinding(input.sourceGrantRef)
    const material = await this.resolveMaterial(binding)
    try {
      return await input.use(material as SecretMaterial)
    } finally {
      material.fill(0)
    }
  }

  async revokeSourceGrant(input: {
    readonly sourceGrantRef: string
    readonly leaseRef: string
  }): Promise<void> {
    assertRef(input.leaseRef, "leaseRef")
    const binding = this.requireBinding(input.sourceGrantRef)
    const envelope = await this.post(
      `/api/portable-capability-grants/${binding.kind}/revoke`,
      { ownerUserId: binding.ownerUserId, grantRef: binding.grantRef },
    )
    const grant = nestedRecord(envelope.grant, "grant")
    if (
      grant.grantRef !== binding.grantRef ||
      grant.status !== "revoked" ||
      envelope.material !== "excluded"
    ) {
      throw new PortableCapabilityRuntimeAdapterError(
        "authority_refused",
        "capability authority returned a mismatched revocation",
      )
    }
  }

  private requireBinding(grantRef: string): PortableGrantAuthorityBinding {
    const binding = this.bindings.get(assertRef(grantRef, "grantRef"))
    if (binding === undefined) {
      throw new PortableCapabilityRuntimeAdapterError(
        "invalid_scope",
        "grant ref is not registered in this runtime",
      )
    }
    return binding
  }

  private async resolveMaterial(
    binding: PortableGrantAuthorityBinding,
  ): Promise<Uint8Array> {
    const path =
      binding.kind === "provider"
        ? "/api/provider-accounts/chatgpt-codex/grants/resolve"
        : "/api/github-write/grants/resolve"
    const envelope = await this.post(path, {
      grantRef: binding.grantRef,
      ...(binding.providerAccountRef === undefined
        ? {}
        : { providerAccountRef: binding.providerAccountRef }),
      ...(binding.runnerSessionId === undefined
        ? {}
        : { runnerSessionId: binding.runnerSessionId }),
      ...(binding.kind === "provider" ? { includeAuthMaterial: true } : {}),
    })
    let text: unknown
    if (binding.kind === "provider") {
      const grant = nestedRecord(envelope.grant, "grant")
      if (grant.grantRef !== binding.grantRef) {
        throw new PortableCapabilityRuntimeAdapterError(
          "authority_refused",
          "provider resolver returned a mismatched grant",
        )
      }
      text = nestedRecord(envelope.authMaterial, "authMaterial").authContentJson
    } else {
      const grant = nestedRecord(envelope.grant, "grant")
      if (grant.grantRef !== binding.grantRef) {
        throw new PortableCapabilityRuntimeAdapterError(
          "authority_refused",
          "GitHub resolver returned a mismatched grant",
        )
      }
      text = nestedRecord(grant.credential, "credential").accessToken
    }
    if (typeof text !== "string" || text.length === 0) {
      throw new PortableCapabilityRuntimeAdapterError(
        "material_unavailable",
        "capability material is unavailable",
      )
    }
    return new TextEncoder().encode(text)
  }

  private async post(
    path: string,
    body: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    let response: Response
    try {
      response = await this.fetch(new URL(path, this.config.baseUrl), {
        method: "POST",
        headers: {
          authorization: `Bearer ${this.config.serviceBearer}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
      })
    } catch {
      throw new PortableCapabilityRuntimeAdapterError(
        "authority_unavailable",
        "capability authority is unavailable",
      )
    }
    return jsonRecord(response)
  }
}

export type PortableCapabilityTargetInstallationPort = Readonly<{
  install: (input: Readonly<{
    lease: PortableCapabilityLease
    permissions: ReadonlyArray<string>
    material: SecretMaterial
    managedMarkerPath?: string | undefined
  }>) => Promise<Readonly<{
    installationRef: string
    evidenceRef: string
    marker?: Readonly<{ leaseRef: string; evidenceRef: string }> | undefined
  }>>
  wipe: (input: Readonly<{
    leaseRef: string
    targetRef: string
    attachmentRef: string
    attachmentGeneration: number
    installationRef?: string | undefined
  }>) => Promise<Readonly<{ wipeReceiptRef: string }>>
}>

export const managedCapabilityMarkerPath = (
  sessionRef: string,
  leaseRef: string,
): string => {
  assertRef(sessionRef, "sessionRef")
  assertRef(leaseRef, "leaseRef")
  const session = createHash("sha256").update(sessionRef).digest("hex").slice(0, 24)
  const lease = createHash("sha256").update(leaseRef).digest("hex").slice(0, 24)
  return `/var/lib/openagents/portable-sessions/${session}/capabilities/${lease}.installed.json`
}

export const makePortableCapabilityTargetAdapter = (input: Readonly<{
  adapterRef: string
  targetClass: PortableTargetClass
  port: PortableCapabilityTargetInstallationPort
}>): CapabilityTargetAdapter => {
  assertRef(input.adapterRef, "adapterRef")
  return {
    adapterRef: input.adapterRef,
    targetClass: input.targetClass,
    redeem: async request => {
      publicSafe({ lease: request.lease, permissions: request.permissions })
      const managedMarkerPathValue =
        input.targetClass === "openagents_managed"
          ? managedCapabilityMarkerPath(
              request.lease.sessionRef,
              request.lease.leaseRef,
            )
          : undefined
      let installed
      try {
        installed = await input.port.install({
          lease: request.lease,
          permissions: request.permissions,
          material: request.material,
          ...(managedMarkerPathValue === undefined
            ? {}
            : { managedMarkerPath: managedMarkerPathValue }),
        })
      } catch {
        throw new PortableCapabilityRuntimeAdapterError(
          "target_refused",
          "capability target installation failed closed",
        )
      }
      assertRef(installed.installationRef, "installationRef")
      assertRef(installed.evidenceRef, "evidenceRef")
      if (
        input.targetClass === "openagents_managed" &&
        (installed.marker?.leaseRef !== request.lease.leaseRef ||
          installed.marker.evidenceRef !== installed.evidenceRef)
      ) {
        throw new PortableCapabilityRuntimeAdapterError(
          "target_refused",
          "managed capability marker does not match the installed lease",
        )
      }
      return publicSafe({ installationRef: installed.installationRef })
    },
    wipe: async request => {
      publicSafe(request)
      try {
        const receipt = publicSafe(await input.port.wipe(request))
        assertRef(receipt.wipeReceiptRef, "wipeReceiptRef")
        return receipt
      } catch {
        throw new PortableCapabilityRuntimeAdapterError(
          "target_refused",
          "capability target wipe failed closed",
        )
      }
    },
  }
}
