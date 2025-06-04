import * as React from "react"
import { cn } from "../../../core/utils/cn.js"
import { HotbarItem } from "./hotbar-item.js"

export interface HotbarSlot {
  slotNumber: number
  icon: React.ReactNode
  title: string
  onClick?: () => void
  isActive?: boolean
  isEnabled?: boolean
}

export interface HotbarProps {
  className?: string
  slots: HotbarSlot[]
  disableHotkeys?: boolean
}

export const Hotbar: React.FC<HotbarProps> = ({ className, slots, disableHotkeys = false }) => {
  // Ensure we have 9 slots (fill with ghost items if needed)
  const fullSlots = React.useMemo(() => {
    const result: (HotbarSlot | null)[] = new Array(9).fill(null)
    slots.forEach((slot) => {
      if (slot.slotNumber >= 1 && slot.slotNumber <= 9) {
        result[slot.slotNumber - 1] = slot
      }
    })
    return result
  }, [slots])

  // Set up keyboard shortcuts
  React.useEffect(() => {
    // Skip if hotkeys are disabled
    if (disableHotkeys) return

    const handleKeyPress = (e: KeyboardEvent) => {
      // Check for modifier key (Cmd on Mac, Ctrl on others)
      const isModifierPressed = navigator.platform.includes("Mac") ? e.metaKey : e.ctrlKey
      
      if (!isModifierPressed) return

      const key = parseInt(e.key)
      if (key >= 1 && key <= 9) {
        e.preventDefault()
        const slot = fullSlots[key - 1]
        if (slot?.isEnabled !== false && slot?.onClick) {
          slot.onClick()
        }
      }
    }

    window.addEventListener("keydown", handleKeyPress)
    return () => window.removeEventListener("keydown", handleKeyPress)
  }, [fullSlots, disableHotkeys])

  return (
    <div
      className={cn(
        "bg-background/50 border-border/30 fixed bottom-4 left-1/2 z-[10000] flex -translate-x-1/2 transform space-x-1 p-1 shadow-lg backdrop-blur-sm border",
        className,
      )}
    >
      {fullSlots.map((slot, index) => {
        const slotNumber = index + 1
        if (!slot || slot.isEnabled === false) {
          return (
            <HotbarItem 
              key={slotNumber} 
              slotNumber={slotNumber} 
              isGhost
              showShortcut={!disableHotkeys}
            >
              <span className="h-5 w-5" />
            </HotbarItem>
          )
        }

        return (
          <HotbarItem
            key={slotNumber}
            slotNumber={slotNumber}
            onClick={slot.onClick!}
            title={slot.title}
            isActive={slot.isActive ?? false}
            showShortcut={!disableHotkeys}
          >
            {slot.icon}
          </HotbarItem>
        )
      })}
    </div>
  )
}