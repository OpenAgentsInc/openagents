import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Alert, AlertDescription } from "../ui/alert.js"
import { Button } from "../ui/button.js"
import { cx, type AIChildren } from "./utils.js"

export type ConfirmationProps = {
  readonly className?: string
  readonly approval?: { approved?: boolean; reason?: string } | null
  readonly state?: string
  readonly children?: AIChildren
}

export const Confirmation = ({ className, children }: ConfirmationProps): TemplateResult =>
  Alert({ className: cx("flex flex-col gap-2", className), children })

export type ConfirmationTitleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ConfirmationTitle = ({ className, children }: ConfirmationTitleProps): TemplateResult =>
  AlertDescription({ className: cx("inline", className), children })

export type ConfirmationRequestProps = {
  readonly children?: AIChildren
}

export const ConfirmationRequest = ({ children }: ConfirmationRequestProps): TemplateResult =>
  html`${children ?? ""}`

export type ConfirmationAcceptedProps = {
  readonly children?: AIChildren
}

export const ConfirmationAccepted = ({ children }: ConfirmationAcceptedProps): TemplateResult =>
  html`${children ?? ""}`

export type ConfirmationRejectedProps = {
  readonly children?: AIChildren
}

export const ConfirmationRejected = ({ children }: ConfirmationRejectedProps): TemplateResult =>
  html`${children ?? ""}`

export type ConfirmationActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ConfirmationActions = ({ className, children }: ConfirmationActionsProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-end gap-2 self-end", className)}">${children ?? ""}</div>
`

export type ConfirmationActionProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ConfirmationAction = ({ className, children }: ConfirmationActionProps): TemplateResult =>
  Button({ className: cx("h-8 px-3 text-sm", className), type: "button", children })
