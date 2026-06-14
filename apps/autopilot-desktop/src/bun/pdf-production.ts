// PDF / document-production core (#4993)
//
// COORDINATOR WIRING:
//   This module is a pure, runtime-agnostic core: it does NOT touch the OS, a
//   real PDF library, or the network directly. Both side-effecting seams (the
//   PDF renderer and the cloud proof-ref upload) are INJECTED, so the whole
//   thing is unit-testable with fakes.
//
//   To wire it into the app, `src/bun/index.ts` would expose a request RPC
//   handler — e.g. add a `producePdf` (or `produceDocument`) entry under
//   `rpc.handlers.requests` in the existing `BrowserView.defineRPC` block, and
//   declare its params/result on `DesktopRPCSchema` in `../shared/rpc`. That
//   handler stays in the Bun main process (where the control token lives), builds
//   a `PdfDocumentSpec` from the webview params, and calls `producePdfArtifact`
//   with:
//     - a LIVE `PdfRenderer` (see below), and
//     - an `uploadProofRef` seam that POSTs the proof payload to the cloud over
//       the existing loopback /command path (mirroring `deployToCloud` /
//       `submitIntent` in `pylon-control.ts`): the cloud keeps the record; the
//       desktop only produces the artifact and uploads the proof-ref. Authority
//       stays read-only / no-new-authority per AGENTS.md — this module mints no
//       tokens and asserts no new capabilities.
//
//   OWNER / RUNTIME-GATED FOLLOW-UP: the LIVE `PdfRenderer` implementation
//   (headless browser render → PDF, a system/OS PDF path, or an MCP document
//   tool) is intentionally NOT built here. It depends on a real runtime and is
//   an owner/runtime-gated follow-up. This file ships only the injectable
//   contract + the pure core, plus a fake-backed test suite.

// ── Spec ────────────────────────────────────────────────────────────────────

/**
 * Branding tokens carried into a document. Kept to a small, public-safe set —
 * colors, font family names, and a logo *reference* (never raw bytes/secrets).
 */
export type BrandingTokens = {
  readonly primaryColor: string
  readonly accentColor: string
  readonly fontFamily: string
  /** A reference (e.g. asset id / path), NOT embedded binary or a secret. */
  readonly logoRef?: string
}

export type PdfSection = {
  readonly heading: string
  readonly body: string
}

/**
 * The kind of document this core produces. These map to the desktop's
 * brand-story guide, style guide, signature system, and lead-magnet outputs.
 */
export type PdfDocumentKind =
  | "brand_story_guide"
  | "style_guide"
  | "signature_system"
  | "lead_magnet"

/** A typed, renderer-agnostic description of a document to produce. */
export type PdfDocumentSpec = {
  readonly kind: PdfDocumentKind
  readonly title: string
  readonly sections: ReadonlyArray<PdfSection>
  readonly branding: BrandingTokens
}

// ── Injected renderer seam ───────────────────────────────────────────────────

/** Bytes plus the content type the renderer produced. */
export type RenderedDocument = {
  readonly bytes: Uint8Array
  readonly contentType: string
}

/**
 * The live PDF renderer is INJECTED. Implementations may be sync or async; the
 * core awaits the result either way. A renderer may throw / reject to signal a
 * render failure — the core surfaces that cleanly (see ProduceResult).
 */
export type PdfRenderer = {
  renderDocument(spec: PdfDocumentSpec): RenderedDocument | Promise<RenderedDocument>
}

// ── Cloud proof / evidence upload seam ───────────────────────────────────────

/**
 * Redaction-safe metadata about a produced artifact. This is the ONLY shape
 * that leaves the desktop for the cloud record: no raw document bytes, no
 * secrets, no full section bodies — just structural, public-safe facts.
 */
export type ProofMetadata = {
  readonly kind: PdfDocumentKind
  readonly title: string
  readonly sectionCount: number
  readonly contentType: string
  readonly byteLength: number
  /** Lowercase hex SHA-256 of the artifact bytes — the content address. */
  readonly artifactSha256: string
  readonly producedAt: string
}

/**
 * The cloud proof / evidence-bundle upload payload. The desktop produces the
 * artifact locally and uploads this proof-ref; the cloud keeps the record.
 */
export type ProofUploadPayload = {
  readonly artifactRef: string
  readonly metadata: ProofMetadata
}

/** Result of uploading the proof-ref to the cloud. */
export type ProofUploadResult = {
  readonly accepted: boolean
  /** The cloud's record reference when accepted; null otherwise. */
  readonly evidenceRef: string | null
  readonly reason: string
}

/** Injected cloud upload seam (mirrors the loopback /command forwarders). */
export type UploadProofRef = (payload: ProofUploadPayload) => Promise<ProofUploadResult>

// ── Produce ───────────────────────────────────────────────────────────────────

export type ProducePdfInput = {
  readonly spec: PdfDocumentSpec
  readonly renderer: PdfRenderer
  readonly uploadProofRef: UploadProofRef
  /**
   * Injectable clock so output is deterministic in tests. Defaults to
   * `Date.now`. Returns epoch milliseconds.
   */
  readonly now?: () => number
}

export type ProducePdfArtifact = {
  readonly ref: string
  readonly bytes: Uint8Array
  readonly contentType: string
  readonly sha256: string
}

/**
 * Outcome of producing a document. `ok:false` carries a clean reason rather
 * than throwing, so the RPC handler / webview never sees a raw stack trace.
 */
export type ProduceResult =
  | {
      readonly ok: true
      readonly artifact: ProducePdfArtifact
      readonly proof: ProofUploadPayload
      readonly upload: ProofUploadResult
    }
  | {
      readonly ok: false
      readonly stage: "render" | "upload"
      readonly reason: string
    }

/**
 * Deterministic lowercase-hex SHA-256 of the artifact bytes. Implemented with
 * the Web Crypto `subtle` API (available in Bun and the browser); falls back to
 * a stable structural digest only if `subtle` is somehow absent, so the core
 * still returns a content address in any runtime.
 */
async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const subtle = (globalThis as { crypto?: { subtle?: SubtleCrypto } }).crypto?.subtle
  if (subtle && typeof subtle.digest === "function") {
    // Copy into a plain ArrayBuffer-backed view so the BufferSource type holds
    // regardless of whether the input is backed by a SharedArrayBuffer.
    const buf = new Uint8Array(bytes.byteLength)
    buf.set(bytes)
    const digest = await subtle.digest("SHA-256", buf)
    const view = new Uint8Array(digest)
    let out = ""
    for (const b of view) out += b.toString(16).padStart(2, "0")
    return out
  }
  // Deterministic fallback (FNV-1a over the bytes) — not cryptographic, but
  // stable and dependency-free. The live path always has subtle available.
  let h = 0x811c9dc5
  for (const b of bytes) {
    h ^= b
    h = Math.imul(h, 0x01000193) >>> 0
  }
  return `fnv1a-${h.toString(16).padStart(8, "0")}`
}

/** Build the content-addressed artifact ref. Stable for identical content. */
function artifactRefFor(kind: PdfDocumentKind, sha256: string): string {
  return `artifact:${kind}:sha256-${sha256}`
}

/**
 * Produce a finished document artifact via the injected renderer, then build
 * and upload a redaction-safe cloud proof/evidence-ref payload via the injected
 * upload seam.
 *
 * Failures are returned, not thrown: a renderer error becomes
 * `{ok:false, stage:"render"}` and an upload error becomes
 * `{ok:false, stage:"upload"}`.
 */
export async function producePdfArtifact(input: ProducePdfInput): Promise<ProduceResult> {
  const now = input.now ?? Date.now

  // 1. Render via the injected renderer. Surface any throw/reject cleanly.
  let rendered: RenderedDocument
  try {
    rendered = await input.renderer.renderDocument(input.spec)
  } catch (e) {
    return { ok: false, stage: "render", reason: e instanceof Error ? e.message : "render failed" }
  }

  const bytes = rendered.bytes
  const sha256 = await sha256Hex(bytes)
  const ref = artifactRefFor(input.spec.kind, sha256)

  const artifact: ProducePdfArtifact = {
    ref,
    bytes,
    contentType: rendered.contentType,
    sha256,
  }

  // 2. Build the redaction-safe proof payload. Note: section *bodies* and
  //    branding tokens are intentionally NOT included — only structural facts.
  const proof: ProofUploadPayload = {
    artifactRef: ref,
    metadata: {
      kind: input.spec.kind,
      title: input.spec.title,
      sectionCount: input.spec.sections.length,
      contentType: rendered.contentType,
      byteLength: bytes.byteLength,
      artifactSha256: sha256,
      producedAt: new Date(now()).toISOString(),
    },
  }

  // 3. Upload the proof-ref to the cloud. The cloud keeps the record.
  let upload: ProofUploadResult
  try {
    upload = await input.uploadProofRef(proof)
  } catch (e) {
    return { ok: false, stage: "upload", reason: e instanceof Error ? e.message : "upload failed" }
  }

  return { ok: true, artifact, proof, upload }
}
