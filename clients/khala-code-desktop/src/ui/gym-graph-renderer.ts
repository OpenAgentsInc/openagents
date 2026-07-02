import {
  arbiterGraphFigure,
  renderArbiterGraphHtml,
  type ArbiterGraphFoldkitInput,
  type ArbiterGraphRenderOptions,
  type ArbiterGraphRenderOutput,
} from "@openagentsinc/arbiter-effect/foldkit"
import type { GraphSpec } from "@openagentsinc/arbiter-effect/core"
import type { Html } from "foldkit/html"

import type { KhalaGymGraphProjection } from "./gym-graph-projection"

export type KhalaGymGraphRenderOptions = ArbiterGraphRenderOptions
export type KhalaGymGraphRenderOutput = ArbiterGraphRenderOutput
export type KhalaGymGraphFigureInput<Message> =
  Omit<ArbiterGraphFoldkitInput<Message>, "spec"> & Readonly<{
    projection: GraphSpec
  }>

export const renderKhalaGymGraphHtml = (
  projection: KhalaGymGraphProjection,
  options: KhalaGymGraphRenderOptions = {},
): KhalaGymGraphRenderOutput => renderArbiterGraphHtml(projection, options)

export const khalaGymGraphFigure = <Message>(
  input: KhalaGymGraphFigureInput<Message>,
): Html =>
  arbiterGraphFigure<Message>({
    spec: input.projection,
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.options === undefined ? {} : { options: input.options }),
  })
