import { html } from "../../effuse/template/html"
import { Canvas } from "./canvas"
import { Edge } from "./edge"
import { Connection } from "./connection"
import {
  Node,
  NodeAction,
  NodeContent,
  NodeDescription,
  NodeFooter,
  NodeHeader,
  NodeTitle,
} from "./node"
import { Button } from "../ui/button"

export default {
  title: "ai/Elements Canvas",
}

export const Flow = {
  render: () =>
    Canvas({
      title: "Guidance Flow",
      subtitle: "Canvas / Node / Edge",
      children: html`
        <div class="relative h-[420px] w-full">
          ${Edge({
            path: "M140,80 C 220,80 220,220 300,220",
            stroke: "var(--color-ring)",
          })}
          ${Edge.Temporary({
            path: "M420,220 C 520,220 520,320 620,320",
            stroke: "var(--color-border)",
          })}
          ${Edge.Animated({
            path: "M140,80 C 240,80 240,320 360,320",
            stroke: "var(--color-primary)",
          })}
          <svg class="pointer-events-none absolute inset-0 h-full w-full">
            ${Connection({ fromX: 140, fromY: 80, toX: 300, toY: 220 })}
          </svg>

          <div class="absolute left-8 top-8">
            ${Node({
              handles: { source: true, bottom: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Turn Summary" })}
                    ${NodeDescription({ text: "Inputs" })}
                  `,
                })}
                ${NodeContent({ children: html`Collect status, diffs, tokens.` })}
              `,
            })}
          </div>

          <div class="absolute left-80 top-40">
            ${Node({
              handles: { target: true, source: true, top: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Guidance Root" })}
                    ${NodeDescription({ text: "DSPy" })}
                    ${NodeAction({ children: Button({ size: "icon-xs", variant: "ghost", children: "…" }) })}
                  `,
                })}
                ${NodeContent({ children: html`Compose policies into one decision.` })}
                ${NodeFooter({ children: html`BudgetPolicy · StopDecider` })}
              `,
            })}
          </div>

          <div class="absolute left-[620px] top-[250px]">
            ${Node({
              handles: { target: true },
              children: html`
                ${NodeHeader({ children: NodeTitle({ text: "Dispatch" }) })}
                ${NodeContent({ children: "continue / pause / stop" })}
              `,
            })}
          </div>
        </div>
      `,
    }),
}
