# Image-capable decisions

Status: **experimental — deferred**. No shipped backend accepts image
input; this document fixes the contract a candidate must meet and records
the evaluation that produced the deferral. Nothing here makes images
available on a text endpoint, and no text-model guarantee transfers to
images by implication.

## Versioned image state

`openagents.state.image.v1` extends a decision request without changing
the text contract:

- `state` remains the instruction and context. Images ride in a separate
  bounded `images` array — they are evidence the questions judge, never
  a silent widening of `state`.
- Each image entry carries `media_type`, `bytes` (inline), and the
  caller's `digest` — a SHA-256 over the wire bytes for idempotency.
- Admission computes a second digest over the *canonical decoded form*:
  EXIF orientation applied, decoded to pixels, downsampled to the bound.
  That canonical digest — not the wire digest — is the representation
  the model saw, and it is what receipts and evaluation pin.
- Allowed media types: `image/png`, `image/jpeg`, `image/webp`.
- Bounds per call: at most 4 images, 8 MiB wire bytes each, 2048 px on
  the long edge after decode, 16 megapixels decoded total.
- Text and images compose as one decision: questions reference both, and
  the image order is part of the state a model sees.

A request carrying an unsupported media type, a corrupt payload, or an
oversized image is refused with a typed refusal. An image is never
dropped to text-only processing silently — the caller always learns the
input was rejected rather than partially judged.

## Upload and fetch boundaries

Version 1 accepts inline uploads only. Remote-URL fetch is not part of
the contract: a caller that fetches external content does so on its own
network boundary, discloses it to its own users, and sends the bytes
inline — the door treats them identically to any upload. Decoding,
orientation, and downsampling are deterministic; two admissions of the
same bytes produce the same canonical digest.

## Candidate evaluation

Every backend the service ships or integrates was surveyed:

| Backend | Input contract | Image-capable? |
| --- | --- | --- |
| Jev (TypeSafe System One) | Text `state` + typed questions | No |
| Kev (`kev-serve`) | Text state, packed prefill, pointer readout | No |
| Laya (`laya-serve`) | ModernBERT-family text encoder | No |
| Lev (`swift/lev-bridge`) | Text via Apple's on-device foundation model | No |

No candidate meets the requirement, so the capability is deferred. The
evaluation stays honest: this is a recorded no, not a roadmap promise.

## Admission requirements for a future candidate

Before any door advertises image support, a candidate must record:

- Model provenance, license, hardware and cost, artifact identity, the
  primitives it supports, and its calibration limits.
- Permitted evaluation data with separate development and locked
  partitions, label provenance, and declared abstention and failure
  cases — never reused training or hosted-model output as unquestioned
  ground truth.
- Measured task quality, calibration, refusal behavior, latency,
  throughput, and memory on that locked partition, pinned in a report.
- Capability advertisement on the published model card and admission
  through the existing tenant-binding path — an image call to a
  text-only door is refused, never degraded.

## Cross-cutting contract

- **Receipts** carry digests only — the canonical image digest stands in
  for content, exactly as request and result digests do for text today.
- **Retention** is request-scoped: image bytes are not persisted beyond
  the settle that produces the receipt.
- **SDK, CLI, and MCP** gain no image flags until a door advertises the
  capability; `oak` and `oak-mcp` send text state only.
- **Batch and review** apply the same per-item bounds; a refused image
  item reports refused rather than a partial result.
- **Review and fallback policies** treat image calls as their own
  capability — text-model review guarantees do not apply.

## Decision

**Deferred.** The contract is defined so a candidate can be evaluated
against it; none exists today. The workstream stays experimental and
unavailable — discovery, the API catalog, and client surfaces make no
image claim. Revisit when an image-capable backend passes the admission
requirements above, or when a caller's funded need justifies evaluating
a specific candidate.
