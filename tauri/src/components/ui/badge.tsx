import * as React from "react"
import { cn } from "@/lib/utils"

function Badge({ className, variant = "secondary", ...props }: React.HTMLAttributes<HTMLDivElement> & { variant?: "default" | "secondary" }) {
  const base = "inline-flex items-center rounded border px-2 py-0.5 text-xs"
  const styles = variant === 'secondary'
    ? "border-[var(--border)] bg-white/5 text-[var(--secondary)]"
    : "border-[var(--border)] bg-white/10 text-foreground"
  return <div className={cn(base, styles, className)} {...props} />
}

export { Badge }

