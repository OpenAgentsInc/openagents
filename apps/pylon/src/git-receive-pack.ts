import { createHash } from "node:crypto"

export type GitReceivePackCommandAction = "create" | "update" | "delete"

export type GitReceivePackCommand = {
  oldObjectId: string
  newObjectId: string
  refName: string
  action: GitReceivePackCommandAction
}

export type GitReceivePackRequest = {
  schema: "openagents.pylon.git_receive_pack.v0.1"
  commands: readonly GitReceivePackCommand[]
  capabilities: readonly string[]
  packfile: Uint8Array
  packfileBytes: number
  packfileSha256: string
  sourceRefs: readonly string[]
}

export class GitReceivePackParseError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "GitReceivePackParseError"
  }
}

const decoder = new TextDecoder("utf-8", { fatal: true })
const objectIdPattern = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/i
const zeroObjectIdPattern = /^(?:0{40}|0{64})$/
const invalidRefCharPattern = /[\x00-\x20\x7f~^:?*[\\]/
const pktLineHeaderPattern = /^[0-9a-fA-F]{4}$/

function fail(message: string): never {
  throw new GitReceivePackParseError(message)
}

function normalizeInput(input: Uint8Array | ArrayBuffer): Uint8Array {
  return input instanceof Uint8Array ? input : new Uint8Array(input)
}

function decodeUtf8(bytes: Uint8Array): string {
  try {
    return decoder.decode(bytes)
  } catch {
    return fail("git receive-pack contains non-UTF-8 command pkt-line")
  }
}

function stripLineBreak(value: string): string {
  const withoutLf = value.endsWith("\n") ? value.slice(0, -1) : value
  return withoutLf.endsWith("\r") ? withoutLf.slice(0, -1) : withoutLf
}

function parsePktLineLength(bytes: Uint8Array, offset: number): number {
  if (offset + 4 > bytes.length) {
    return fail("git receive-pack ended inside a pkt-line header")
  }
  const header = decodeUtf8(bytes.subarray(offset, offset + 4))
  if (!pktLineHeaderPattern.test(header)) {
    return fail("git receive-pack pkt-line header is not four hex digits")
  }

  const length = Number.parseInt(header, 16)
  if (length !== 0 && length < 4) {
    return fail("git receive-pack pkt-line length is below the header size")
  }
  if (offset + length > bytes.length) {
    return fail("git receive-pack pkt-line length exceeds request body")
  }
  return length
}

function validateObjectId(value: string, field: string): string {
  if (!objectIdPattern.test(value)) {
    return fail(`git receive-pack ${field} must be a SHA-1 or SHA-256 object id`)
  }
  return value.toLowerCase()
}

function isZeroObjectId(value: string): boolean {
  return zeroObjectIdPattern.test(value)
}

function validateRefName(value: string): string {
  if (!value.startsWith("refs/")) {
    return fail("git receive-pack ref name must start with refs/")
  }
  if (value.length > 255) {
    return fail("git receive-pack ref name is too long")
  }
  if (value.endsWith("/") || value.endsWith(".")) {
    return fail("git receive-pack ref name has an invalid suffix")
  }
  if (value.includes("..") || value.includes("@{")) {
    return fail("git receive-pack ref name contains a forbidden sequence")
  }
  if (invalidRefCharPattern.test(value)) {
    return fail("git receive-pack ref name contains a forbidden character")
  }

  const components = value.split("/")
  for (const component of components) {
    if (
      component.length === 0 ||
      component === "." ||
      component === ".." ||
      component.endsWith(".lock")
    ) {
      return fail("git receive-pack ref name contains an invalid path component")
    }
  }

  return value
}

function parseCapabilities(value: string): readonly string[] {
  if (value.trim().length === 0) {
    return []
  }
  return value.trim().split(/\s+/).map((capability) => {
    if (capability.length > 128 || /[\x00-\x20\x7f]/.test(capability)) {
      return fail("git receive-pack capability contains invalid bytes")
    }
    return capability
  })
}

function actionForObjectIds(
  oldObjectId: string,
  newObjectId: string,
): GitReceivePackCommandAction {
  const oldIsZero = isZeroObjectId(oldObjectId)
  const newIsZero = isZeroObjectId(newObjectId)
  if (oldIsZero && newIsZero) {
    return fail("git receive-pack command cannot use two zero object ids")
  }
  if (oldIsZero) {
    return "create"
  }
  if (newIsZero) {
    return "delete"
  }
  return "update"
}

function parseCommandPktLine(input: {
  line: string
  isFirstCommand: boolean
}): { command: GitReceivePackCommand; capabilities: readonly string[] } {
  const line = stripLineBreak(input.line)
  const nulIndex = line.indexOf("\0")
  if (!input.isFirstCommand && nulIndex !== -1) {
    return fail("git receive-pack capabilities are only allowed on the first command")
  }

  const commandText = nulIndex === -1 ? line : line.slice(0, nulIndex)
  const capabilities = nulIndex === -1 ? [] : parseCapabilities(line.slice(nulIndex + 1))
  const parts = commandText.split(" ")
  if (parts.length !== 3 || parts.some((part) => part.length === 0)) {
    return fail("git receive-pack command must be '<old> <new> <ref>'")
  }

  const oldObjectId = validateObjectId(parts[0]!, "old object id")
  const newObjectId = validateObjectId(parts[1]!, "new object id")
  if (oldObjectId.length !== newObjectId.length) {
    return fail("git receive-pack old and new object ids must use the same hash width")
  }

  return {
    command: {
      oldObjectId,
      newObjectId,
      refName: validateRefName(parts[2]!),
      action: actionForObjectIds(oldObjectId, newObjectId),
    },
    capabilities,
  }
}

function startsWithPackHeader(bytes: Uint8Array): boolean {
  return (
    bytes.length >= 4 &&
    bytes[0] === 0x50 &&
    bytes[1] === 0x41 &&
    bytes[2] === 0x43 &&
    bytes[3] === 0x4b
  )
}

export function parseGitReceivePackRequest(
  input: Uint8Array | ArrayBuffer,
): GitReceivePackRequest {
  const bytes = normalizeInput(input)
  const commands: GitReceivePackCommand[] = []
  let capabilities: readonly string[] = []
  let offset = 0
  let sawFlush = false

  while (offset < bytes.length) {
    const lineLength = parsePktLineLength(bytes, offset)
    offset += 4

    if (lineLength === 0) {
      sawFlush = true
      break
    }

    const payloadLength = lineLength - 4
    const lineBytes = bytes.subarray(offset, offset + payloadLength)
    const parsed = parseCommandPktLine({
      line: decodeUtf8(lineBytes),
      isFirstCommand: commands.length === 0,
    })
    commands.push(parsed.command)
    if (commands.length === 1) {
      capabilities = parsed.capabilities
    }
    offset += payloadLength
  }

  if (!sawFlush) {
    return fail("git receive-pack command list is missing its flush pkt-line")
  }
  if (commands.length === 0) {
    return fail("git receive-pack request must contain at least one ref command")
  }

  const packfile = bytes.slice(offset)
  const writesObjects = commands.some((command) => command.action !== "delete")
  if (writesObjects && packfile.length === 0) {
    return fail("git receive-pack object update is missing a packfile")
  }
  if (packfile.length > 0 && !startsWithPackHeader(packfile)) {
    return fail("git receive-pack trailing payload is not a PACK file")
  }

  return {
    schema: "openagents.pylon.git_receive_pack.v0.1",
    commands,
    capabilities,
    packfile,
    packfileBytes: packfile.length,
    packfileSha256: createHash("sha256").update(packfile).digest("hex"),
    sourceRefs: [
      "issue.public.github.OpenAgentsInc.openagents.6747",
      "doc.public.forge.owned_coordination_layer.2026-06-28",
    ],
  }
}
