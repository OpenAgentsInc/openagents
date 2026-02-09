import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

export type ProjectDetailsProps = {
  readonly projectId?: string | null
}

/**
 * Minimal "Project" badge for overlay parity. Renders nothing when projectId is not set.
 */
export function ProjectDetails({ projectId }: ProjectDetailsProps): TemplateResult {
  if (projectId == null || projectId === "") return html``
  return html`
    <div
      class="oa-flow-project"
      style="pointer-events:auto;position:absolute;left:14px;top:14px;border-radius:10px;border:1px solid var(--oa-flow-stroke);background:rgba(18,26,40,0.92);backdrop-filter:blur(10px);padding:8px 10px;box-shadow:0 10px 30px rgba(0,0,0,0.22);"
    >
      <span style="font-size:11px;font-weight:800;color:var(--oa-flow-muted);">Project</span>
      <span style="margin-left:8px;font-size:11px;color:var(--oa-flow-text);" title="${projectId}">${projectId}</span>
    </div>
  `
}
