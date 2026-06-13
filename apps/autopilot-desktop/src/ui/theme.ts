import { cssVars, darkTokens } from "@openagentsinc/autopilot-ui"

export function darkThemeStyleCss(): string {
  const declarations = Object.entries(cssVars(darkTokens))
    .map(([name, value]) => `${name}:${value};`)
    .join("")

  return `:root{${declarations}}body{background:var(--bg);color:var(--text);}`
}
