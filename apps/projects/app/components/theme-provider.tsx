import React, { createContext, useContext, useEffect, useState } from "react"

type Theme = "dark" | "light" | "system"

type ThemeProviderProps = {
  children: React.ReactNode
  defaultTheme?: Theme
  storageKey?: string
}

type ThemeProviderState = {
  theme: Theme
  setTheme: (theme: Theme) => void
}

const initialState: ThemeProviderState = {
  theme: "dark",
  setTheme: () => null,
}

const ThemeProviderContext = createContext<ThemeProviderState>(initialState)

export function ThemeProvider({
  children,
  defaultTheme = "dark",
  storageKey = "openagents-ui-theme",
  ...props
}: ThemeProviderProps) {
  // Simple state management with a default theme of "dark"
  const [theme, setTheme] = useState<Theme>(defaultTheme)

  // Apply theme changes to document element
  useEffect(() => {
    // Skip server-side rendering
    if (typeof document === 'undefined') return;
    
    const root = document.documentElement
    
    // Remove existing theme classes
    root.classList.remove("light", "dark")
    
    // Apply appropriate theme class
    if (theme === "system") {
      const systemTheme = window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      root.classList.add(systemTheme)
    } else {
      root.classList.add(theme)
    }
  }, [theme])

  // Load saved theme on first render
  useEffect(() => {
    // Skip server-side rendering
    if (typeof window === 'undefined') return;
    
    try {
      const savedTheme = localStorage.getItem(storageKey) as Theme | null
      
      if (savedTheme) {
        setTheme(savedTheme)
      } else if (defaultTheme === "system") {
        // If no saved theme and default is system, check user preference
        const systemTheme = window.matchMedia("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light"
        setTheme(systemTheme)
      }
    } catch (error) {
      console.error("Error reading theme from localStorage:", error)
    }
  }, [])

  // Context value with theme and update function
  const value = {
    theme,
    setTheme: (newTheme: Theme) => {
      try {
        // Skip localStorage on server
        if (typeof window !== 'undefined') {
          localStorage.setItem(storageKey, newTheme)
        }
        setTheme(newTheme)
      } catch (error) {
        console.error("Error saving theme to localStorage:", error)
      }
    },
  }

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  )
}

export const useTheme = () => {
  const context = useContext(ThemeProviderContext)

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider")

  return context
}