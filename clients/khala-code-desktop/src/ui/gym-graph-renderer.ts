import {
  renderArbiterGraphHtml,
  type ArbiterGraphRenderOptions,
  type ArbiterGraphRenderOutput,
} from "@openagentsinc/arbiter-effect/foldkit"

import type { KhalaGymGraphProjection } from "./gym-graph-projection"

export type KhalaGymGraphRenderOptions = ArbiterGraphRenderOptions
export type KhalaGymGraphRenderOutput = ArbiterGraphRenderOutput

export const renderKhalaGymGraphHtml = (
  projection: KhalaGymGraphProjection,
  options: KhalaGymGraphRenderOptions = {},
): KhalaGymGraphRenderOutput => renderArbiterGraphHtml(projection, options)
