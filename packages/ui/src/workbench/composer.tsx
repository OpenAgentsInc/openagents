import { forwardRef, type ComponentPropsWithoutRef, type ReactElement, type ReactNode } from "react"

import { cx } from "./internal.ts"

export const DesktopComposerFrame = forwardRef<
  HTMLFormElement,
  ComponentPropsWithoutRef<"form">
>(({
  children,
  className,
  ...props
}, ref): ReactElement => <form
  {...props}
  className={cx("oa-react-composer", className)}
  data-chat-composer="true"
  data-chat-composer-form="true"
  data-en-key="shell-composer"
  ref={ref}
>{children}</form>)
DesktopComposerFrame.displayName = "DesktopComposerFrame"

export const DesktopComposerInput = ({ children }: Readonly<{ children: ReactNode }>): ReactElement =>
  <div className="oa-react-composer-input" data-en-key="shell-input">{children}</div>

export const DesktopComposerBar = ({ children }: Readonly<{ children: ReactNode }>): ReactElement =>
  <div className="oa-react-composer-bar" data-chat-composer-footer="true">{children}</div>

export type DesktopComposerButtonKind = "action" | "stop" | "submit" | "toggle"

export const DesktopComposerButton = forwardRef<
  HTMLButtonElement,
  ComponentPropsWithoutRef<"button"> & Readonly<{ kind: DesktopComposerButtonKind }>
>(({ children, className, kind, type = "button", ...props }, ref): ReactElement =>
  <button
    {...props}
    className={cx(
      "oa-react-composer-button",
      kind === "stop" && "oa-react-stop",
      kind === "submit" && "oa-react-submit",
      className,
    )}
    data-composer-button-kind={kind}
    ref={ref}
    type={type}
  >
    {children}
  </button>)
DesktopComposerButton.displayName = "DesktopComposerButton"
