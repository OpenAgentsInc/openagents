import { rawHtml } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

// Minimal, dependency-free styling for the flow graph system.
// Consumers can override via CSS variables or by providing their own styles.
export const FLOW_STYLES_CSS = `
:root {
  --oa-flow-bg: #0b0f17;
  --oa-flow-panel: rgba(255, 255, 255, 0.06);
  --oa-flow-panel2: rgba(255, 255, 255, 0.09);
  --oa-flow-text: #e6edf7;
  --oa-flow-muted: #9db0cc;
  --oa-flow-stroke: rgba(255, 255, 255, 0.12);
  --oa-flow-connection-stroke: rgba(255, 255, 255, 0.16);
  --oa-flow-ok-stroke: rgba(61, 220, 151, 0.45);
  --oa-flow-bad-stroke: rgba(255, 92, 122, 0.55);
}

.oa-flow-canvas {
  position: relative;
  width: 100%;
  height: 100%;
}

.oa-flow-canvas__svg {
  width: 100%;
  height: 100%;
  display: block;
  background: var(--oa-flow-bg);
  cursor: grab;
}

.oa-flow-canvas[data-oa-flow-panning="1"] .oa-flow-canvas__svg {
  cursor: grabbing;
}

.oa-flow-canvas__overlay {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.oa-flow-node-wrap {
  user-select: none;
}

.oa-flow-node {
  color: var(--oa-flow-text);
  background: var(--oa-flow-panel);
  border: 1px solid var(--oa-flow-stroke);
  border-radius: 10px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.22);
}

.oa-flow-node[data-selected="1"] {
  border-color: rgba(255, 255, 255, 0.28);
  box-shadow:
    0 10px 30px rgba(0, 0, 0, 0.22),
    0 0 0 2px rgba(255, 255, 255, 0.08);
}

.oa-flow-node--root {
  width: 140px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 12px;
  font-weight: 700;
  font-size: 13px;
  letter-spacing: 0.2px;
}

.oa-flow-node--leaf {
  width: 180px;
  height: 56px;
  padding: 8px 12px;
}

.oa-flow-node__row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.oa-flow-node__main {
  flex: 1;
  min-width: 0;
}

.oa-flow-node__title {
  font-weight: 700;
  font-size: 13px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.oa-flow-node__subtitle {
  margin-top: 2px;
  font-size: 11px;
  line-height: 1.2;
  color: var(--oa-flow-muted);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.oa-flow-node__aside {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 6px;
}

.oa-flow-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  display: inline-block;
  background: rgba(255, 255, 255, 0.35);
}
.oa-flow-status-dot[data-status="ok"] { background: rgba(255, 255, 255, 0.4); }
.oa-flow-status-dot[data-status="live"] { background: rgb(56, 189, 248); }
.oa-flow-status-dot[data-status="running"] { background: rgb(52, 211, 153); }
.oa-flow-status-dot[data-status="pending"] { background: rgb(245, 158, 11); }
.oa-flow-status-dot[data-status="error"] { background: rgb(248, 113, 113); }

.oa-flow-pill {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  border: 1px solid var(--oa-flow-stroke);
  padding: 2px 8px;
  font-weight: 700;
  font-size: 10px;
  letter-spacing: 0.02em;
  line-height: 1;
  color: var(--oa-flow-muted);
  background: rgba(255, 255, 255, 0.02);
}

.oa-flow-pill[data-tone="info"] {
  border-color: rgba(56, 189, 248, 0.35);
  color: rgba(56, 189, 248, 0.9);
  background: rgba(56, 189, 248, 0.10);
}
.oa-flow-pill[data-tone="success"] {
  border-color: rgba(52, 211, 153, 0.35);
  color: rgba(52, 211, 153, 0.95);
  background: rgba(52, 211, 153, 0.10);
}
.oa-flow-pill[data-tone="warning"] {
  border-color: rgba(245, 158, 11, 0.35);
  color: rgba(245, 158, 11, 0.95);
  background: rgba(245, 158, 11, 0.10);
}
.oa-flow-pill[data-tone="destructive"] {
  border-color: rgba(248, 113, 113, 0.35);
  color: rgba(248, 113, 113, 0.95);
  background: rgba(248, 113, 113, 0.10);
}

.oa-flow-pill--tiny {
  font-size: 10px;
  padding: 2px 7px;
}

.oa-flow-node--skeleton {
  background: rgba(255, 255, 255, 0.04);
}

.oa-flow-skeleton-line {
  height: 12px;
  border-radius: 8px;
  background: linear-gradient(
    90deg,
    rgba(255, 255, 255, 0.06),
    rgba(255, 255, 255, 0.14),
    rgba(255, 255, 255, 0.06)
  );
  background-size: 200% 100%;
  animation: oa-flow-skeleton 1.6s ease-in-out infinite;
}
.oa-flow-skeleton-line--a { width: 120px; margin-top: 6px; }
.oa-flow-skeleton-line--b { width: 86px; margin-top: 10px; opacity: 0.9; }

@keyframes oa-flow-skeleton {
  0% { background-position: 0% 0%; }
  50% { background-position: 100% 0%; }
  100% { background-position: 0% 0%; }
}

.oa-flow-dev-btn {
  appearance: none;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(255, 255, 255, 0.03);
  color: var(--oa-flow-text);
  border-radius: 10px;
  padding: 6px 10px;
  font-size: 11px;
  font-weight: 800;
  cursor: pointer;
}
.oa-flow-dev-btn:hover {
  background: rgba(255, 255, 255, 0.06);
}

.oa-flow-details {
  position: absolute;
  top: 14px;
  right: 14px;
  width: 320px;
  pointer-events: auto;
  border-radius: 12px;
  border: 1px solid var(--oa-flow-stroke);
  background: rgba(18, 26, 40, 0.92);
  backdrop-filter: blur(10px);
  box-shadow: 0 16px 50px rgba(0, 0, 0, 0.35);
  overflow: hidden;
}

.oa-flow-details__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.oa-flow-details__title {
  font-weight: 800;
  font-size: 13px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.oa-flow-details__body {
  padding: 12px 14px;
}

.oa-flow-details__row {
  display: flex;
  gap: 10px;
  padding: 6px 0;
  font-size: 12px;
  line-height: 1.2;
}

.oa-flow-details__label {
  width: 80px;
  color: var(--oa-flow-muted);
  font-weight: 700;
}

.oa-flow-details__value {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.oa-flow-details__close {
  border: 0;
  background: transparent;
  color: var(--oa-flow-muted);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
  padding: 0 6px;
}
.oa-flow-details__close:hover { color: var(--oa-flow-text); }

.oa-flow-grid-dot {
  fill: rgba(255, 255, 255, 0.16);
}
`

export const FlowStyles = (): TemplateResult =>
  rawHtml(`<style data-oa-flow-styles="1">${FLOW_STYLES_CSS}</style>`)
