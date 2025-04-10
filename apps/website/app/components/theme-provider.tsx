import React, { createContext, useContext, useEffect, useState, useRef } from "react"

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
  theme: "system",
  setTheme: () => null,
}

const ThemeProviderContext = createContext<ThemeProviderState>(initialState)

export function ThemeProvider({
  children,
  defaultTheme = "system",
  storageKey = "vite-ui-theme",
  ...props
}: ThemeProviderProps) {
  // Always use "dark" as the initial theme for hydration consistency
  const initialTheme = "dark";
  
  // After hydration, we'll get the actual theme from localStorage
  const [theme, setTheme] = useState<Theme>(initialTheme);
  
  // Track if we're hydrated
  const hasHydrated = useRef(false);
  
  // Initialize theme after first render (after hydration)
  useEffect(() => {
    if (!hasHydrated.current) {
      hasHydrated.current = true;
      
      // Now we can safely get the user's preferred theme
      const userTheme = getTheme(storageKey, defaultTheme);
      if (userTheme !== initialTheme) {
        setTheme(userTheme);
      }
    }
  }, [storageKey, defaultTheme]);
  
  // Handle theme changes (only after initial hydration)
  useEffect(() => {
    if (hasHydrated.current && typeof window !== 'undefined') {
      const root = window.document.documentElement;
      const resolvedTheme = theme === "system"
        ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
        : theme;
        
      root.classList.remove("light", "dark");
      root.classList.add(resolvedTheme);
    }
  }, [theme])

  const value = {
    theme,
    setTheme: (theme: Theme) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem(storageKey, theme)
      }
      setTheme(theme)
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

export function getTheme(storageKey: string = "vite-ui-theme", defaultTheme: Theme = "system"): Theme {
  if (typeof window === 'undefined') return defaultTheme;

  // Use the initial theme for consistency if available
  if (window.__INITIAL_THEME__ && !Array.isArray(window.__INITIAL_THEME__)) {
    return window.__INITIAL_THEME__ as Theme;
  }

  try {
    const theme = localStorage.getItem(storageKey) as Theme || defaultTheme;

    if (theme === "system") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }

    return theme;
  } catch (e) {
    return defaultTheme;
  }
}

// Add TypeScript declaration for the global window object
declare global {
  interface Window {
    __INITIAL_THEME__?: string;
  }
}
