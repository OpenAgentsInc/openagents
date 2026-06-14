// CL / #4995: local filesystem / MCP brand-asset ingestion (desktop, Bun main).
//
// COORDINATOR WIRING:
//   - Expose `ingestAssets` from `src/bun/index.ts` and surface it over the
//     Electrobun RPC bridge as a webview-callable handler (e.g. an
//     `assets.ingest` verb), returning only the redaction-safe descriptors —
//     never raw bytes or absolute machine paths beyond what the descriptor
//     already carries.
//   - The LIVE `AssetSource` (a real filesystem watch-folder reader, or an MCP
//     filesystem server client reading Drive-synced brand assets) is
//     owner/runtime-gated: it is constructed in the Bun process behind an
//     explicit enable + a configured root, and INJECTED here. This module never
//     touches the real FS / MCP itself so it stays fully unit-testable.
//   - Uploading the enumerated descriptors into a workroom, and any subsequent
//     externally-visible action, routes through the cloud approval/lifecycle
//     decision seam (see pylon-control.ts + ambient-browser-automation.ts). No
//     new authority is introduced on the desktop.

// A single asset as the source presents it before classification. `path` is the
// source-relative locator (not necessarily an absolute machine path); `bytes`
// is the on-disk/remote size if the source knows it.
export type RawAsset = {
  readonly path: string
  readonly bytes?: number | undefined
  // Source-provided MIME/type hint, if any (e.g. from an MCP file listing).
  readonly contentType?: string | undefined
  // Source-provided last-modified marker, if any (ISO-8601 preferred).
  readonly modifiedAt?: string | undefined
}

// The injected source. Models either a local filesystem watch-folder or an MCP
// filesystem server. No real FS/MCP at this layer — fakes are used in tests.
export interface AssetSource {
  // Enumerate the candidate assets under the source root. May reject; the
  // ingest run surfaces that as a run-level error rather than throwing.
  listAssets(): Promise<readonly RawAsset[]>
  // Read a single asset's bytes by its source path. Per-asset failures are
  // surfaced as errors on that descriptor; they do not abort the whole run.
  readAsset(path: string): Promise<Uint8Array>
}

export type AssetKind = "image" | "video" | "audio" | "document" | "font" | "other"

// The redaction-safe descriptor that is safe to cross the RPC boundary and to
// hand to the workroom-upload path. It carries no asset bytes and no secrets.
export type AssetDescriptor = {
  readonly path: string
  readonly kind: AssetKind
  // Lower-cased extension without the dot, or "" when there is none.
  readonly ext: string
  // Byte size if known (from the source listing or the read), else null.
  readonly bytes: number | null
  readonly contentType: string | null
  readonly modifiedAt: string | null
  // True once readAsset succeeded for this asset during the run.
  readonly readable: boolean
  // A per-asset read error message when the read failed, else null. Always a
  // short message string — never the raw error object or a stack.
  readonly error: string | null
}

export type AssetIngestResult = {
  readonly descriptors: readonly AssetDescriptor[]
  // A run-level error (e.g. listAssets rejected). Null on a clean enumeration,
  // even when individual assets failed to read.
  readonly listError: string | null
}

const KIND_BY_EXT: Record<string, AssetKind> = {
  png: "image",
  jpg: "image",
  jpeg: "image",
  gif: "image",
  webp: "image",
  svg: "image",
  heic: "image",
  tiff: "image",
  bmp: "image",
  mp4: "video",
  mov: "video",
  webm: "video",
  mkv: "video",
  mp3: "audio",
  wav: "audio",
  aac: "audio",
  flac: "audio",
  m4a: "audio",
  pdf: "document",
  doc: "document",
  docx: "document",
  ppt: "document",
  pptx: "document",
  key: "document",
  txt: "document",
  md: "document",
  ttf: "font",
  otf: "font",
  woff: "font",
  woff2: "font",
}

export function extOf(path: string): string {
  // Strip any directory part first so a dot in a folder name can't be mistaken
  // for an extension, then take the trailing segment after the last dot.
  const base = path.slice(path.replace(/\\/g, "/").lastIndexOf("/") + 1)
  const dot = base.lastIndexOf(".")
  if (dot <= 0 || dot === base.length - 1) return ""
  return base.slice(dot + 1).toLowerCase()
}

// Default classifier: extension-driven kind. Callers may inject their own (for
// example a semantic/content classifier) via `ingestAssets({ classify })`.
export function classifyByExtension(asset: RawAsset): AssetKind {
  const ext = extOf(asset.path)
  return KIND_BY_EXT[ext] ?? "other"
}

function errMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

// Enumerate the source and produce typed, redaction-safe descriptors. The
// source and classifier are injected; nothing real is touched here.
//
// - `listAssets` rejecting => `{ descriptors: [], listError }` (no throw).
// - a per-asset `readAsset` rejecting => that descriptor gets
//   `{ readable: false, error }`; other assets still process.
export async function ingestAssets(input: {
  readonly source: AssetSource
  readonly classify?: (asset: RawAsset) => AssetKind
}): Promise<AssetIngestResult> {
  const classify = input.classify ?? classifyByExtension

  let raw: readonly RawAsset[]
  try {
    raw = await input.source.listAssets()
  } catch (e) {
    return { descriptors: [], listError: errMessage(e) }
  }

  const descriptors: AssetDescriptor[] = []
  for (const asset of raw) {
    const kind = classify(asset)
    const ext = extOf(asset.path)
    let readable = false
    let error: string | null = null
    let bytes: number | null = typeof asset.bytes === "number" ? asset.bytes : null
    try {
      const data = await input.source.readAsset(asset.path)
      readable = true
      // Prefer the actual read length when the listing didn't carry a size.
      if (bytes === null) bytes = data.byteLength
    } catch (e) {
      error = errMessage(e)
    }
    descriptors.push({
      path: asset.path,
      kind,
      ext,
      bytes,
      contentType: asset.contentType ?? null,
      modifiedAt: asset.modifiedAt ?? null,
      readable,
      error,
    })
  }

  return { descriptors, listError: null }
}
