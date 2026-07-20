/**
 * Composer image attachments (capability I1).
 *
 * The renderer holds pending image attachments as bounded base64 in shell
 * state, shows thumbnails with a remove affordance, and threads them into the
 * claude-local start payload on submit. This module owns the pure, testable
 * pieces: media-type/size classification, honest rejection copy, base64
 * decoding of an in-renderer `File`/`Blob` (drop or paste — never an arbitrary
 * filesystem read), thumbnail data URLs, and the bounded add/remove state
 * helpers. Nothing here reads the disk; `File` objects come only from a
 * user-driven drop/paste or a main-mediated file picker.
 */
import {
  CLAUDE_LOCAL_IMAGE_BYTES_LIMIT,
  CLAUDE_LOCAL_IMAGE_COUNT_LIMIT,
  CLAUDE_LOCAL_IMAGE_MEDIA_TYPES,
  type ClaudeLocalImageAttachment,
  type ClaudeLocalImageMediaType,
} from "../claude-local-contract.ts"

export const COMPOSER_IMAGE_COUNT_LIMIT = CLAUDE_LOCAL_IMAGE_COUNT_LIMIT
export const COMPOSER_IMAGE_BYTES_LIMIT = CLAUDE_LOCAL_IMAGE_BYTES_LIMIT

/** A pending composer attachment: base64 payload plus bounded display facts. */
export type ComposerImageAttachment = Readonly<{
  /** Stable id for keying thumbnails and the remove intent. */
  id: string
  mediaType: ClaudeLocalImageMediaType
  /** Raw base64, no `data:` prefix (matches the boundary contract). */
  data: string
  /** Bounded display name (never a filesystem path). */
  name: string
  /** Decoded byte size, for the thumbnail caption. */
  sizeBytes: number
}>

export type ImageClassification =
  | Readonly<{ ok: true; mediaType: ClaudeLocalImageMediaType }>
  | Readonly<{ ok: false; reason: ImageRejectionReason }>

export type ImageRejectionReason = "wrong_type" | "too_large" | "count_limit" | "unreadable"

/**
 * Classify a file-like descriptor by media type and size. Pure — no I/O — so
 * the reject path is unit-testable without a real `File`.
 */
export const classifyImageFile = (
  file: Readonly<{ type: string; size: number }>,
  currentCount: number,
): ImageClassification => {
  if (currentCount >= COMPOSER_IMAGE_COUNT_LIMIT) {
    return { ok: false, reason: "count_limit" }
  }
  const mediaType = (CLAUDE_LOCAL_IMAGE_MEDIA_TYPES as ReadonlyArray<string>).includes(file.type)
    ? (file.type as ClaudeLocalImageMediaType)
    : null
  if (mediaType === null) return { ok: false, reason: "wrong_type" }
  if (file.size <= 0 || file.size > COMPOSER_IMAGE_BYTES_LIMIT) {
    return { ok: false, reason: "too_large" }
  }
  return { ok: true, mediaType }
}

/** Honest, bounded rejection copy — surfaced transiently, never a standing caption. */
export const composerImageRejectionMessage = (reason: ImageRejectionReason): string => {
  switch (reason) {
    case "wrong_type":
      return "That file isn't a supported image (PNG, JPEG, WebP, or GIF)."
    case "too_large":
      return `That image is larger than the ${Math.round(COMPOSER_IMAGE_BYTES_LIMIT / (1024 * 1024))} MB limit.`
    case "count_limit":
      return `You can attach at most ${COMPOSER_IMAGE_COUNT_LIMIT} images per message.`
    case "unreadable":
      return "That image could not be read. Choose another file."
  }
}

/** base64-encode raw bytes (chunked; works in both Chromium and bun). */
export const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = ""
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk))
  }
  return btoa(binary)
}

export type FileLike = Readonly<{
  type: string
  size: number
  name?: string
  arrayBuffer: () => Promise<ArrayBuffer>
}>

export type ReadImageResult =
  | Readonly<{ ok: true; attachment: ComposerImageAttachment }>
  | Readonly<{ ok: false; reason: ImageRejectionReason }>

/**
 * Read an in-renderer `File`/`Blob` into a bounded base64 attachment. The
 * bytes already live in the renderer (a drop/paste `File` or a main-picker
 * payload) — this never touches the filesystem. Classification runs first so an
 * oversize/wrong-type file is rejected before decoding.
 */
export const readImageFile = async (
  file: FileLike,
  currentCount: number,
  makeId: () => string = () => globalThis.crypto.randomUUID(),
): Promise<ReadImageResult> => {
  const classification = classifyImageFile(file, currentCount)
  if (!classification.ok) return { ok: false, reason: classification.reason }
  const bytes = new Uint8Array(await file.arrayBuffer())
  // Re-check the decoded size (a lying `size` field cannot smuggle an oversize blob).
  if (bytes.length <= 0 || bytes.length > COMPOSER_IMAGE_BYTES_LIMIT) {
    return { ok: false, reason: "too_large" }
  }
  return {
    ok: true,
    attachment: {
      id: makeId(),
      mediaType: classification.mediaType,
      data: bytesToBase64(bytes),
      name: boundedName(file.name, classification.mediaType),
      sizeBytes: bytes.length,
    },
  }
}

const extensionFor = (mediaType: ClaudeLocalImageMediaType): string =>
  mediaType === "image/jpeg" ? "jpg" : mediaType.slice("image/".length)

const boundedName = (name: string | undefined, mediaType: ClaudeLocalImageMediaType): string => {
  const trimmed = (name ?? "").trim()
  if (trimmed !== "") return trimmed.slice(0, 256)
  return `image.${extensionFor(mediaType)}`
}

/** The `data:` URL for a thumbnail (renderer-only; never crosses the boundary). */
export const composerImageDataUrl = (attachment: ComposerImageAttachment): string =>
  `data:${attachment.mediaType};base64,${attachment.data}`

/** Add an attachment, bounded by the count limit (over-limit adds are dropped). */
export const addComposerImage = (
  list: ReadonlyArray<ComposerImageAttachment>,
  attachment: ComposerImageAttachment,
): ReadonlyArray<ComposerImageAttachment> =>
  list.length >= COMPOSER_IMAGE_COUNT_LIMIT || list.some((item) => item.id === attachment.id)
    ? list
    : [...list, attachment]

export const removeComposerImage = (
  list: ReadonlyArray<ComposerImageAttachment>,
  id: string,
): ReadonlyArray<ComposerImageAttachment> => list.filter((item) => item.id !== id)

export const canAttachMoreImages = (list: ReadonlyArray<ComposerImageAttachment>): boolean =>
  list.length < COMPOSER_IMAGE_COUNT_LIMIT

/** Project pending attachments to the frozen boundary shape (drops renderer-only fields). */
export const toStartImages = (
  list: ReadonlyArray<ComposerImageAttachment>,
): ReadonlyArray<ClaudeLocalImageAttachment> =>
  list.map((item) => ({ mediaType: item.mediaType, data: item.data, name: item.name }))

/** Human byte-size label for a thumbnail caption. */
export const formatImageSize = (bytes: number): string =>
  bytes >= 1024 * 1024
    ? `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1024))} KB`
