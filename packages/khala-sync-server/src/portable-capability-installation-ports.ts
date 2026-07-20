import { createHash, randomUUID, timingSafeEqual } from "node:crypto"
import {
  chmod,
  mkdir,
  open,
  readFile,
  rename,
  rm,
  writeFile,
} from "node:fs/promises"
import type { FileHandle } from "node:fs/promises"
import { isAbsolute, join, resolve } from "node:path"

import type {
  PortableCapabilityLease,
  SecretMaterial,
} from "@openagentsinc/portable-session-contract"

import {
  managedCapabilityMarkerPath,
  type PortableCapabilityTargetInstallationPort,
} from "./portable-capability-runtime-adapters.js"
import type { SyncSql } from "./sql.js"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "::1"])
const MARKER_FILE = "installed.json"
const MATERIAL_FILE = "material.bin"
const FORBIDDEN_RESPONSE =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|"(?:token|accessToken|authContent|authorization|password|secret|credential|path|hostname|processId|pid)"\s*:/iu
const CAPABILITY_KINDS = new Set(["provider", "scm_read", "scm_write", "tool", "api"])

type Fetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

export class PortableCapabilityInstallationError extends Error {
  readonly _tag = "PortableCapabilityInstallationError"
  override readonly name = "PortableCapabilityInstallationError"

  constructor(
    readonly code:
      | "invalid_scope"
      | "installation_busy"
      | "installation_conflict"
      | "material_unavailable"
      | "target_unavailable"
      | "target_refused"
      | "unsafe_response",
    message: string,
  ) {
    super(message)
  }
}

const assertRef = (value: string, field: string): string => {
  if (!SAFE_REF.test(value)) {
    throw new PortableCapabilityInstallationError(
      "invalid_scope",
      `${field} is not a public-safe ref`,
    )
  }
  return value
}

const digestRef = (prefix: string, values: ReadonlyArray<string>): string =>
  `${prefix}.${createHash("sha256").update(values.join("\u0000")).digest("hex").slice(0, 32)}`

const sortedPermissions = (permissions: ReadonlyArray<string>): string[] => {
  const values = [...new Set(permissions.map(permission => assertRef(permission, "permission")))].sort()
  if (values.length === 0) {
    throw new PortableCapabilityInstallationError(
      "invalid_scope",
      "at least one capability permission is required",
    )
  }
  return values
}

const installationRefs = (
  lease: PortableCapabilityLease,
  permissions: ReadonlyArray<string>,
  executableProfileRef?: string,
  installReceiptRef?: string,
): Readonly<{ installationRef: string; evidenceRef: string }> => {
  const binding = [
    lease.ownerRef,
    lease.sessionRef,
    lease.attachmentRef,
    String(lease.attachmentGeneration),
    lease.targetRef,
    lease.leaseRef,
    lease.capability,
    ...(executableProfileRef === undefined || installReceiptRef === undefined
      ? []
      : [executableProfileRef, installReceiptRef]),
    ...sortedPermissions(permissions),
  ]
  return {
    installationRef: digestRef("installation.capability", binding),
    evidenceRef: digestRef("evidence.capability-installed", binding),
  }
}

const assertLeaseBinding = (
  lease: PortableCapabilityLease,
  binding: Readonly<{ ownerRef: string; targetRef: string; sessionRef?: string }>,
): void => {
  assertRef(lease.leaseRef, "leaseRef")
  assertRef(lease.ownerRef, "ownerRef")
  assertRef(lease.sessionRef, "sessionRef")
  assertRef(lease.attachmentRef, "attachmentRef")
  assertRef(lease.targetRef, "targetRef")
  if (
    lease.ownerRef !== binding.ownerRef ||
    lease.targetRef !== binding.targetRef ||
    (binding.sessionRef !== undefined && lease.sessionRef !== binding.sessionRef) ||
    !Number.isSafeInteger(lease.attachmentGeneration) ||
    lease.attachmentGeneration < 0 ||
    lease.state !== "issued"
  ) {
    throw new PortableCapabilityInstallationError(
      "invalid_scope",
      "capability lease does not match the bound target",
    )
  }
}

const capabilityDirectory = (root: string, leaseRef: string): string =>
  join(
    root,
    createHash("sha256").update(assertRef(leaseRef, "leaseRef")).digest("hex"),
  )

const markerBytes = (
  lease: PortableCapabilityLease,
  permissions: ReadonlyArray<string>,
  installationRef: string,
  evidenceRef: string,
  executableProfileRef?: string,
  installReceiptRef?: string,
): Uint8Array => new TextEncoder().encode(JSON.stringify(
  executableProfileRef === undefined
    ? { leaseRef: lease.leaseRef, evidenceRef }
    : {
        ownerRef: lease.ownerRef,
        targetRef: lease.targetRef,
        sessionRef: lease.sessionRef,
        attachmentRef: lease.attachmentRef,
        attachmentGeneration: lease.attachmentGeneration,
        leaseRef: lease.leaseRef,
        capability: lease.capability,
        permissionRefs: sortedPermissions(permissions),
        installationRef,
        evidenceRef,
        executableProfileRef,
        installReceiptRef,
      },
))

const wipeFile = async (path: string): Promise<void> => {
  let handle: FileHandle | undefined
  try {
    handle = await open(path, "r+")
    const file = await handle.stat()
    const zeros = new Uint8Array(Math.min(Math.max(file.size, 1), 64 * 1024))
    try {
      let offset = 0
      while (offset < file.size) {
        const length = Math.min(zeros.length, file.size - offset)
        await handle.write(zeros, 0, length, offset)
        offset += length
      }
      await handle.sync()
    } finally {
      zeros.fill(0)
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  } finally {
    await handle?.close()
  }
  await rm(path, { force: true })
}

export type OwnerLocalPortableCapabilityInstallationConfig = Readonly<{
  pylonHome: string
  ownerRef: string
  targetRef: string
}>

/**
 * Lease-scoped owner-local custody. This never reads or writes the display
 * account's ~/.codex home: callers must pass the active Pylon's isolated home.
 */
export class OwnerLocalPortableCapabilityInstallationPort
  implements PortableCapabilityTargetInstallationPort
{
  private readonly root: string
  private readonly ownerRef: string
  private readonly targetRef: string

  constructor(config: OwnerLocalPortableCapabilityInstallationConfig) {
    if (!isAbsolute(config.pylonHome)) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "owner-local Pylon home must be absolute",
      )
    }
    this.ownerRef = assertRef(config.ownerRef, "ownerRef")
    this.targetRef = assertRef(config.targetRef, "targetRef")
    this.root = resolve(config.pylonHome, "runtime", "portable-capabilities")
  }

  async install(input: Readonly<{
    lease: PortableCapabilityLease
    permissions: ReadonlyArray<string>
    material: SecretMaterial
    managedMarkerPath?: string | undefined
    executableProfileRef?: string | undefined
    installReceiptRef?: string | undefined
  }>): Promise<Readonly<{
    installationRef: string
    evidenceRef: string
    marker?: Readonly<{ leaseRef: string; evidenceRef: string }> | undefined
  }>> {
    assertLeaseBinding(input.lease, {
      ownerRef: this.ownerRef,
      targetRef: this.targetRef,
    })
    if (input.managedMarkerPath !== undefined || input.material.byteLength === 0) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "owner-local installation input is invalid",
      )
    }
    const hasExecutableProfile = input.executableProfileRef !== undefined
    const hasInstallReceipt = input.installReceiptRef !== undefined
    if (
      hasExecutableProfile !== hasInstallReceipt ||
      (input.executableProfileRef !== undefined && input.installReceiptRef !== undefined &&
        (!SAFE_REF.test(input.executableProfileRef) || !SAFE_REF.test(input.installReceiptRef)))
    ) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "owner-local executable profile binding is invalid",
      )
    }
    const refs = installationRefs(
      input.lease,
      input.permissions,
      input.executableProfileRef,
      input.installReceiptRef,
    )
    const directory = capabilityDirectory(this.root, input.lease.leaseRef)
    const lock = `${directory}.lock`
    await mkdir(this.root, { recursive: true, mode: 0o700 })
    await chmod(this.root, 0o700)
    try {
      await mkdir(lock, { mode: 0o700 })
    } catch {
      throw new PortableCapabilityInstallationError(
        "installation_busy",
        "owner-local capability installation is already in progress",
      )
    }

    try {
      const materialPath = join(directory, MATERIAL_FILE)
      const markerPath = join(directory, MARKER_FILE)
      let existingMaterial: Buffer | undefined
      let existingMarker: string | undefined
      try {
        existingMaterial = await readFile(materialPath)
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
      }
      try {
        existingMarker = await readFile(markerPath, "utf8")
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
      }
      if (existingMaterial !== undefined) {
        try {
          const expected = markerBytes(
            input.lease,
            input.permissions,
            refs.installationRef,
            refs.evidenceRef,
            input.executableProfileRef,
            input.installReceiptRef,
          )
          const materialMatches =
            existingMaterial.byteLength === input.material.byteLength &&
            timingSafeEqual(existingMaterial, input.material)
          if (!materialMatches) {
            throw new PortableCapabilityInstallationError(
              "installation_conflict",
              "owner-local capability installation conflicts with durable custody",
            )
          }
          if (existingMarker === undefined) {
            const markerTemp = join(directory, `.${MARKER_FILE}.${randomUUID()}`)
            try {
              await writeFile(markerTemp, expected, { mode: 0o600, flag: "wx" })
              await chmod(markerTemp, 0o600)
              await rename(markerTemp, markerPath)
            } finally {
              await wipeFile(markerTemp).catch(() => undefined)
            }
          } else {
            const actualMarker = new TextEncoder().encode(existingMarker)
            try {
              if (
                actualMarker.byteLength !== expected.byteLength ||
                !timingSafeEqual(actualMarker, expected)
              ) {
                throw new PortableCapabilityInstallationError(
                  "installation_conflict",
                  "owner-local capability installation conflicts with durable custody",
                )
              }
            } finally {
              actualMarker.fill(0)
            }
          }
          expected.fill(0)
          return refs
        } finally {
          existingMaterial.fill(0)
        }
      }
      if (existingMarker !== undefined) {
        throw new PortableCapabilityInstallationError(
          "installation_conflict",
          "owner-local capability marker exists without installed material",
        )
      }

      await mkdir(directory, { recursive: true, mode: 0o700 })
      await chmod(directory, 0o700)
      const materialTemp = join(directory, `.${MATERIAL_FILE}.${randomUUID()}`)
      const markerTemp = join(directory, `.${MARKER_FILE}.${randomUUID()}`)
      const marker = markerBytes(
        input.lease,
        input.permissions,
        refs.installationRef,
        refs.evidenceRef,
        input.executableProfileRef,
        input.installReceiptRef,
      )
      try {
        await writeFile(materialTemp, input.material, { mode: 0o600, flag: "wx" })
        await chmod(materialTemp, 0o600)
        await rename(materialTemp, materialPath)
        await writeFile(markerTemp, marker, { mode: 0o600, flag: "wx" })
        await chmod(markerTemp, 0o600)
        await rename(markerTemp, markerPath)
      } catch (error) {
        await wipeFile(materialTemp).catch(() => undefined)
        await wipeFile(markerTemp).catch(() => undefined)
        await wipeFile(materialPath).catch(() => undefined)
        await wipeFile(markerPath).catch(() => undefined)
        await rm(directory, { force: true, recursive: true }).catch(() => undefined)
        throw error
      } finally {
        marker.fill(0)
      }
      return refs
    } finally {
      await rm(lock, { force: true, recursive: true })
    }
  }

  async withInstalledMaterial<A>(input: Readonly<{
    leaseRef: string
    installationRef: string
    use: (material: SecretMaterial) => Promise<A>
  }>): Promise<A> {
    const leaseRef = assertRef(input.leaseRef, "leaseRef")
    assertRef(input.installationRef, "installationRef")
    const directory = capabilityDirectory(this.root, leaseRef)
    let material: Buffer
    try {
      material = await readFile(join(directory, MATERIAL_FILE))
    } catch {
      throw new PortableCapabilityInstallationError(
        "material_unavailable",
        "owner-local capability material is unavailable",
      )
    }
    try {
      let markerText: string
      try {
        markerText = await readFile(join(directory, MARKER_FILE), "utf8")
      } catch {
        throw new PortableCapabilityInstallationError(
          "material_unavailable",
          "owner-local capability marker is unavailable",
        )
      }
      const marker: unknown = JSON.parse(markerText)
      const record = marker !== null && typeof marker === "object" && !Array.isArray(marker)
        ? marker as Record<string, unknown>
        : null
      const evidenceDigest = typeof record?.evidenceRef === "string"
        ? record.evidenceRef.split(".").at(-1)
        : undefined
      const executableProfileRef = record?.executableProfileRef
      const hasExecutableProfile = record !== null && Object.hasOwn(record, "executableProfileRef")
      if (hasExecutableProfile && typeof executableProfileRef !== "string") {
        throw new PortableCapabilityInstallationError(
          "installation_conflict",
          "owner-local executable profile marker is invalid",
        )
      }
      if (record !== null && typeof executableProfileRef === "string") {
        const permissionRefs = Array.isArray(record?.permissionRefs) &&
          record.permissionRefs.every(ref => typeof ref === "string" && SAFE_REF.test(ref))
          ? record.permissionRefs as string[]
          : null
        const expectedRefs = permissionRefs === null ||
          typeof record?.ownerRef !== "string" ||
          typeof record?.targetRef !== "string" ||
          typeof record?.sessionRef !== "string" ||
          typeof record?.attachmentRef !== "string" ||
          !Number.isSafeInteger(record?.attachmentGeneration) ||
          typeof record?.capability !== "string" ||
          !CAPABILITY_KINDS.has(record.capability) ||
          typeof record?.installReceiptRef !== "string"
          ? null
          : {
              installationRef: digestRef("installation.capability", [
                record.ownerRef,
                record.sessionRef,
                record.attachmentRef,
                String(record.attachmentGeneration),
                record.targetRef,
                leaseRef,
                record.capability,
                executableProfileRef,
                record.installReceiptRef,
                ...sortedPermissions(permissionRefs),
              ]),
              evidenceRef: digestRef("evidence.capability-installed", [
                record.ownerRef,
                record.sessionRef,
                record.attachmentRef,
                String(record.attachmentGeneration),
                record.targetRef,
                leaseRef,
                record.capability,
                executableProfileRef,
                record.installReceiptRef,
                ...sortedPermissions(permissionRefs),
              ]),
            }
        if (
          expectedRefs === null ||
          ![
            record.ownerRef,
            record.targetRef,
            record.sessionRef,
            record.attachmentRef,
            executableProfileRef,
            record.installReceiptRef,
          ].every(value => typeof value === "string" && SAFE_REF.test(value)) ||
          typeof record.attachmentGeneration !== "number" ||
          record.attachmentGeneration <= 0 ||
          record.ownerRef !== this.ownerRef ||
          record.targetRef !== this.targetRef ||
          record.installationRef !== expectedRefs.installationRef ||
          record.evidenceRef !== expectedRefs.evidenceRef
        ) {
          throw new PortableCapabilityInstallationError(
            "installation_conflict",
            "owner-local executable profile marker is invalid",
          )
        }
      }
      if (
        record === null ||
        record.leaseRef !== leaseRef ||
        evidenceDigest === undefined ||
        input.installationRef !== `installation.capability.${evidenceDigest}`
      ) {
        throw new PortableCapabilityInstallationError(
          "installation_conflict",
          "owner-local capability marker is invalid",
        )
      }
      return await input.use(material as unknown as SecretMaterial)
    } finally {
      material.fill(0)
    }
  }

  async wipe(input: Readonly<{
    leaseRef: string
    targetRef: string
    attachmentRef: string
    attachmentGeneration: number
    installationRef?: string | undefined
  }>): Promise<Readonly<{ wipeReceiptRef: string }>> {
    const leaseRef = assertRef(input.leaseRef, "leaseRef")
    if (
      input.targetRef !== this.targetRef ||
      !SAFE_REF.test(input.attachmentRef) ||
      !Number.isSafeInteger(input.attachmentGeneration) ||
      input.attachmentGeneration < 0 ||
      input.installationRef === undefined ||
      !SAFE_REF.test(input.installationRef)
    ) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "owner-local wipe does not match the bound target",
      )
    }
    const directory = capabilityDirectory(this.root, leaseRef)
    await wipeFile(join(directory, MATERIAL_FILE))
    await wipeFile(join(directory, MARKER_FILE))
    await rm(directory, { force: true, recursive: true })
    return {
      wipeReceiptRef: digestRef("receipt.capability-wiped", [
        leaseRef,
        input.targetRef,
        input.attachmentRef,
        String(input.attachmentGeneration),
        input.installationRef,
      ]),
    }
  }

  custodyDirectory(leaseRef: string): string {
    return capabilityDirectory(this.root, leaseRef)
  }
}

export type ManagedPortableCapabilityInstallationConfig = Readonly<{
  baseUrl: string
  bearerToken: string
  ownerRef: string
  targetRef: string
  sessionRef: string
  resolveResource: (
    binding: ManagedPortableCapabilityResourceKey,
  ) => Promise<ManagedPortableCapabilityResourceBinding>
  fetch?: Fetch | undefined
  timeoutMs?: number | undefined
}>

export type ManagedPortableCapabilityResourceKey = Readonly<{
  ownerRef: string
  targetRef: string
  sessionRef: string
  attachmentRef: string
  attachmentGeneration: number
}>

export type ManagedPortableCapabilityResourceBinding =
  ManagedPortableCapabilityResourceKey & Readonly<{
    resourceRef: string
    state: "staged" | "active" | "quiesced" | "reclaimed"
  }>

type ManagedResourceRow = Readonly<{
  owner_user_id: string
  target_ref: string
  session_ref: string
  attachment_ref: string
  generation: string | number
  resource_ref: string
  state: "staged" | "active" | "quiesced" | "reclaimed"
  accepting_work: boolean
}>

export const createPostgresManagedPortableCapabilityResourceResolver = (
  config: Readonly<{
    sql: SyncSql
    ownerRef: string
    targetRef: string
    sessionRef: string
  }>,
): ManagedPortableCapabilityInstallationConfig["resolveResource"] => {
  const ownerRef = assertRef(config.ownerRef, "ownerRef")
  const targetRef = assertRef(config.targetRef, "targetRef")
  const sessionRef = assertRef(config.sessionRef, "sessionRef")
  return async key => {
    if (
      key.ownerRef !== ownerRef ||
      key.targetRef !== targetRef ||
      key.sessionRef !== sessionRef ||
      !SAFE_REF.test(key.attachmentRef) ||
      !Number.isSafeInteger(key.attachmentGeneration) ||
      key.attachmentGeneration <= 0
    ) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability lookup does not match the bound destination",
      )
    }
    const rows = (await config.sql`
      SELECT owner_user_id, target_ref, session_ref, attachment_ref, generation,
             resource_ref, state, accepting_work
      FROM khala_sync_portable_managed_targets
      WHERE owner_user_id = ${ownerRef}
        AND target_ref = ${targetRef}
        AND session_ref = ${sessionRef}
        AND attachment_ref = ${key.attachmentRef}
        AND generation = ${key.attachmentGeneration}
    `) as ReadonlyArray<ManagedResourceRow>
    if (rows.length !== 1) {
      throw new PortableCapabilityInstallationError(
        "target_unavailable",
        "exact retained managed capability destination is unavailable",
      )
    }
    const row = rows[0]!
    const generation = Number(row.generation)
    if (
      row.owner_user_id !== ownerRef ||
      row.target_ref !== targetRef ||
      row.session_ref !== sessionRef ||
      row.attachment_ref !== key.attachmentRef ||
      generation !== key.attachmentGeneration ||
      !SAFE_REF.test(row.resource_ref) ||
      !["staged", "active", "quiesced", "reclaimed"].includes(row.state) ||
      (row.state === "staged" && row.accepting_work)
    ) {
      throw new PortableCapabilityInstallationError(
        "installation_conflict",
        "retained managed capability destination has conflicting state",
      )
    }
    return {
      ownerRef,
      targetRef,
      sessionRef,
      attachmentRef: row.attachment_ref,
      attachmentGeneration: generation,
      resourceRef: row.resource_ref,
      state: row.state,
    }
  }
}

export const createPostgresManagedPortableCapabilityInstallationPort = (
  config: Omit<ManagedPortableCapabilityInstallationConfig, "resolveResource"> &
    Readonly<{ sql: SyncSql }>,
): ManagedPortableCapabilityInstallationPort =>
  new ManagedPortableCapabilityInstallationPort({
    ...config,
    resolveResource: createPostgresManagedPortableCapabilityResourceResolver({
      sql: config.sql,
      ownerRef: config.ownerRef,
      targetRef: config.targetRef,
      sessionRef: config.sessionRef,
    }),
  })

export class ManagedPortableCapabilityInstallationPort
  implements PortableCapabilityTargetInstallationPort
{
  private readonly installEndpoint: URL
  private readonly operationsEndpoint: URL
  private readonly fetch: Fetch
  private readonly timeoutMs: number
  private readonly ownerRef: string
  private readonly targetRef: string
  private readonly sessionRef: string

  constructor(private readonly config: ManagedPortableCapabilityInstallationConfig) {
    let baseUrl: URL
    try {
      baseUrl = new URL(config.baseUrl)
    } catch {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability target base URL is invalid",
      )
    }
    if (baseUrl.protocol !== "https:" && !LOOPBACK_HOSTS.has(baseUrl.hostname)) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability target requires HTTPS or authenticated loopback HTTP",
      )
    }
    if (config.bearerToken.length < 16) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability target bearer is missing",
      )
    }
    this.ownerRef = assertRef(config.ownerRef, "ownerRef")
    this.targetRef = assertRef(config.targetRef, "targetRef")
    this.sessionRef = assertRef(config.sessionRef, "sessionRef")
    this.installEndpoint = new URL(
      "/v1/portable-agent-computers/capabilities/install",
      baseUrl,
    )
    this.operationsEndpoint = new URL(
      "/v1/portable-agent-computers/operations",
      baseUrl,
    )
    this.fetch = config.fetch ?? globalThis.fetch
    this.timeoutMs = config.timeoutMs ?? 120_000
  }

  async install(input: Readonly<{
    lease: PortableCapabilityLease
    permissions: ReadonlyArray<string>
    material: SecretMaterial
    managedMarkerPath?: string | undefined
  }>): Promise<Readonly<{
    installationRef: string
    evidenceRef: string
    marker?: Readonly<{ leaseRef: string; evidenceRef: string }> | undefined
  }>> {
    assertLeaseBinding(input.lease, {
      ownerRef: this.ownerRef,
      targetRef: this.targetRef,
      sessionRef: this.sessionRef,
    })
    const refs = installationRefs(input.lease, input.permissions)
    const resource = await this.resolveExactResource(input.lease)
    if (resource.state !== "staged") {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability install requires the exact staged destination",
      )
    }
    const installationRef = `installation.agent-computer.capability.${createHash("sha256")
      .update(`${resource.resourceRef}|${input.lease.leaseRef}`)
      .digest("hex")
      .slice(0, 16)}`
    if (
      input.material.byteLength === 0 ||
      input.managedMarkerPath !==
        managedCapabilityMarkerPath(this.sessionRef, input.lease.leaseRef)
    ) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability install marker is invalid",
      )
    }
    const operationRef = digestRef("operation.capability-install", [
      installationRef,
      input.lease.leaseRef,
    ])
    const ownedMaterial = input.material.slice()
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs)
    let response: Response
    try {
      response = await this.fetch(this.installEndpoint, {
        method: "POST",
        headers: {
          authorization: `Bearer ${this.config.bearerToken}`,
          "content-type": "application/octet-stream",
          "X-OA-Operation-Ref": operationRef,
          "X-OA-Owner-Ref": this.ownerRef,
          "X-OA-Target-Ref": this.targetRef,
          "X-OA-Session-Ref": this.sessionRef,
          "X-OA-Attachment-Ref": input.lease.attachmentRef,
          "X-OA-Attachment-Generation": String(input.lease.attachmentGeneration),
          "X-OA-Lease-Ref": input.lease.leaseRef,
          "X-OA-Evidence-Ref": refs.evidenceRef,
          "X-OA-Capability": input.lease.capability,
        },
        body: ownedMaterial,
        signal: controller.signal,
      })
    } catch {
      throw new PortableCapabilityInstallationError(
        "target_unavailable",
        "managed capability target is unavailable",
      )
    } finally {
      clearTimeout(timeout)
      ownedMaterial.fill(0)
    }
    const envelope = await this.decodeResponse(response)
    const marker = record(envelope.marker)
    assertExactKeys(envelope, [
      "installationRef",
      "evidenceRef",
      "resourceRef",
      "marker",
      "material",
    ])
    assertExactKeys(marker, ["leaseRef", "evidenceRef"])
    if (
      envelope.installationRef !== installationRef ||
      envelope.evidenceRef !== refs.evidenceRef ||
      envelope.resourceRef !== resource.resourceRef ||
      envelope.material !== "excluded" ||
      marker.leaseRef !== input.lease.leaseRef ||
      marker.evidenceRef !== refs.evidenceRef
    ) {
      throw new PortableCapabilityInstallationError(
        "target_refused",
        "managed capability target returned a mismatched installation",
      )
    }
    return {
      installationRef,
      evidenceRef: refs.evidenceRef,
      marker: {
        leaseRef: input.lease.leaseRef,
        evidenceRef: refs.evidenceRef,
      },
    }
  }

  async wipe(input: Readonly<{
    leaseRef: string
    targetRef: string
    attachmentRef: string
    attachmentGeneration: number
    installationRef?: string | undefined
  }>): Promise<Readonly<{ wipeReceiptRef: string }>> {
    const leaseRef = assertRef(input.leaseRef, "leaseRef")
    if (
      input.targetRef !== this.targetRef ||
      !SAFE_REF.test(input.attachmentRef) ||
      !Number.isSafeInteger(input.attachmentGeneration) ||
      input.attachmentGeneration < 0 ||
      input.installationRef === undefined ||
      !SAFE_REF.test(input.installationRef)
    ) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability wipe does not match the bound target",
      )
    }
    const operationRef = digestRef("operation.capability-wipe", [
      leaseRef,
      input.installationRef,
      input.attachmentRef,
      String(input.attachmentGeneration),
    ])
    const resource = await this.resolveExactResource({
      ownerRef: this.ownerRef,
      targetRef: this.targetRef,
      sessionRef: this.sessionRef,
      attachmentRef: input.attachmentRef,
      attachmentGeneration: input.attachmentGeneration,
    })
    if (resource.state === "reclaimed") {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability wipe requires a retained destination",
      )
    }
    const body = {
      operationRef,
      action: "wipeCapability",
      ownerRef: this.ownerRef,
      targetRef: this.targetRef,
      resourceRef: resource.resourceRef,
      sessionRef: this.sessionRef,
      attachmentRef: input.attachmentRef,
      generation: input.attachmentGeneration,
      payload: { leaseRef, installationRef: input.installationRef },
    }
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs)
    let response: Response
    try {
      response = await this.fetch(this.operationsEndpoint, {
        method: "POST",
        headers: {
          authorization: `Bearer ${this.config.bearerToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
        signal: controller.signal,
      })
    } catch {
      throw new PortableCapabilityInstallationError(
        "target_unavailable",
        "managed capability target is unavailable",
      )
    } finally {
      clearTimeout(timeout)
    }
    const envelope = await this.decodeResponse(response)
    assertExactKeys(envelope, ["wipeReceiptRef", "material"])
    if (
      typeof envelope.wipeReceiptRef !== "string" ||
      !SAFE_REF.test(envelope.wipeReceiptRef) ||
      envelope.material !== "excluded"
    ) {
      throw new PortableCapabilityInstallationError(
        "target_refused",
        "managed capability target returned a mismatched wipe receipt",
      )
    }
    return { wipeReceiptRef: envelope.wipeReceiptRef }
  }

  private async decodeResponse(response: Response): Promise<Record<string, unknown>> {
    if (!response.ok) {
      throw new PortableCapabilityInstallationError(
        "target_refused",
        `managed capability target refused operation (${response.status})`,
      )
    }
    let value: unknown
    try {
      value = await response.json()
    } catch {
      throw new PortableCapabilityInstallationError(
        "target_refused",
        "managed capability target returned an invalid response",
      )
    }
    const envelope = record(value)
    if (FORBIDDEN_RESPONSE.test(JSON.stringify(envelope))) {
      throw new PortableCapabilityInstallationError(
        "unsafe_response",
        "managed capability target returned private material",
      )
    }
    return envelope
  }

  private async resolveExactResource(
    lease: Pick<
      PortableCapabilityLease,
      | "ownerRef"
      | "targetRef"
      | "sessionRef"
      | "attachmentRef"
      | "attachmentGeneration"
    >,
  ): Promise<ManagedPortableCapabilityResourceBinding> {
    const key = {
      ownerRef: lease.ownerRef,
      targetRef: lease.targetRef,
      sessionRef: lease.sessionRef,
      attachmentRef: lease.attachmentRef,
      attachmentGeneration: lease.attachmentGeneration,
    }
    let resource: ManagedPortableCapabilityResourceBinding
    try {
      resource = await this.config.resolveResource(key)
    } catch {
      throw new PortableCapabilityInstallationError(
        "target_unavailable",
        "managed capability staged resource is unavailable",
      )
    }
    if (
      resource.ownerRef !== key.ownerRef ||
      resource.targetRef !== key.targetRef ||
      resource.sessionRef !== key.sessionRef ||
      resource.attachmentRef !== key.attachmentRef ||
      resource.attachmentGeneration !== key.attachmentGeneration ||
      !SAFE_REF.test(resource.resourceRef) ||
      !["staged", "active", "quiesced", "reclaimed"].includes(resource.state)
    ) {
      throw new PortableCapabilityInstallationError(
        "invalid_scope",
        "managed capability resource does not match the exact destination",
      )
    }
    return resource
  }
}

const record = (value: unknown): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new PortableCapabilityInstallationError(
      "target_refused",
      "managed capability target returned an invalid response envelope",
    )
  }
  return value as Record<string, unknown>
}

const assertExactKeys = (
  value: Record<string, unknown>,
  expected: ReadonlyArray<string>,
): void => {
  const actual = Object.keys(value).sort()
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== [...expected].sort()[index])
  ) {
    throw new PortableCapabilityInstallationError(
      "unsafe_response",
      "managed capability target returned unexpected response fields",
    )
  }
}
