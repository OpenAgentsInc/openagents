import { html } from "../template/html.js"
import type { TemplateResult } from "../template/types.js"
import type { BlobRefLike, BoundedText, ToolPartDetails, ToolPartModel } from "./types.js"

export type RenderToolPartOptions = {
  /**
   * EZ action name used by the "View full" affordance.
   *
   * The app is responsible for registering this action in its EzRegistry.
   */
  readonly blobViewAction?: string
  readonly blobViewLabel?: string
}

const sanitizeDomId = (value: string): string =>
  value.replace(/[^a-zA-Z0-9_-]+/g, "_").slice(0, 80)

const slotIdFor = (toolCallId: string, kind: string, blob: BlobRefLike): string =>
  `effuse-blob-${sanitizeDomId(toolCallId)}-${sanitizeDomId(kind)}-${sanitizeDomId(blob.id)}`

const blobValsJson = (blob: BlobRefLike): string =>
  JSON.stringify({
    blobId: blob.id,
    hash: blob.hash,
    size: blob.size,
    ...(blob.mime ? { mime: blob.mime } : {}),
  })

const renderBounded = (
  toolCallId: string,
  kind: "input" | "output" | "error",
  value: BoundedText,
  options: Required<Pick<RenderToolPartOptions, "blobViewAction" | "blobViewLabel">>
): TemplateResult => {
  const blob = value.blob
  if (!blob) {
    return html`<pre data-effuse-tool-${kind}="1">${value.preview}</pre>`
  }

  const slotId = slotIdFor(toolCallId, kind, blob)
  const vals = blobValsJson(blob)

  return html`
    <div data-effuse-tool-${kind}="1">
      <pre id="${slotId}">${value.preview}</pre>
      <button
        type="button"
        data-ez="${options.blobViewAction}"
        data-ez-target="#${slotId}"
        data-ez-swap="inner"
        data-ez-disable
        data-ez-vals="${vals}"
        data-effuse-blob-id="${blob.id}"
        data-effuse-blob-hash="${blob.hash}"
        data-effuse-blob-size="${String(blob.size)}"
        data-effuse-blob-mime="${blob.mime ?? ""}"
      >
        ${options.blobViewLabel}
      </button>
    </div>
  `
}

const renderDetails = (
  model: ToolPartModel,
  details: ToolPartDetails,
  options: Required<Pick<RenderToolPartOptions, "blobViewAction" | "blobViewLabel">>
): TemplateResult => {
  return html`
    <div data-effuse-tool-details="1">
      ${details.extra ?? null}
      ${details.input
        ? html`
            <div data-effuse-tool-field="input">
              <div data-effuse-tool-label="1">Input</div>
              ${renderBounded(model.toolCallId, "input", details.input, options)}
            </div>
          `
        : null}
      ${details.output
        ? html`
            <div data-effuse-tool-field="output">
              <div data-effuse-tool-label="1">Output</div>
              ${renderBounded(model.toolCallId, "output", details.output, options)}
            </div>
          `
        : null}
      ${details.error
        ? html`
            <div data-effuse-tool-field="error">
              <div data-effuse-tool-label="1">Error</div>
              ${renderBounded(model.toolCallId, "error", details.error, options)}
            </div>
          `
        : null}
    </div>
  `
}

/**
 * Default tool-part renderer implementing the minimal UI schema from
 * `MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md` §3.6.2.
 *
 * - Always renders a visible card for tool parts
 * - Always includes toolName + toolCallId
 * - Renders details behind <details>
 * - Supports BlobRef-backed "View full" affordance
 */
export const renderToolPart = (
  model: ToolPartModel,
  options?: RenderToolPartOptions
): TemplateResult => {
  const opts = {
    blobViewAction: options?.blobViewAction ?? "effuse.blob.view",
    blobViewLabel: options?.blobViewLabel ?? "View full",
  }

  const details = model.details
    ? renderDetails(model, model.details, opts)
    : null

  return html`
    <details
      data-effuse-tool-part="1"
      data-effuse-tool-status="${model.status}"
      data-effuse-tool-name="${model.toolName}"
      data-effuse-tool-call-id="${model.toolCallId}"
    >
      <summary data-effuse-tool-summary="1">
        <span data-effuse-tool-status-badge="1">${model.status}</span>
        <span data-effuse-tool-name-label="1">${model.toolName}</span>
        <span data-effuse-tool-call-id-label="1">${model.toolCallId}</span>
        <span data-effuse-tool-summary-text="1">${model.summary}</span>
      </summary>
      ${details}
    </details>
  `
}
