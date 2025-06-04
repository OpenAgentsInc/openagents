import * as React from "react"
import { createPortal } from "react-dom"

interface PanePortalProps {
  children: React.ReactNode
}

export function PanePortal({ children }: PanePortalProps) {
  const [mounted, setMounted] = React.useState(false)

  React.useEffect(() => {
    setMounted(true)
    return () => setMounted(false)
  }, [])

  if (!mounted) {
    return null
  }

  return createPortal(children, document.body)
}