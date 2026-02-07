export type UiBlobRef = {
  readonly id: string
  readonly hash: string
  readonly size: number
  readonly mime?: string | undefined
}

export type UiBlobStore = {
  readonly putText: (options: { readonly text: string; readonly mime?: string }) => UiBlobRef
  readonly getText: (id: string) => string | null
}

function byteLengthUtf8(text: string): number {
  return new TextEncoder().encode(text).byteLength
}

// Simple deterministic hash for in-memory blob ids (not cryptographic).
function fnv1a32(text: string): string {
  let hash = 0x811c9dc5
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193)
  }
  // >>> 0 coerces to uint32
  return (hash >>> 0).toString(16).padStart(8, "0")
}

function blobIdFromText(text: string): string {
  return `${fnv1a32(text)}-${text.length.toString(16)}`
}

const store = new Map<string, { readonly text: string; readonly ref: UiBlobRef }>()

export const UiBlobStore: UiBlobStore = {
  putText: (options) => {
    const id = blobIdFromText(options.text)
    const existing = store.get(id)
    if (existing) return existing.ref

    const ref: UiBlobRef = {
      id,
      hash: id,
      size: byteLengthUtf8(options.text),
      ...(options.mime ? { mime: options.mime } : {}),
    }
    store.set(id, { text: options.text, ref })
    return ref
  },

  getText: (id) => store.get(id)?.text ?? null,
}

