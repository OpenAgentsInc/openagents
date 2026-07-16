import { PanelLeft } from "lucide-react"
import { forwardRef, type ComponentPropsWithoutRef, type ReactElement } from "react"

import { cx } from "./internal.ts"

export const DesktopWorkbench = ({
  children,
  className,
  railCollapsed = false,
  ...props
}: ComponentPropsWithoutRef<"div"> & Readonly<{ railCollapsed?: boolean }>): ReactElement =>
  <div
    {...props}
    className={cx("oa-react-workbench", className)}
    data-en-react-surface="true"
    data-rail-collapsed={railCollapsed ? "true" : "false"}
  >
    {children}
  </div>

export const DesktopSidebarExpand = forwardRef<HTMLButtonElement, ComponentPropsWithoutRef<"button">>(({ className, ...props }, ref): ReactElement =>
  <button {...props} className={cx("oa-react-sidebar-expand", className)} ref={ref} type="button">
    <PanelLeft aria-hidden="true" data-icon-name="Menu" />
  </button>)
DesktopSidebarExpand.displayName = "DesktopSidebarExpand"

export const DesktopRailScrim = (props: ComponentPropsWithoutRef<"button">): ReactElement =>
  <button {...props} className={cx("oa-react-rail-scrim", props.className)} type="button" />
