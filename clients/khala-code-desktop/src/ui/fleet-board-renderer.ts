import {
  renderArbiterGraphHtml,
  type ArbiterGraphRenderOptions,
  type ArbiterGraphRenderOutput,
} from "@openagentsinc/arbiter-effect/foldkit"

import type {
  KhalaFleetBoardProjection,
  KhalaFleetTimelineEvent,
} from "./fleet-board-projection"

export type KhalaFleetBoardRenderOptions = ArbiterGraphRenderOptions

export type KhalaFleetBoardRenderOutput = Readonly<{
  html: string
  graph: ArbiterGraphRenderOutput
  timelineHtml: string
}>

const escapeHtml = (value: string | number | boolean): string =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")

const formatSlotSummary = (projection: KhalaFleetBoardProjection): string => {
  const { availableCodexAssignments, maxCodexAssignments } = projection.summary
  if (availableCodexAssignments === null || maxCodexAssignments === null) {
    return "capacity pending"
  }
  return `${availableCodexAssignments}/${maxCodexAssignments} slots free`
}

const renderSummary = (projection: KhalaFleetBoardProjection): string => [
  '<div class="khala-fleet-board-summary" role="list" aria-label="Fleet board summary">',
  `<span class="khala-fleet-board-stat" role="listitem"><span>Workers</span><strong>${escapeHtml(projection.summary.readyAccounts)}/${escapeHtml(projection.summary.totalAccounts)}</strong></span>`,
  `<span class="khala-fleet-board-stat" role="listitem"><span>Capacity</span><strong>${escapeHtml(formatSlotSummary(projection))}</strong></span>`,
  `<span class="khala-fleet-board-stat" role="listitem"><span>Runs</span><strong>${escapeHtml(projection.summary.activeAssignments)}</strong></span>`,
  `<span class="khala-fleet-board-stat" role="listitem"><span>Exec</span><strong>${escapeHtml(projection.summary.runningProcesses)}</strong></span>`,
  "</div>",
].join("")

const timelineRefs = (event: KhalaFleetTimelineEvent): string => {
  const refs = event.evidenceRefs.length > 0 ? event.evidenceRefs : event.blockerRefs
  if (refs.length === 0) return ""
  return `<span class="khala-fleet-timeline-refs">${escapeHtml(refs.join(" "))}</span>`
}

const renderTimelineEvent = (event: KhalaFleetTimelineEvent): string => [
  `<li class="khala-fleet-timeline-event" data-status="${escapeHtml(event.status)}">`,
  '<span class="khala-fleet-timeline-marker" aria-hidden="true"></span>',
  '<div class="khala-fleet-timeline-card">',
  '<div class="khala-fleet-timeline-topline">',
  `<strong>${escapeHtml(event.label)}</strong>`,
  `<span class="khala-fleet-timeline-time">${escapeHtml(event.observedAt)}</span>`,
  "</div>",
  `<p>${escapeHtml(event.detail)}</p>`,
  `<span class="khala-fleet-timeline-subject">${escapeHtml(event.subjectRef)}</span>`,
  timelineRefs(event),
  "</div>",
  "</li>",
].join("")

const renderTimeline = (
  projection: KhalaFleetBoardProjection,
): string => [
  '<section class="khala-fleet-timeline" aria-label="Run timeline">',
  '<div class="khala-fleet-board-heading">',
  '<h3 class="khala-fleet-section-title">Run timeline</h3>',
  `<span class="khala-fleet-section-meta">${escapeHtml(projection.timeline.length)} events</span>`,
  "</div>",
  `<ol class="khala-fleet-timeline-list">${projection.timeline.map(renderTimelineEvent).join("")}</ol>`,
  "</section>",
].join("")

export const renderKhalaFleetBoardHtml = (
  projection: KhalaFleetBoardProjection,
  options: KhalaFleetBoardRenderOptions = {},
): KhalaFleetBoardRenderOutput => {
  const graph = renderArbiterGraphHtml(projection.graph, options)
  const timelineHtml = renderTimeline(projection)
  const html = [
    `<section class="khala-fleet-board" data-fleet-board="${escapeHtml(projection.schemaVersion)}" data-status="${escapeHtml(projection.graph.status)}" aria-label="Fleet board graph and run timeline">`,
    '<div class="khala-fleet-board-heading">',
    '<h3 class="khala-fleet-section-title">Fleet board</h3>',
    `<span class="khala-fleet-section-meta">${escapeHtml(projection.generatedAt)}</span>`,
    "</div>",
    renderSummary(projection),
    graph.html,
    timelineHtml,
    "</section>",
  ].join("")

  return { html, graph, timelineHtml }
}
