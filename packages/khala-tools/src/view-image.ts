import { readFile, realpath, stat } from "node:fs/promises"
import { basename, isAbsolute, relative, resolve } from "node:path"
import { Effect } from "effect"
import {
  khalaToolDenied,
  khalaToolError,
  khalaToolOk,
  type KhalaPermissionRequest,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export interface KhalaViewImageToolOptions {
  readonly maxBytes?: number
  readonly visionSupported?: boolean
}

export const viewImageToolDefinition: KhalaToolDefinition = {
  authority: "read",
  availability: ["inspect", "coding", "owner_local_full"],
  description: "Inspect a local image file after normal read permission succeeds.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      path: {
        description: "Workspace-relative path or approved absolute image path.",
        type: "string",
      },
    },
    required: ["path"],
    type: "object",
  },
  internalId: "khala.media.view_image",
  label: "View Image",
  name: "view_image",
  outputSchema: {
    additionalProperties: false,
    properties: {
      artifactRef: { type: "string" },
      height: { type: "integer" },
      mediaType: { type: "string" },
      path: { type: "string" },
      visionSupported: { type: "boolean" },
      width: { type: "integer" },
    },
    required: ["path", "mediaType", "width", "height", "artifactRef", "visionSupported"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "View one local image file with a private preview artifact.",
  promptGuidelines: [
    "Use read for text files and view_image for PNG, JPEG, GIF, or WebP files.",
    "Do not include private image bytes in public summaries.",
    "If the active backend lacks vision support, use the UI preview metadata and continue accordingly.",
  ],
  renderer: { kind: "image_preview", rendererRef: "khala.renderer.image_preview.v1" },
}

export function createViewImageTool(options: KhalaViewImageToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: viewImageToolDefinition,
    execute: (input, context) => executeViewImageTool(input, context, options),
  }
}

type ViewImageInput = Readonly<{
  path: string
}>

type ImageInfo = Readonly<{
  height: number
  mediaType: "image/gif" | "image/jpeg" | "image/png" | "image/webp"
  width: number
}>

function executeViewImageTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaViewImageToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeViewImageInput(input)
      if (credentialPathDenied(args.path)) {
        return khalaToolError("view_image_blocked_credential_path", "view_image blocked a credential-shaped path")
      }

      const resolved = await resolveReadablePath(args.path, context)
      if (resolved._tag === "denied") {
        return khalaToolDenied("view_image_external_directory_denied", "view_image outside the workspace was denied")
      }

      const info = await stat(resolved.realPath)
      if (!info.isFile()) {
        return khalaToolError("view_image_blocked_file_type", "view_image only supports regular image files")
      }
      const maxBytes = options.maxBytes ?? 8 * 1024 * 1024
      if (info.size > maxBytes) {
        return khalaToolError("view_image_oversized", `image is ${info.size} bytes, larger than the ${maxBytes} byte limit`)
      }

      const bytes = await readFile(resolved.realPath)
      const image = decodeImageInfo(bytes)
      if (image === undefined) {
        return khalaToolError("view_image_unsupported_format", "view_image supports PNG, JPEG, GIF, and WebP images")
      }

      const artifact = await Effect.runPromise(
        context.services.outputStore.writeArtifact({
          bytes,
          mediaType: image.mediaType,
          summary: `image preview for ${resolved.displayPath}`,
        }),
      )
      const visionSupported = options.visionSupported === true
      return khalaToolOk({
        artifacts: [artifact],
        modelText: visionSupported
          ? `Image ${resolved.displayPath} attached as private artifact ${artifact.artifactRef} (${image.width}x${image.height}, ${image.mediaType}).`
          : `Image ${resolved.displayPath} is available as private UI artifact ${artifact.artifactRef} (${image.width}x${image.height}, ${image.mediaType}); active backend does not support vision content parts.`,
        privateDataRefs: [artifact.artifactRef],
        publicSafety: "private",
        publicSummary: `Viewed image ${resolved.displayPath} (${image.width}x${image.height}, ${image.mediaType}); private bytes stored as artifact.`,
        ui: {
          artifactRef: artifact.artifactRef,
          byteLength: bytes.byteLength,
          displayPath: resolved.displayPath,
          height: image.height,
          kind: "image_preview",
          mediaType: image.mediaType,
          modelContentParts: visionSupported
            ? [
              {
                artifactRef: artifact.artifactRef,
                mediaType: image.mediaType,
                type: "image",
              },
            ]
            : [],
          redaction: {
            classification: "private_image_bytes",
            publicSafe: false,
          },
          visionSupported,
          width: image.width,
        },
      })
    } catch (error) {
      return khalaToolError("view_image_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

function decodeViewImageInput(input: Readonly<Record<string, unknown>>): ViewImageInput {
  const path = typeof input.path === "string" ? input.path.trim() : ""
  if (path.length === 0) throw new Error("view_image requires a non-empty path")
  return { path }
}

async function resolveReadablePath(
  rawPath: string,
  context: KhalaToolExecuteContext,
): Promise<
  | Readonly<{
      _tag: "ok"
      displayPath: string
      realPath: string
    }>
  | Readonly<{ _tag: "denied" }>
> {
  const workspaceRoot = await realpath(context.services.workspace.workingDirectory)
  const candidate = isAbsolute(rawPath) ? rawPath : resolve(workspaceRoot, rawPath)
  const target = await realpath(candidate)
  const inside = pathIsInside(workspaceRoot, target)
  if (!inside) {
    const decision = await Effect.runPromise(
      context.services.permission.decide(externalDirectoryPermission(rawPath, context)),
    )
    if (decision === "deny") return { _tag: "denied" }
  }
  return {
    _tag: "ok",
    displayPath: inside ? toWorkspaceRelative(workspaceRoot, target) : rawPath,
    realPath: target,
  }
}

function externalDirectoryPermission(rawPath: string, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "external_directory",
    authorityMode: "local",
    publicSafety: "private",
    resources: [rawPath],
    saveScope: "once",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "view_image",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function decodeImageInfo(bytes: Uint8Array): ImageInfo | undefined {
  return decodePng(bytes) ?? decodeJpeg(bytes) ?? decodeGif(bytes) ?? decodeWebp(bytes)
}

function decodePng(bytes: Uint8Array): ImageInfo | undefined {
  if (bytes.byteLength < 24) return undefined
  if (
    bytes[0] !== 0x89 ||
    bytes[1] !== 0x50 ||
    bytes[2] !== 0x4e ||
    bytes[3] !== 0x47 ||
    bytes[4] !== 0x0d ||
    bytes[5] !== 0x0a ||
    bytes[6] !== 0x1a ||
    bytes[7] !== 0x0a ||
    ascii(bytes, 12, 16) !== "IHDR"
  ) {
    return undefined
  }
  return {
    height: readUInt32BE(bytes, 20),
    mediaType: "image/png",
    width: readUInt32BE(bytes, 16),
  }
}

function decodeGif(bytes: Uint8Array): ImageInfo | undefined {
  if (bytes.byteLength < 10) return undefined
  const header = ascii(bytes, 0, 6)
  if (header !== "GIF87a" && header !== "GIF89a") return undefined
  return {
    height: readUInt16LE(bytes, 8),
    mediaType: "image/gif",
    width: readUInt16LE(bytes, 6),
  }
}

function decodeJpeg(bytes: Uint8Array): ImageInfo | undefined {
  if (bytes.byteLength < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return undefined
  let offset = 2
  while (offset + 9 < bytes.byteLength) {
    if (bytes[offset] !== 0xff) return undefined
    const marker = bytes[offset + 1]
    offset += 2
    if (marker === 0xd9 || marker === 0xda) return undefined
    if (offset + 2 > bytes.byteLength) return undefined
    const segmentLength = readUInt16BE(bytes, offset)
    if (segmentLength < 2 || offset + segmentLength > bytes.byteLength) return undefined
    if (isJpegStartOfFrame(marker)) {
      return {
        height: readUInt16BE(bytes, offset + 3),
        mediaType: "image/jpeg",
        width: readUInt16BE(bytes, offset + 5),
      }
    }
    offset += segmentLength
  }
  return undefined
}

function decodeWebp(bytes: Uint8Array): ImageInfo | undefined {
  if (bytes.byteLength < 30 || ascii(bytes, 0, 4) !== "RIFF" || ascii(bytes, 8, 12) !== "WEBP") return undefined
  const chunk = ascii(bytes, 12, 16)
  if (chunk === "VP8X") {
    return {
      height: readUInt24LE(bytes, 27) + 1,
      mediaType: "image/webp",
      width: readUInt24LE(bytes, 24) + 1,
    }
  }
  if (chunk === "VP8L" && bytes.byteLength >= 25) {
    const b0 = bytes[21] ?? 0
    const b1 = bytes[22] ?? 0
    const b2 = bytes[23] ?? 0
    const b3 = bytes[24] ?? 0
    return {
      height: 1 + ((b2 >> 6) | b3 << 2),
      mediaType: "image/webp",
      width: 1 + (((b1 & 0x3f) << 8) | b0),
    }
  }
  if (chunk === "VP8 " && bytes.byteLength >= 30) {
    return {
      height: readUInt16LE(bytes, 28) & 0x3fff,
      mediaType: "image/webp",
      width: readUInt16LE(bytes, 26) & 0x3fff,
    }
  }
  return undefined
}

function isJpegStartOfFrame(marker: number): boolean {
  return marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc
}

function ascii(bytes: Uint8Array, start: number, end: number): string {
  return Buffer.from(bytes.subarray(start, end)).toString("ascii")
}

function readUInt16BE(bytes: Uint8Array, offset: number): number {
  return ((bytes[offset] ?? 0) << 8) | (bytes[offset + 1] ?? 0)
}

function readUInt16LE(bytes: Uint8Array, offset: number): number {
  return (bytes[offset] ?? 0) | ((bytes[offset + 1] ?? 0) << 8)
}

function readUInt24LE(bytes: Uint8Array, offset: number): number {
  return (bytes[offset] ?? 0) | ((bytes[offset + 1] ?? 0) << 8) | ((bytes[offset + 2] ?? 0) << 16)
}

function readUInt32BE(bytes: Uint8Array, offset: number): number {
  return (
    ((bytes[offset] ?? 0) * 0x1000000) +
    ((bytes[offset + 1] ?? 0) << 16) +
    ((bytes[offset + 2] ?? 0) << 8) +
    (bytes[offset + 3] ?? 0)
  )
}

function pathIsInside(root: string, target: string): boolean {
  const rel = relative(root, target)
  return rel === "" || !rel.startsWith("..") && !isAbsolute(rel)
}

function toWorkspaceRelative(root: string, target: string): string {
  const rel = relative(root, target)
  return rel === "" ? "." : rel.split("\\").join("/")
}

const CREDENTIAL_BASENAMES = new Set([
  ".env",
  ".npmrc",
  "auth.json",
  "id_ed25519",
  "id_rsa",
  "provider-key.json",
])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
