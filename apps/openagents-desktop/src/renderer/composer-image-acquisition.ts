/**
 * One DOM acquisition adapter for composer images. Picker results already
 * enter through the same typed shell intents; this adapter makes clipboard
 * and drop files converge on the same decoder, limits, and add/reject intents.
 */
import {
  composerImageRejectionMessage,
  readImageFile,
  type ComposerImageAttachment,
} from "./composer-images.ts"

export type ComposerImageAcquisitionSnapshot = Readonly<{
  pending: boolean
  imageCount: number
}>

export type ComposerImageAcquisitionHost = Readonly<{
  readSnapshot: () => Promise<ComposerImageAcquisitionSnapshot>
  add: (attachment: ComposerImageAttachment) => Promise<void>
  reject: (message: string) => Promise<void>
}>

const targetInComposer = (target: EventTarget | null): boolean => {
  const closest = (target as { closest?: (selector: string) => Element | null } | null)?.closest
  return typeof closest === "function" && closest.call(target, '[data-en-key="shell-composer"]') !== null
}

export const acquireComposerImages = async (
  files: ReadonlyArray<File>,
  host: ComposerImageAcquisitionHost,
): Promise<void> => {
  const snapshot = await host.readSnapshot()
  if (snapshot.pending) return
  let count = snapshot.imageCount
  let firstRejection: string | null = null
  for (const file of files) {
    const result = await readImageFile(file, count)
    if (result.ok) {
      count += 1
      await host.add(result.attachment)
    } else if (firstRejection === null) {
      firstRejection = composerImageRejectionMessage(result.reason)
    }
  }
  if (firstRejection !== null) await host.reject(firstRejection)
}

/** Installs the scoped paste/drop bridge and returns its exact cleanup. */
export const installComposerImageAcquisition = (
  targetWindow: Window,
  host: ComposerImageAcquisitionHost,
): (() => void) => {
  let queue = Promise.resolve()
  const enqueue = (files: ReadonlyArray<File>): void => {
    queue = queue.then(() => acquireComposerImages(files, host)).catch(() => host.reject(
      composerImageRejectionMessage("unreadable"),
    ))
  }
  const onDragOver = (event: DragEvent): void => {
    if (!targetInComposer(event.target)) return
    event.preventDefault()
    if (event.dataTransfer !== null) event.dataTransfer.dropEffect = "copy"
  }
  const onDrop = (event: DragEvent): void => {
    if (!targetInComposer(event.target)) return
    const files = event.dataTransfer?.files === undefined ? [] : [...event.dataTransfer.files]
    if (files.length === 0) return
    event.preventDefault()
    enqueue(files)
  }
  const onPaste = (event: ClipboardEvent): void => {
    if (!targetInComposer(event.target)) return
    const files: File[] = []
    for (const item of event.clipboardData?.items ?? []) {
      if (item.kind !== "file") continue
      const file = item.getAsFile()
      if (file !== null) files.push(file)
    }
    if (files.length === 0) return // ordinary text paste remains native
    event.preventDefault()
    enqueue(files)
  }
  targetWindow.addEventListener("dragover", onDragOver)
  targetWindow.addEventListener("drop", onDrop)
  targetWindow.addEventListener("paste", onPaste)
  return () => {
    targetWindow.removeEventListener("dragover", onDragOver)
    targetWindow.removeEventListener("drop", onDrop)
    targetWindow.removeEventListener("paste", onPaste)
  }
}
