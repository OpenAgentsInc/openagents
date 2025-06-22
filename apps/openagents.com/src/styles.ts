import { css } from "@openagentsinc/psionic"
import { syntaxHighlightingStyles } from "./styles/syntax-highlighting"

// Base styles for OpenAgents v1 components
export const baseStyles = css`
  /* Tailwind OpenAgents v1 components - inlined to avoid static file issues */
  
  /* Custom color definitions for dark theme */
  :root {
    --oa-black: #000000;
    --oa-offblack: #0a0a0a;
    --oa-darkgray: #333333;
    --oa-gray: #666666;
    --oa-lightgray: #999999;
    --oa-white: #ffffff;
    --oa-text: #e5e5e5;
  }

  /* Button Component */
  .oa-button {
    @apply px-4 py-2 rounded-lg font-medium transition-colors duration-200 cursor-pointer inline-flex items-center justify-center;
    @apply focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-black;
    @apply disabled:opacity-50 disabled:cursor-not-allowed;
  }

  .oa-button-primary {
    @apply oa-button;
    @apply bg-blue-600 text-white hover:bg-blue-700 focus:ring-blue-500;
    @apply active:bg-blue-800;
  }

  .oa-button-secondary {
    @apply oa-button;
    @apply bg-gray-700 text-gray-100 hover:bg-gray-600 focus:ring-gray-500;
    @apply active:bg-gray-800;
  }

  /* Card Component */
  .oa-card {
    @apply bg-black border border-gray-800 rounded-lg;
    @apply p-6;
  }

  .oa-card-shadow {
    @apply oa-card shadow-lg shadow-black/50;
  }

  /* Header Component */
  .oa-header {
    @apply fixed top-0 left-0 right-0 z-30;
    @apply bg-black/95 backdrop-blur-sm border-b border-gray-900;
    @apply px-6 py-4;
  }

  .oa-header-content {
    @apply max-w-screen-xl mx-auto;
    @apply flex items-center justify-between;
  }

  .oa-header-brand {
    @apply flex items-center;
  }

  .oa-header-title {
    @apply text-xl font-bold text-white tracking-tight;
    @apply hover:text-gray-200 transition-colors;
  }

  .oa-header-nav {
    @apply flex items-center space-x-8;
  }

  .oa-header-nav-link {
    @apply text-gray-400 hover:text-white transition-all duration-200;
    @apply text-sm font-medium tracking-wide;
    @apply relative;
  }

  .oa-header-nav-link:after {
    @apply content-[''] absolute bottom-0 left-0 w-0 h-[2px];
    @apply bg-blue-500 transition-all duration-200;
  }

  .oa-header-nav-link:hover:after {
    @apply w-full;
  }

  .oa-header-nav-link.active {
    @apply text-white;
  }

  .oa-header-nav-link.active:after {
    @apply w-full bg-white;
  }
  
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
