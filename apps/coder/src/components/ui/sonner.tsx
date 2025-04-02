import { Toaster as Sonner, ToasterProps } from "sonner"
import { useDarkMode } from "../../hooks/use-dark-mode"

const Toaster = ({ ...props }: ToasterProps) => {
  const { isDark } = useDarkMode()

  return (
    <Sonner
      theme={isDark ? "dark" : "light"}
      position="bottom-left"
      className="toaster group !font-mono"
      style={{
        '--normal-bg': 'hsl(var(--popover))',
        '--normal-text': 'hsl(var(--popover-foreground))',
        '--normal-border': 'hsl(var(--border))',
        '--success-bg': isDark ? 'hsl(142.1 70.6% 45.3%)' : 'hsl(142.1 76.2% 91.1%)',
        '--success-text': isDark ? 'hsl(142.1 70.6% 95.3%)' : 'hsl(142.1 76.2% 21.1%)',
        '--error-bg': isDark ? 'hsl(0 84.2% 40.2%)' : 'hsl(0 84.2% 95.2%)',
        '--error-text': isDark ? 'hsl(0 0% 98%)' : 'hsl(0 72.2% 30.6%)',
        // Explicitly set transition properties to ensure animations work properly
        '--duration': '400ms',
        '--enter': 'transform 0.4s ease, opacity 0.4s ease',
        '--exit': 'transform 0.2s ease, opacity 0.2s ease',
        '--initial-height': 'auto',
      } as React.CSSProperties}
      {...props}
    />
  )
}

export { Toaster }
