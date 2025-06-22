import type { PsionicConfig } from "../types"

export function getTailwindHead(config: PsionicConfig): string {
  const tailwindConfig = config.tailwind ?? {}
  const enabled = tailwindConfig.enabled ?? true
  const useCdn = tailwindConfig.cdn ?? true

  if (!enabled) {
    return ""
  }

  if (!useCdn) {
    // Future: Support for build-time Tailwind integration
    return ""
  }

  // Base Tailwind CDN script
  let head = `<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"></script>`

  // Add custom configuration if provided
  if (tailwindConfig.config) {
    head += `
<style type="text/tailwindcss">
${tailwindConfig.config}
</style>`
  }

  // Add default Psionic theme configuration
  head += `
<style type="text/tailwindcss">
  @theme {
    /* Psionic default theme variables */
    --font-family-mono: ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace;
    
    /* Terminal-inspired color palette */
    --color-terminal-bg: #1a1b26;
    --color-terminal-fg: #c0caf5;
    --color-terminal-border: #414868;
    --color-terminal-accent: #7aa2f7;
    --color-terminal-success: #9ece6a;
    --color-terminal-warning: #e0af68;
    --color-terminal-danger: #f7768e;
  }
  
  /* Base styles */
  @layer base {
    body {
      font-family: var(--font-family-mono);
      background-color: var(--color-terminal-bg);
      color: var(--color-terminal-fg);
    }
  }
  
  /* Terminal-inspired components */
  @layer components {
    .btn-terminal {
      @apply px-4 py-2 font-mono border border-[--color-terminal-border] bg-[--color-terminal-bg] text-[--color-terminal-fg] hover:bg-[--color-terminal-border] transition-colors;
    }
    
    .box-terminal {
      @apply border border-[--color-terminal-border] p-4 bg-[--color-terminal-bg];
    }
  }
</style>`

  return head
}

export function wrapHtmlWithTailwind(html: string, config: PsionicConfig): string {
  // If the HTML already has a complete document structure, inject into head
  if (html.includes("</head>")) {
    return html.replace("</head>", getTailwindHead(config) + "\n</head>")
  }

  // Otherwise, wrap in a complete document
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${config.name || "Psionic App"}</title>
  ${getTailwindHead(config)}
</head>
<body>
  ${html}
</body>
</html>`
}
