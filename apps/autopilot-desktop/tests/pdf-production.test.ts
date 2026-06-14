import { describe, expect, test } from "bun:test"
import {
  producePdfArtifact,
  type PdfDocumentSpec,
  type PdfRenderer,
  type ProofUploadPayload,
  type ProofUploadResult,
  type RenderedDocument,
} from "../src/bun/pdf-production"

// ── Fakes ─────────────────────────────────────────────────────────────────────

const spec: PdfDocumentSpec = {
  kind: "brand_story_guide",
  title: "OpenAgents Brand Story",
  sections: [
    { heading: "Origin", body: "The machine-work economy begins here." },
    { heading: "Voice", body: "Direct, technical, honest." },
  ],
  branding: {
    primaryColor: "#0b0b0f",
    accentColor: "#7c5cff",
    fontFamily: "Inter",
    logoRef: "asset:logo-dark",
  },
}

/** Renderer that returns fixed bytes; records how many times it was called. */
function fakeRenderer(bytes: Uint8Array, contentType = "application/pdf"): PdfRenderer & { calls: number } {
  const r = {
    calls: 0,
    renderDocument(_spec: PdfDocumentSpec): RenderedDocument {
      r.calls++
      return { bytes, contentType }
    },
  }
  return r
}

/** Upload seam that captures the payload and returns a canned result. */
function fakeUpload(result: ProofUploadResult): {
  fn: (p: ProofUploadPayload) => Promise<ProofUploadResult>
  captured: ProofUploadPayload[]
} {
  const captured: ProofUploadPayload[] = []
  return {
    captured,
    fn: async (p) => {
      captured.push(p)
      return result
    },
  }
}

const FIXED_NOW = () => Date.parse("2026-06-14T12:00:00.000Z")
const accepted: ProofUploadResult = { accepted: true, evidenceRef: "evidence:rec-1", reason: "ok" }

const bytes = new Uint8Array([0x25, 0x50, 0x44, 0x46, 0x2d, 0x31, 0x2e, 0x37]) // "%PDF-1.7"

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("producePdfArtifact (#4993)", () => {
  test("renders via the injected renderer and produces an artifact", async () => {
    const renderer = fakeRenderer(bytes)
    const upload = fakeUpload(accepted)

    const res = await producePdfArtifact({ spec, renderer, uploadProofRef: upload.fn, now: FIXED_NOW })

    expect(res.ok).toBe(true)
    if (!res.ok) throw new Error("expected ok")
    expect(renderer.calls).toBe(1)
    expect(res.artifact.bytes).toEqual(bytes)
    expect(res.artifact.contentType).toBe("application/pdf")
    expect(res.artifact.ref).toBe(res.proof.artifactRef)
    expect(res.artifact.ref).toContain("brand_story_guide")
    expect(res.artifact.ref).toContain(res.artifact.sha256)
  })

  test("proof payload has the expected shape and structural metadata", async () => {
    const upload = fakeUpload(accepted)
    const res = await producePdfArtifact({
      spec,
      renderer: fakeRenderer(bytes),
      uploadProofRef: upload.fn,
      now: FIXED_NOW,
    })

    expect(res.ok).toBe(true)
    if (!res.ok) throw new Error("expected ok")
    expect(upload.captured.length).toBe(1)
    const payload = upload.captured[0]!
    expect(payload.artifactRef).toBe(res.artifact.ref)
    expect(payload.metadata).toEqual({
      kind: "brand_story_guide",
      title: "OpenAgents Brand Story",
      sectionCount: 2,
      contentType: "application/pdf",
      byteLength: 8,
      artifactSha256: res.artifact.sha256,
      producedAt: "2026-06-14T12:00:00.000Z",
    })
    expect(res.upload).toEqual(accepted)
  })

  test("proof metadata is redaction-safe: no raw bytes, no section bodies, no branding secrets", async () => {
    const upload = fakeUpload(accepted)
    await producePdfArtifact({ spec, renderer: fakeRenderer(bytes), uploadProofRef: upload.fn, now: FIXED_NOW })

    const payload = upload.captured[0]!
    const serialized = JSON.stringify(payload)

    // No raw document content leaks: section bodies must be absent.
    expect(serialized).not.toContain("machine-work economy")
    expect(serialized).not.toContain("Direct, technical, honest")
    // Branding tokens (colors / font / logo ref) must not ride along.
    expect(serialized).not.toContain("#7c5cff")
    expect(serialized).not.toContain("asset:logo-dark")
    expect(serialized).not.toContain("Inter")
    // The payload must not embed the raw bytes (only a hash + length).
    expect(payload.metadata).not.toHaveProperty("bytes")
    expect(payload).not.toHaveProperty("bytes")
    // It MUST carry the content address + length.
    expect(payload.metadata.byteLength).toBe(8)
    expect(typeof payload.metadata.artifactSha256).toBe("string")
    expect(payload.metadata.artifactSha256.length).toBeGreaterThan(0)
  })

  test("a renderer error is surfaced cleanly as stage:render (no throw)", async () => {
    const failing: PdfRenderer = {
      renderDocument() {
        throw new Error("headless render crashed")
      },
    }
    const upload = fakeUpload(accepted)

    const res = await producePdfArtifact({ spec, renderer: failing, uploadProofRef: upload.fn, now: FIXED_NOW })

    expect(res.ok).toBe(false)
    if (res.ok) throw new Error("expected failure")
    expect(res.stage).toBe("render")
    expect(res.reason).toBe("headless render crashed")
    // Upload must NOT be attempted when render fails.
    expect(upload.captured.length).toBe(0)
  })

  test("an upload error is surfaced cleanly as stage:upload (no throw)", async () => {
    const res = await producePdfArtifact({
      spec,
      renderer: fakeRenderer(bytes),
      uploadProofRef: async () => {
        throw new Error("cloud unreachable")
      },
      now: FIXED_NOW,
    })

    expect(res.ok).toBe(false)
    if (res.ok) throw new Error("expected failure")
    expect(res.stage).toBe("upload")
    expect(res.reason).toBe("cloud unreachable")
  })

  test("a rejected upload (accepted:false) still returns ok:true with the cloud's reason", async () => {
    const rejected: ProofUploadResult = { accepted: false, evidenceRef: null, reason: "quota_exceeded" }
    const res = await producePdfArtifact({
      spec,
      renderer: fakeRenderer(bytes),
      uploadProofRef: fakeUpload(rejected).fn,
      now: FIXED_NOW,
    })

    expect(res.ok).toBe(true)
    if (!res.ok) throw new Error("expected ok")
    // Producing succeeded; the cloud declined the record — surfaced, not hidden.
    expect(res.upload.accepted).toBe(false)
    expect(res.upload.reason).toBe("quota_exceeded")
    expect(res.upload.evidenceRef).toBeNull()
  })

  test("output is deterministic: same spec + bytes ⇒ same ref + sha + proof", async () => {
    const run = () =>
      producePdfArtifact({
        spec,
        renderer: fakeRenderer(bytes),
        uploadProofRef: fakeUpload(accepted).fn,
        now: FIXED_NOW,
      })

    const a = await run()
    const b = await run()
    expect(a.ok && b.ok).toBe(true)
    if (!a.ok || !b.ok) throw new Error("expected ok")
    expect(a.artifact.sha256).toBe(b.artifact.sha256)
    expect(a.artifact.ref).toBe(b.artifact.ref)
    expect(a.proof).toEqual(b.proof)
  })

  test("different content ⇒ different content address (sha + ref)", async () => {
    const a = await producePdfArtifact({
      spec,
      renderer: fakeRenderer(new Uint8Array([1, 2, 3])),
      uploadProofRef: fakeUpload(accepted).fn,
      now: FIXED_NOW,
    })
    const b = await producePdfArtifact({
      spec,
      renderer: fakeRenderer(new Uint8Array([4, 5, 6])),
      uploadProofRef: fakeUpload(accepted).fn,
      now: FIXED_NOW,
    })
    expect(a.ok && b.ok).toBe(true)
    if (!a.ok || !b.ok) throw new Error("expected ok")
    expect(a.artifact.sha256).not.toBe(b.artifact.sha256)
    expect(a.artifact.ref).not.toBe(b.artifact.ref)
  })

  test("async renderers are awaited", async () => {
    const asyncRenderer: PdfRenderer = {
      async renderDocument() {
        return { bytes, contentType: "application/pdf" }
      },
    }
    const res = await producePdfArtifact({
      spec,
      renderer: asyncRenderer,
      uploadProofRef: fakeUpload(accepted).fn,
      now: FIXED_NOW,
    })
    expect(res.ok).toBe(true)
  })
})
