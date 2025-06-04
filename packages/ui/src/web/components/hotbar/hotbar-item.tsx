import * as React from "react"
import { cn } from "../../../core/utils/cn.js"
import { getModifierKey } from "../../../core/utils/os.js"

export interface HotbarItemProps {
  slotNumber: number
  onClick?: () => void
  children?: React.ReactNode
  title?: string
  isActive?: boolean
  isGhost?: boolean
  className?: string
}

export const HotbarItem: React.FC<HotbarItemProps> = ({
  slotNumber,
  onClick,
  children,
  title,
  isActive,
  isGhost,
  className,
}) => {
  const modifierPrefix = getModifierKey()
  const shortcutText = `${modifierPrefix}${slotNumber}`

  return (
    <button
      onClick={onClick}
      aria-label={title || `Hotbar slot ${slotNumber}`}
      title={!isGhost ? `${title || `Slot ${slotNumber}`} (${shortcutText})` : undefined}
      className={cn(
        "border-border/50 bg-background/70 hover:bg-accent hover:border-primary focus:ring-primary relative flex h-10 w-10 items-center justify-center border shadow-md backdrop-blur-sm transition-all duration-150 focus:ring-1 focus:outline-none sm:h-12 sm:w-12",
        isActive && "bg-primary/20 border-primary ring-primary ring-1",
        isGhost && "cursor-default opacity-30 hover:opacity-50",
        className,
      )}
      disabled={isGhost}
    >
      {children}
      {!isGhost && (
        <div className="text-muted-foreground bg-background/50 absolute right-0.5 bottom-0.5 flex items-center px-0.5 text-[0.6rem] leading-none font-mono">
          <span>{modifierPrefix}</span>
          <span>{slotNumber}</span>
        </div>
      )}
    </button>
  )
}