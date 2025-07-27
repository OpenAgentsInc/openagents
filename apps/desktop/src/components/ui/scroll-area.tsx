import * as React from "react"
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area"

import { cn } from "@/lib/utils"

// Type assertions for Radix UI components to fix TypeScript strict JSX checking
const ScrollAreaRoot = ScrollAreaPrimitive.Root as any;
const ScrollAreaViewport = ScrollAreaPrimitive.Viewport as any;
const ScrollAreaScrollbar = ScrollAreaPrimitive.Scrollbar as any;
const ScrollAreaThumb = ScrollAreaPrimitive.Thumb as any;
const ScrollAreaCorner = ScrollAreaPrimitive.Corner as any;

const ScrollArea = React.forwardRef<
  React.ElementRef<typeof ScrollAreaPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.Root>
>(({ className, children, ...props }, ref) => (
  <ScrollAreaRoot
    ref={ref}
    data-slot="scroll-area"
    className={cn("relative", className)}
    {...props}
  >
    <ScrollAreaViewport
      data-slot="scroll-area-viewport"
      className="focus-visible:ring-ring/50 size-full rounded-[inherit] transition-[color,box-shadow] outline-none focus-visible:ring-[3px] focus-visible:outline-1"
    >
      {children}
    </ScrollAreaViewport>
    <ScrollBar />
    <ScrollAreaCorner />
  </ScrollAreaRoot>
))
ScrollArea.displayName = ScrollAreaPrimitive.Root.displayName

const ScrollBar = React.forwardRef<
  React.ElementRef<typeof ScrollAreaPrimitive.Scrollbar>,
  React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.Scrollbar>
>(({ className, orientation = "vertical", ...props }, ref) => (
  <ScrollAreaScrollbar
    ref={ref}
    data-slot="scroll-area-scrollbar"
    orientation={orientation}
    className={cn(
      "flex touch-none p-px transition-colors select-none",
      orientation === "vertical" &&
        "h-full w-2.5 border-l border-l-transparent",
      orientation === "horizontal" &&
        "h-2.5 flex-col border-t border-t-transparent",
      className
    )}
    {...props}
  >
    <ScrollAreaThumb
      data-slot="scroll-area-thumb"
      className="bg-border relative flex-1 rounded-full"
    />
  </ScrollAreaScrollbar>
))
ScrollBar.displayName = ScrollAreaPrimitive.Scrollbar.displayName

export { ScrollArea, ScrollBar }
