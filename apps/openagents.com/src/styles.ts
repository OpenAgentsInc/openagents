import { css } from "@openagentsinc/psionic"

// Import WebTUI CSS (will be included via link tags)
export const webtuiStyles = css`
  @import '/webtui.css';
  @import '/theme-zinc.css';
  
  /* Catppuccin Theme */
  .webtui-theme-catppuccin {
    --webtui-background0: #1e1e2e;
    --webtui-background1: #313244;
    --webtui-background2: #45475a;
    --webtui-background3: #585b70;
    --webtui-foreground0: #bac2de;
    --webtui-foreground1: #cdd6f4;
    --webtui-foreground2: #f5e0dc;
    --webtui-accent: #89b4fa;
    --webtui-success: #a6e3a1;
    --webtui-warning: #f9e2af;
    --webtui-danger: #f38ba8;
  }
  
  /* Nord Theme */
  .webtui-theme-nord {
    --webtui-background0: #2e3440;
    --webtui-background1: #3b4252;
    --webtui-background2: #434c5e;
    --webtui-background3: #4c566a;
    --webtui-foreground0: #d8dee9;
    --webtui-foreground1: #e5e9f0;
    --webtui-foreground2: #eceff4;
    --webtui-accent: #88c0d0;
    --webtui-success: #a3be8c;
    --webtui-warning: #ebcb8b;
    --webtui-danger: #bf616a;
  }
  
  /* Gruvbox Theme */
  .webtui-theme-gruvbox {
    --webtui-background0: #282828;
    --webtui-background1: #3c3836;
    --webtui-background2: #504945;
    --webtui-background3: #665c54;
    --webtui-foreground0: #bdae93;
    --webtui-foreground1: #ebdbb2;
    --webtui-foreground2: #fbf1c7;
    --webtui-accent: #83a598;
    --webtui-success: #b8bb26;
    --webtui-warning: #fabd2f;
    --webtui-danger: #fb4934;
  }
  
  /* Fix light theme button contrast */
  .webtui-theme-zinc-light .webtui-button.webtui-variant-foreground1 {
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
    background-color: var(--webtui-background0);
    color: var(--webtui-foreground1);
    overflow-x: hidden;
  }
  
  /* Fix white overflow bars */
  html {
    background-color: var(--webtui-background0);
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
    border-bottom: 1px solid var(--webtui-background2);
    margin-bottom: 3rem;
  }
  
  .nav-links {
    display: flex;
    gap: 2rem;
  }
`
