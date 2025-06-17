import { css } from "@openagentsinc/psionic"

// Import WebTUI CSS (will be included via link tags)
export const webtuiStyles = css`
  @import '/webtui/index.css';
  
  /* Override base :root colors for themes */
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
  
  :root:has(.theme-zinc-light) {
    --background0: #fafafa;
    --background1: #f4f4f5;
    --background2: #e4e4e7;
    --background3: #d4d4d8;
    --foreground0: #52525b;
    --foreground1: #3f3f46;
    --foreground2: #27272a;
    --accent: #52525b;
    --success: #71717a;
    --warning: #52525b;
    --danger: #3f3f46;
    --surface0: #f4f4f5;
    --surface1: #e4e4e7;
    --surface2: #d4d4d8;
    --overlay0: #a1a1aa;
    --overlay1: #71717a;
    --overlay2: #52525b;
    --font-size: 16px;
    --line-height: 1.3;
    --font-weight-bold: 700;
    --font-weight-normal: 400;
    --font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
  }
  
  :root:has(.theme-catppuccin) {
    --background0: #1e1e2e;
    --background1: #313244;
    --background2: #45475a;
    --background3: #585b70;
    --foreground0: #bac2de;
    --foreground1: #cdd6f4;
    --foreground2: #f5e0dc;
  }
  
  :root:has(.theme-gruvbox) {
    --background0: #282828;
    --background1: #3c3836;
    --background2: #504945;
    --background3: #665c54;
    --foreground0: #bdae93;
    --foreground1: #ebdbb2;
    --foreground2: #fbf1c7;
  }
  
  :root:has(.theme-nord) {
    --background0: #2e3440;
    --background1: #3b4252;
    --background2: #434c5e;
    --background3: #4c566a;
    --foreground0: #d8dee9;
    --foreground1: #e5e9f0;
    --foreground2: #eceff4;
  }
`

// Base styles for layout and WebTUI integration
export const baseStyles = css`
  ${webtuiStyles}
  
  /* Default to zinc dark theme if no theme class is present */
  :root:has(body:not([class*="theme-"])) {
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
    padding: 2rem;
  }
  
  .hero {
    text-align: center;
    padding: 4rem 0;
  }
  
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 2rem;
    margin-top: 3rem;
  }
  
  .theme-switcher {
    position: absolute;
    top: 1rem;
    right: 1rem;
  }
  
  .nav-container {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 2rem;
    border-bottom: 1px solid var(--background2);
    margin-bottom: 3rem;
  }
  
  .nav-links {
    display: flex;
    gap: 2rem;
  }
`
