import { css } from "@openagentsinc/psionic"

// Import WebTUI CSS (will be included via link tags)
export const webtuiStyles = css`
  @import '/webtui.css';
  @import '/theme-zinc.css';
  @import '/webtui/index.css';
  
  /* Catppuccin Theme */
  .theme-catppuccin {
    --background0: #1e1e2e;
    --background1: #313244;
    --background2: #45475a;
    --background3: #585b70;
    --foreground0: #bac2de;
    --foreground1: #cdd6f4;
    --foreground2: #f5e0dc;
    --accent: #89b4fa;
    --success: #a6e3a1;
    --warning: #f9e2af;
    --danger: #f38ba8;
  }
  
  /* Nord Theme */
  .theme-nord {
    --background0: #2e3440;
    --background1: #3b4252;
    --background2: #434c5e;
    --background3: #4c566a;
    --foreground0: #d8dee9;
    --foreground1: #e5e9f0;
    --foreground2: #eceff4;
    --accent: #88c0d0;
    --success: #a3be8c;
    --warning: #ebcb8b;
    --danger: #bf616a;
  }
  
  /* Gruvbox Theme */
  .theme-gruvbox {
    --background0: #282828;
    --background1: #3c3836;
    --background2: #504945;
    --background3: #665c54;
    --foreground0: #bdae93;
    --foreground1: #ebdbb2;
    --foreground2: #fbf1c7;
    --accent: #83a598;
    --success: #b8bb26;
    --warning: #fabd2f;
    --danger: #fb4934;
  }
  
  /* Fix light theme button contrast */
  .theme-zinc-light [is-~="button"][variant-~="foreground1"] {
    background-color: #3f3f46 !important;
    color: #fafafa !important;
    border-color: #3f3f46 !important;
  }
`

// Base styles for layout and WebTUI integration
export const baseStyles = css`
  ${webtuiStyles}
  
  html, body {
    margin: 0;
    padding: 0;
    width: 100%;
    min-height: 100vh;
    background-color: var(--background0);
    color: var(--foreground1);
    overflow-x: hidden;
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
