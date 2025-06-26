import { css } from "@openagentsinc/psionic"
import { syntaxHighlightingStyles } from "./styles/syntax-highlighting"

// Base styles for OpenAgents v1 components
export const baseStyles = css`
  /* Import Basecoat UI from the @openagentsinc/ui package */
  @import '/@openagentsinc/ui/basecoat';
  
  ${syntaxHighlightingStyles}
  
  /* Default zinc dark theme */
  :root {
    --background0: #09090b;
    --background1: #18181b;
    --background2: #27272a;
    --background3: #3f3f46;
    --foreground0: #a1a1aa;
    --foreground1: #d4d4d8;
    --foreground2: #e4e4e7;
    --accent: #71717a;
    --success: #52525b;
    --warning: #a1a1aa;
    --danger: #52525b;
    --surface0: #18181b;
    --surface1: #27272a;
    --surface2: #3f3f46;
    --overlay0: #52525b;
    --overlay1: #71717a;
    --overlay2: #a1a1aa;
    --font-size: 16px;
    --line-height: 1.3;
    --font-weight-bold: 700;
    --font-weight-normal: 400;
    --font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
  }
  
  /* Zinc theme override for consistency */
  :root:has(.theme-zinc) {
    --background0: #09090b;
    --background1: #18181b;
    --background2: #27272a;
    --background3: #3f3f46;
    --foreground0: #a1a1aa;
    --foreground1: #d4d4d8;
    --foreground2: #e4e4e7;
    --accent: #71717a;
    --success: #52525b;
    --warning: #a1a1aa;
    --danger: #52525b;
  }
  
  html, body {
    margin: 0;
    padding: 0;
    width: 100%;
    min-height: 100vh;
    background-color: var(--background0);
    color: var(--foreground1);
    overflow-x: hidden;
    font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
  }
  
  /* Fix white overflow bars */
  html {
    background-color: var(--background0);
  }
  
  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 1rem;
  }
  
  .hero {
    text-align: center;
    padding: 2rem 0;
  }
  
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1rem;
    margin-top: 1.5rem;
  }
  
  .theme-switcher {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
  }
  
  .nav-container {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--background2);
    margin-bottom: 1rem;
  }
  
  .nav-links {
    display: flex;
    gap: 0.5rem;
  }
`
