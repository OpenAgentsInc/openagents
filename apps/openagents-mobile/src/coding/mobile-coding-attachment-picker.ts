import {
  MAX_MOBILE_CODING_ATTACHMENT_BYTES,
  MAX_MOBILE_CODING_ATTACHMENT_FILES_PER_PICK,
  type MobileCodingAttachmentFile,
} from "./mobile-coding-composer"

export type MobileCodingPickedFile = Readonly<{
  name: string
  mime: string
  sizeBytes: number
  bytes: () => Promise<Uint8Array>
  persist: (digest: string) => Promise<void>
}>

export type MobileCodingAttachmentPickerPort = Readonly<{
  pickFiles: () => Promise<Readonly<{
    canceled: boolean
    files: ReadonlyArray<MobileCodingPickedFile>
  }>>
  sha256: (bytes: Uint8Array) => Promise<string>
}>

export type MobileCodingAttachmentPickResult =
  | Readonly<{ status: "cancelled" }>
  | Readonly<{
      status: "selected"
      files: ReadonlyArray<MobileCodingAttachmentFile>
    }>
  | Readonly<{ status: "failed"; error: string }>

export type MobileCodingAttachmentPicker = Readonly<{
  pick: () => Promise<MobileCodingAttachmentPickResult>
}>

const pickerFailure = (error: string): MobileCodingAttachmentPickResult => ({
  status: "failed",
  error,
})

export const openMobileCodingAttachmentPicker = (
  port: MobileCodingAttachmentPickerPort,
): MobileCodingAttachmentPicker => ({
  pick: async () => {
    try {
      const selected = await port.pickFiles()
      if (selected.canceled) return { status: "cancelled" }
      if (selected.files.length === 0) return { status: "cancelled" }
      if (selected.files.length > MAX_MOBILE_CODING_ATTACHMENT_FILES_PER_PICK) {
        return pickerFailure(
          `Choose up to ${MAX_MOBILE_CODING_ATTACHMENT_FILES_PER_PICK} files or images at a time.`,
        )
      }
      if (selected.files.some(file =>
        !Number.isSafeInteger(file.sizeBytes) ||
        file.sizeBytes < 0 ||
        file.sizeBytes > MAX_MOBILE_CODING_ATTACHMENT_BYTES)) {
        return pickerFailure("Each file or image must be 25 MB or smaller.")
      }

      const prepared: MobileCodingAttachmentFile[] = []
      const digests = new Set<string>()
      for (const file of selected.files) {
        const bytes = await file.bytes()
        if (bytes.byteLength > MAX_MOBILE_CODING_ATTACHMENT_BYTES) {
          return pickerFailure("Each file or image must be 25 MB or smaller.")
        }
        const digest = (await port.sha256(bytes)).trim().toLowerCase()
        if (!/^[a-f0-9]{64}$/u.test(digest)) {
          return pickerFailure("The selected file or image could not be stored on this device.")
        }
        if (digests.has(digest)) continue
        await file.persist(digest)
        digests.add(digest)
        prepared.push({
          name: file.name.trim().slice(0, 160) || "attachment",
          mime: file.mime.trim().slice(0, 128) || "application/octet-stream",
          sizeBytes: bytes.byteLength,
          digest,
        })
      }
      return prepared.length === 0
        ? { status: "cancelled" }
        : { status: "selected", files: prepared }
    } catch {
      return pickerFailure("The selected file or image could not be stored on this device.")
    }
  },
})
