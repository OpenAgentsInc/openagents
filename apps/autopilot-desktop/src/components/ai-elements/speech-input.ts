import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { cx } from "./utils.js"

export type SpeechInputProps = {
  readonly className?: string
  readonly isListening?: boolean
  readonly label?: string
}

export const SpeechInput = ({ className, isListening = false, label = "Speech" }: SpeechInputProps): TemplateResult =>
  Button({
    className: cx("flex items-center gap-2", className),
    size: "sm",
    type: "button",
    variant: isListening ? "secondary" : "outline",
    children: html`<span>${isListening ? "O" : "o"}</span><span>${label}</span>`,
  })
