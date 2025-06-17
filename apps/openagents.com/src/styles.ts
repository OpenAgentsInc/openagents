import { css } from "@openagentsinc/psionic"

export const baseStyles = css`
  * {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }
  
  :root {
    --bg-primary: #000;
    --bg-secondary: #111;
    --text-primary: #fff;
    --text-secondary: #aaa;
    --accent: #00ff00;
    --font-mono: "Berkeley Mono", "SF Mono", "Monaco", monospace;
  }
  
  html, body {
    width: 100%;
    min-height: 100vh;
    background: var(--bg-primary);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 16px;
    line-height: 1.6;
  }
  
  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
  }
  
  nav {
    display: flex;
    gap: 2rem;
    padding: 2rem;
    border-bottom: 1px solid var(--bg-secondary);
    margin-bottom: 3rem;
  }
  
  nav a {
    color: var(--text-secondary);
    text-decoration: none;
    transition: color 0.2s;
  }
  
  nav a:hover,
  nav a.active {
    color: var(--text-primary);
  }
  
  h1 {
    font-size: 3rem;
    font-weight: normal;
    margin-bottom: 1rem;
  }
  
  h2 {
    font-size: 2rem;
    font-weight: normal;
    margin-bottom: 1rem;
  }
  
  p {
    color: var(--text-secondary);
    margin-bottom: 1rem;
  }
  
  .hero {
    text-align: center;
    padding: 4rem 0;
  }
  
  .hero h1 {
    font-size: 4rem;
    margin-bottom: 1rem;
  }
  
  .hero .tagline {
    font-size: 1.5rem;
    color: var(--text-secondary);
    margin-bottom: 2rem;
  }
  
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 2rem;
    margin-top: 3rem;
  }
  
  .card {
    background: var(--bg-secondary);
    border: 1px solid #222;
    padding: 2rem;
    transition: border-color 0.2s;
  }
  
  .card:hover {
    border-color: var(--accent);
  }
  
  .status-indicator {
    display: inline-block;
    width: 8px;
    height: 8px;
    background: var(--accent);
    border-radius: 50%;
    margin-right: 0.5rem;
  }
  
  .status-indicator.offline {
    background: #ff3333;
  }
  
  code {
    background: var(--bg-secondary);
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
  }
  
  pre {
    background: var(--bg-secondary);
    padding: 1rem;
    overflow-x: auto;
    margin-bottom: 1rem;
  }
`
