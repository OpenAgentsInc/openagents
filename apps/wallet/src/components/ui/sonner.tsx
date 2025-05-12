import { useTheme } from "@/components/theme-provider"
import { Toaster as Sonner, ToasterProps } from "sonner"

const Toaster = ({ ...props }: ToasterProps) => {
  const { resolvedTheme } = useTheme()

  return (
    <Sonner
      theme={resolvedTheme}
      className="toaster group pointer-events-auto font-mono"
      position="top-right"
      closeButton={false}
      richColors
      toastOptions={{
        className: "font-mono",
        style: {
          fontFamily: "'Berkeley Mono', monospace"
        }
      }}
      {...props}
    />
  )
}

export { Toaster }
