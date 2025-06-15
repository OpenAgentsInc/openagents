import { Effect, Layer, Context, pipe } from "effect"

export type WebTUITheme = "default" | "catppuccin" | "gruvbox" | "nord"

export class WebTUIThemeService extends Context.Tag("WebTUIThemeService")<
  WebTUIThemeService,
  {
    readonly setTheme: (theme: WebTUITheme) => Effect.Effect<void>
    readonly getCurrentTheme: () => Effect.Effect<WebTUITheme>
    readonly availableThemes: () => Effect.Effect<readonly WebTUITheme[]>
  }
>() {}

export const WebTUIThemeServiceLive = Layer.succeed(
  WebTUIThemeService,
  {
    setTheme: (theme: WebTUITheme) => 
      Effect.sync(() => {
        document.documentElement.setAttribute('data-webtui-theme', theme)
        // Store theme preference in localStorage
        localStorage.setItem('webtui-theme', theme)
      }),
    
    getCurrentTheme: () => 
      Effect.sync(() => {
        const stored = localStorage.getItem('webtui-theme')
        const current = document.documentElement.getAttribute('data-webtui-theme')
        return (stored || current || 'default') as WebTUITheme
      }),
    
    availableThemes: () => 
      Effect.succeed(['default', 'catppuccin', 'gruvbox', 'nord'] as const)
  }
)

// Helper functions for easy theme usage
export const setTheme = (theme: WebTUITheme) => 
  pipe(
    WebTUIThemeService,
    Effect.flatMap(service => service.setTheme(theme))
  )

export const getCurrentTheme = () => 
  pipe(
    WebTUIThemeService,
    Effect.flatMap(service => service.getCurrentTheme())
  )

export const getAvailableThemes = () => 
  pipe(
    WebTUIThemeService,
    Effect.flatMap(service => service.availableThemes())
  )

// Initialize theme on page load
export const initializeTheme = () =>
  pipe(
    getCurrentTheme(),
    Effect.flatMap(theme => setTheme(theme))
  )