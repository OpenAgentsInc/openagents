import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type HotbarProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const Hotbar = ({ className, children }: HotbarProps): TemplateResult => html`
  <div class="${cx("hotbar", className)}" data-slot="hotbar">
    ${children ?? ""}
  </div>
`

export type HotbarItemProps = {
  readonly slot: number
  readonly label: string
  readonly shortcut?: string
  readonly icon?: string
  readonly active?: boolean
  readonly ghost?: boolean
  readonly disabled?: boolean
  readonly className?: string
  readonly title?: string
  readonly dataAction?: string
}

export const HotbarItem = ({
  slot,
  label,
  shortcut,
  icon,
  active = false,
  ghost = false,
  disabled = false,
  className,
  title,
  dataAction,
}: HotbarItemProps): TemplateResult => html`
  <button
    class="${cx(
      "hotbar-item",
      active ? "hotbar-item--active" : "",
      ghost ? "hotbar-item--ghost" : "",
      disabled ? "hotbar-item--disabled" : "",
      className
    )}"
    data-slot="hotbar-item"
    data-slot-index="${slot}"
    data-action="${dataAction ?? ""}"
    data-hotbar-slot="${slot}"
    title="${title ?? label}"
    type="button"
    ${disabled ? "disabled" : ""}
  >
    <span class="hotbar-item__index">${slot}</span>
    ${icon ? html`<span class="hotbar-item__icon">${icon}</span>` : ""}
    <span class="hotbar-item__label">${label}</span>
    ${shortcut ? html`<span class="hotbar-item__shortcut">${shortcut}</span>` : ""}
  </button>
`
