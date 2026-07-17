import * as React from "react"

import { cn } from "#lib/utils"

type ContextMenuValue = Readonly<{
  open: boolean
  point: Readonly<{ x: number; y: number }>
  setOpen: (open: boolean) => void
  setPoint: (point: Readonly<{ x: number; y: number }>) => void
}>

const ContextMenuContext = React.createContext<ContextMenuValue | null>(null)

function useContextMenu(): ContextMenuValue {
  const value = React.useContext(ContextMenuContext)
  if (value === null) throw new Error("ContextMenu components require ContextMenu")
  return value
}

type ContextMenuProps = Readonly<{
  children: React.ReactNode
  open?: boolean
  onOpenChange?: (open: boolean) => void
}>

function ContextMenu({ children, open: controlledOpen, onOpenChange }: ContextMenuProps) {
  const [internalOpen, setInternalOpen] = React.useState(false)
  const [point, setPoint] = React.useState({ x: 0, y: 0 })
  const open = controlledOpen ?? internalOpen
  const setOpen = (next: boolean): void => {
    if (controlledOpen === undefined) setInternalOpen(next)
    onOpenChange?.(next)
  }
  return <ContextMenuContext.Provider value={{ open, point, setOpen, setPoint }}>
    {children}
  </ContextMenuContext.Provider>
}

type ContextMenuTriggerProps = Readonly<{
  render: React.ReactElement<React.ButtonHTMLAttributes<HTMLButtonElement>>
}>

function ContextMenuTrigger({ render }: ContextMenuTriggerProps) {
  const menu = useContextMenu()
  const props: React.ButtonHTMLAttributes<HTMLButtonElement> & { "data-slot": string } = {
    "aria-expanded": menu.open,
    "aria-haspopup": "menu",
    "data-slot": "context-menu-trigger",
    onContextMenu: event => {
      event.preventDefault()
      render.props.onContextMenu?.(event)
      menu.setPoint({ x: event.clientX, y: event.clientY })
      menu.setOpen(true)
    },
    onKeyDown: event => {
      render.props.onKeyDown?.(event)
      if (event.defaultPrevented || (event.key !== "ContextMenu" && !(event.key === "F10" && event.shiftKey))) return
      event.preventDefault()
      const rect = event.currentTarget.getBoundingClientRect()
      menu.setPoint({ x: rect.left + 12, y: rect.top + 12 })
      menu.setOpen(true)
    },
  }
  return React.cloneElement(render, props)
}

function ContextMenuContent({ className, onKeyDown, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  const menu = useContextMenu()
  const contentRef = React.useRef<HTMLDivElement>(null)
  React.useEffect(() => {
    if (menu.open) contentRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus()
  }, [menu.open])
  if (!menu.open) return null
  return <div
      {...props}
      className={cn(
        "fixed z-50 min-w-36 rounded-md bg-popover p-1 text-sm text-popover-foreground shadow-md outline-none ring-1 ring-foreground/10",
        className,
      )}
      data-slot="context-menu-content"
      onKeyDown={event => {
        onKeyDown?.(event)
        if (event.defaultPrevented) return
        if (event.key === "Escape") {
          event.preventDefault()
          menu.setOpen(false)
        }
      }}
      ref={contentRef}
      role="menu"
      style={{ left: menu.point.x, top: menu.point.y, ...props.style }}
    />
}

function ContextMenuItem({ className, onClick, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const menu = useContextMenu()
  return <button
    {...props}
    className={cn(
      "flex min-h-8 w-full cursor-default select-none items-center rounded-sm px-2 py-1.5 text-left outline-none hover:bg-muted focus:bg-muted disabled:pointer-events-none disabled:opacity-50",
      className,
    )}
    data-slot="context-menu-item"
    onClick={event => {
      onClick?.(event)
      if (!event.defaultPrevented) menu.setOpen(false)
    }}
    role="menuitem"
    type="button"
  />
}

export { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger }
