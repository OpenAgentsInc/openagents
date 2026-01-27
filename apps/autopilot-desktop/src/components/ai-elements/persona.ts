import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type PersonaState = "idle" | "listening" | "thinking" | "speaking" | "asleep"

export type PersonaProps = {
  readonly state: PersonaState
  readonly className?: string
  readonly variant?: string
}

export const Persona = ({ state, className, variant = "obsidian" }: PersonaProps): TemplateResult => html`
  <div class="${cx("flex flex-col items-center justify-center gap-2", className)}">
    <div class="size-16 rounded-full border border-border bg-muted/40"></div>
    <div class="text-xs uppercase tracking-wide text-muted-foreground">${variant}</div>
    <div class="text-xs text-foreground">${state}</div>
  </div>
`
