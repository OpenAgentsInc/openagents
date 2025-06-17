import { css } from "@openagentsinc/psionic"

// Import WebTUI CSS (will be included via link tags)
export const webtuiStyles = css`
  @import '/webtui.css';
  @import '/theme-zinc.css';
`

// Base styles for layout and WebTUI integration
export const baseStyles = css`
  ${webtuiStyles}
  
  html, body {
    margin: 0;
    padding: 0;
    width: 100%;
    min-height: 100vh;
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
