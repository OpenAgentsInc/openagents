import { useTheme } from "@/components/theme-provider"
import { Toaster as Sonner, ToasterProps } from "sonner"

const Toaster = ({ ...props }: ToasterProps) => {
  const { resolvedTheme } = useTheme()

  return (
    <Sonner
      theme={resolvedTheme}
      className="toaster group pointer-events-auto"
      position="top-right"
      closeButton
      richColors
      {...props}
    />
  )
}

export { Toaster }
