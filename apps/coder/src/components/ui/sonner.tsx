import { Toaster as Sonner, ToasterProps } from "sonner"
import { useDarkMode } from "../../hooks/use-dark-mode"

const Toaster = ({ ...props }: ToasterProps) => {
  const { isDark } = useDarkMode()

  return (
    <Sonner
      theme={isDark ? "dark" : "light"}
      className="toaster group !font-mono"
      style={{
        '--normal-bg': 'hsl(var(--popover))',
        '--normal-text': 'hsl(var(--popover-foreground))',
        '--normal-border': 'hsl(var(--border))',
        '--success-bg': isDark ? 'hsl(142.1 70.6% 45.3%)' : 'hsl(142.1 76.2% 91.1%)',
        '--success-text': isDark ? 'hsl(142.1 70.6% 95.3%)' : 'hsl(142.1 76.2% 21.1%)',
        '--error-bg': isDark ? 'hsl(0 84.2% 40.2%)' : 'hsl(0 84.2% 95.2%)',
        '--error-text': isDark ? 'hsl(0 0% 98%)' : 'hsl(0 72.2% 30.6%)',
      } as React.CSSProperties}
      {...props}
    />
  )
}

export { Toaster }
