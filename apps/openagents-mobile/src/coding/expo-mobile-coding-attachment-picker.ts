import { CryptoDigestAlgorithm, digest } from "expo-crypto"
import { Directory, File, Paths } from "expo-file-system"

import {
  openMobileCodingAttachmentPicker,
  type MobileCodingAttachmentPicker,
} from "./mobile-coding-attachment-picker"
import type { MobileCodingAttachmentDeliveryPort } from "./mobile-coding-attachment-delivery"

const hex = (buffer: ArrayBuffer): string =>
  Array.from(new Uint8Array(buffer), byte => byte.toString(16).padStart(2, "0")).join("")

const attachmentDirectory = (): Directory =>
  new Directory(Paths.document, "openagents-coding-attachments")

const sha256 = async (bytes: Uint8Array): Promise<string> => hex(await digest(
  CryptoDigestAlgorithm.SHA256,
  Uint8Array.from(bytes).buffer,
))

export const openExpoMobileCodingAttachmentDelivery = (): MobileCodingAttachmentDeliveryPort => ({
  read: digest => new File(attachmentDirectory(), digest).bytes(),
  sha256,
})

export const openExpoMobileCodingAttachmentPicker = (): MobileCodingAttachmentPicker => {
  const attachments = attachmentDirectory()
  return openMobileCodingAttachmentPicker({
    pickFiles: async () => {
      const result = await File.pickFileAsync({
        mimeTypes: "*/*",
        multipleFiles: true,
      })
      if (result.canceled) return { canceled: true, files: [] }
      return {
        canceled: false,
        files: result.result.map(file => ({
          name: file.name,
          mime: file.type,
          sizeBytes: file.size,
          bytes: () => file.bytes(),
          persist: async (sha256: string) => {
            attachments.create({ idempotent: true, intermediates: true })
            const destination = new File(attachments, sha256)
            if (!destination.exists) await file.copy(destination)
          },
        })),
      }
    },
    sha256,
  })
}
