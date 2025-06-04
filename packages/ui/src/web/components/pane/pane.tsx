import * as React from "react"
import { useDrag } from "@use-gesture/react"
import { X } from "lucide-react"
import type { Pane as PaneType } from "../../../core/types/pane.js"
import { cn } from "../../../core/utils/cn.js"
import { Button } from "../button.js"

export interface PaneProps extends PaneType {
  children?: React.ReactNode
  onMove?: (id: string, x: number, y: number) => void
  onResize?: (id: string, width: number, height: number) => void
  onClose?: (id: string) => void
  onActivate?: (id: string) => void
  className?: string
}

export const Pane = React.forwardRef<HTMLDivElement, PaneProps>(
  (
    {
      id,
      type,
      title,
      x,
      y,
      width,
      height,
      isActive,
      dismissable = true,
      children,
      onMove,
      onResize,
      onClose,
      onActivate,
      className,
      zIndex = 0,
      minimized = false,
      maximized = false,
      ...props
    },
    ref
  ) => {
    const [position, setPosition] = React.useState({ x, y })
    const [size, setSize] = React.useState({ width, height })
    const [isDragging, setIsDragging] = React.useState(false)

    // Update position when props change
    React.useEffect(() => {
      if (!isDragging) {
        setPosition({ x, y })
      }
    }, [x, y, isDragging])

    // Update size when props change
    React.useEffect(() => {
      setSize({ width, height })
    }, [width, height])

    // Handle dragging
    const bindDrag = useDrag(
      ({ active, movement: [mx, my], first, last, memo }) => {
        setIsDragging(active)
        
        if (first) {
          onActivate?.(id)
          return { x: position.x, y: position.y }
        }

        const newX = (memo as { x: number; y: number }).x + mx
        const newY = (memo as { x: number; y: number }).y + my

        setPosition({ x: newX, y: newY })

        if (last) {
          onMove?.(id, newX, newY)
        }

        return memo
      },
      {
        from: () => [position.x, position.y],
      }
    )

    if (minimized) {
      return null // Or render a minimized version
    }


    return (
      <div
        ref={ref}
        className={cn(
          "absolute bg-background border-2 border-border shadow-lg flex flex-col",
          isActive && "ring-2 ring-ring border-primary",
          maximized && "!inset-0 !w-full !h-full",
          className
        )}
        style={{
          left: maximized ? 0 : position.x,
          top: maximized ? 0 : position.y,
          width: maximized ? "100%" : size.width,
          height: maximized ? "100%" : size.height,
          zIndex: zIndex || 1,
        }}
        onClick={() => onActivate?.(id)}
        {...props}
      >
        {/* Title Bar */}
        <div
          className="flex items-center justify-between h-9 px-3 border-b border-border bg-muted/50 cursor-move select-none"
          {...bindDrag()}
        >
          <span className="text-sm font-medium font-mono truncate">{title}</span>
          {dismissable && (
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 ml-2"
              onClick={(e) => {
                e.stopPropagation()
                onClose?.(id)
              }}
            >
              <X className="h-3 w-3" />
            </Button>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto">{children}</div>
      </div>
    )
  }
)

Pane.displayName = "Pane"