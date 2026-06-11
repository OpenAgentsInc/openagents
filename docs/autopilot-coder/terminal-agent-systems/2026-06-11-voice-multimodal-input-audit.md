# Voice And Multimodal Input Audit

Date: 2026-06-11

This is system #38 from the Bun/Effect terminal-agent systems list. It defines
how OpenAgents should accept speech, images, screenshots, files, and other
non-text input without weakening the same runtime, approval, redaction, and
receipt rules used for text turns.

## Target

Build a multimodal input layer that turns raw user media into typed runtime
events, bounded context refs, and user-reviewable attachments.

Voice and image input should be optional. A terminal coding agent must still
work in text-only, headless, and non-interactive modes.

## User-Visible Capability

Users should be able to:

- Dictate a prompt or follow-up instruction.
- Attach screenshots, clipboard images, PDFs, text files, logs, and structured
  documents.
- See whether transcription, image understanding, and file extraction are
  available for the current provider.
- Review or edit transcribed text before it is submitted.
- Remove an attachment before it enters model context.
- Keep raw media local unless they explicitly choose a model or service path
  that requires upload.
- Resume a run with attachment refs preserved.

The UI should clearly separate "captured", "processed", "submitted", and
"included in context". Capturing an image or recording audio is not permission
to send it to a model.

## Core Contract

The runtime should model multimodal input as:

- A local capture record.
- A media artifact ref.
- A redaction and visibility classification.
- A processing plan.
- A normalized context part.
- A user or policy approval record when upload is required.
- A deletion eligibility timestamp.

Transcription output is derived text. It should carry a confidence and
provenance ref so the user can correct mistakes before it becomes instruction
authority.

## Bun/Effect Boundary

Use Effect services for:

- `MultimodalInputService`: capture, import, classify, and normalize input.
- `SpeechBoundaryService`: voice activity, recording, transcription, and
  review state.
- `MediaArtifactService`: private local media refs, digests, preview metadata,
  and cleanup.
- `AttachmentContextService`: converts approved media into bounded context
  parts.
- `MultimodalPolicyService`: decides what can be processed locally, uploaded,
  retained, or projected.

Use Schema for media kinds, capture channels, processing status, visibility,
provider capability, and deletion state. Use Stream for audio chunks and
transcription deltas. Use Scope for temporary file cleanup and microphone
handles.

## Trust Boundaries

Raw media is private by default. Generated summaries may become public only
after a public-safe projection scan.

High-risk input includes:

- Screenshots containing credentials or customer data.
- Clipboard images from private apps.
- Voice recordings with bystanders.
- Documents containing private repo, wallet, provider, or customer details.
- Logs that contain tokens or full prompts.

The model should receive only the minimum approved representation needed for
the turn. Raw files should be referenced by artifact refs, not embedded into
transcripts.

## Approval Rules

Require explicit approval before:

- Uploading raw audio or image data to a provider.
- Sending an attachment from outside the active workspace.
- Including a screenshot from a browser, desktop, or private app.
- Persisting raw media beyond the current session.
- Sharing any media-derived ref outside the local user boundary.

Unattended runs may process already-approved refs under their declared policy.
They may not start recording, read a new clipboard image, or capture the
desktop.

## OpenAgents Translation Notes

As of 2026-06-11, the OpenAgents terminal-agent README has no imported voice or
multimodal audit. OpenAgents has attachment-heavy product workrooms and
artifact/redaction policy in adjacent surfaces, but the terminal runtime does
not yet have a single multimodal input service contract.

Related open issue anchors:

- #4773 API parity contract: every browser capability needs an agent-API peer.
- #4765 decision queue and notifications: voice input should not bypass normal
  approval state.
- #4769 repo connect and per-mission data-scope UX: attachment scope should be
  shown with the same data-boundary language.

No live claim should say terminal voice, transcription, screenshot ingestion,
or multimodal context is shipped until a receipt proves capture, review,
redaction, and deletion behavior.

## Tests

Minimum coverage:

- Import each supported media kind and reject unsupported kinds.
- Keep raw media private by default.
- Require approval before provider upload.
- Preserve and restore reviewed transcription text.
- Enforce deletion of temporary capture files.
- Redact sensitive patterns from previews and summaries.
- Verify context-window caps for large attachments.
- Confirm non-interactive mode rejects capture and accepts predeclared refs.

## Decision

Voice and multimodal input should be an attachment-to-context pipeline, not an
alternate instruction channel. The typed runtime event is authoritative; raw
media is private evidence until a policy explicitly promotes a derived,
public-safe ref.
