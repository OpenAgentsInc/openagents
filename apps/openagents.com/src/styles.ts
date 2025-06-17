import { css } from "@openagentsinc/psionic"

// Import WebTUI CSS (will be included via link tags)
export const webtuiStyles = css`
  @import '/webtui/index.css';
  @import '/theme-zinc.css';
  @import '/theme-catppuccin.css';
  @import '/theme-gruvbox.css';
  @import '/theme-nord.css';
`

// Base styles for layout and WebTUI integration
export const baseStyles = css`
  ${webtuiStyles}
  
  /* Default to zinc theme if no theme class is present */
  body:not([class*="theme-"]) {
    --background0: #09090b;
    --background1: #18181b;
    --background2: #27272a;
    --background3: #3f3f46;
    --foreground0: #a1a1aa;
    --foreground1: #d4d4d8;
    --foreground2: #e4e4e7;
  }
  
  html, body {
    margin: 0;
    padding: 0;
    width: 100%;
    min-height: 100vh;
    background-color: var(--background0);
    color: var(--foreground1);
    overflow-x: hidden;
    font-family: monospace;
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
