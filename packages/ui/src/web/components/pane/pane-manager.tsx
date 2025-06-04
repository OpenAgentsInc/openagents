import * as React from "react"
import type { Pane as PaneType } from "../../../core/types/pane.js"
import { Pane } from "./pane.js"
import { cn } from "../../../core/utils/cn.js"

export interface PaneManagerProps {
  panes: PaneType[]
  onPaneMove?: (id: string, x: number, y: number) => void
  onPaneResize?: (id: string, width: number, height: number) => void
  onPaneClose?: (id: string) => void
  onPaneActivate?: (id: string) => void
  renderPaneContent?: (pane: PaneType) => React.ReactNode
  className?: string
}

export function PaneManager({
  panes,
  onPaneMove,
  onPaneResize,
  onPaneClose,
  onPaneActivate,
  renderPaneContent,
  className,
}: PaneManagerProps) {
  // Sort panes by z-index to ensure proper rendering order
  const sortedPanes = React.useMemo(() => {
    return [...panes].sort((a, b) => (a.zIndex || 0) - (b.zIndex || 0))
  }, [panes])

  console.log('PaneManager rendering with panes:', sortedPanes)

  return (
    <div className={cn("relative w-full h-full", className)}>
      {sortedPanes.map((pane) => (
        <Pane
          key={pane.id}
          {...pane}
          onMove={onPaneMove}
          onResize={onPaneResize}
          onClose={onPaneClose}
          onActivate={onPaneActivate}
        >
          {renderPaneContent ? renderPaneContent(pane) : null}
        </Pane>
      ))}
    </div>
  )
}