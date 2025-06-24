# Tailwind CSS with Vite Integration Guide

This guide explains how Tailwind CSS v4 is integrated with Vite in the openagents.com application.

## Overview

We use Tailwind CSS v4 with the new `@tailwindcss/vite` plugin for optimal bundling and hot module replacement (HMR). This provides a production-ready setup that's superior to the CDN approach used in Psionic.

## Key Files

### 1. Vite Configuration (`vite.config.ts`)
```typescript
import tailwindcss from "@tailwindcss/vite"

export default defineConfig({
  plugins: [tailwindcss()],
  // ... rest of config
})
```

### 2. Main CSS Entry (`src/client/main.css`)
```css
@import "tailwindcss";

/* Tailwind v4 theme configuration */
@theme {
  /* Custom colors matching OpenAgents design */
  --color-offblack: #0a0a0a;
  --color-darkgray: #1a1a1a;
  --color-gray: #333333;
  --color-lightgray: #666666;
  --color-offwhite: #f5f5f5;
  
  /* Terminal-inspired colors */
  --color-terminal-bg: #1a1b26;
  --color-terminal-fg: #c0caf5;
  --color-terminal-border: #414868;
  --color-terminal-accent: #7aa2f7;
  
  /* Font stacks */
  --font-family-mono: 'Berkeley Mono', ui-monospace, SFMono-Regular, 'SF Mono', Consolas, 'Liberation Mono', Menlo, monospace;
}
```

### 3. Client Entry Point (`src/client/index.ts`)
```typescript
// Import styles
import './main.css'
```

## Build Process

### Development
- Vite dev server automatically injects styles via HMR
- No `<link>` tag needed in development
- Instant style updates without page reload

### Production
1. Vite builds CSS to `public/css/client.css`
2. Components include link tag: `<link rel="stylesheet" href="/css/main.css">`
3. CSS is minified and optimized

## Key Features

### 1. Custom Theme Configuration
We use Tailwind v4's `@theme` directive to define custom properties:
- OpenAgents color palette (offblack, darkgray, etc.)
- Terminal-inspired accent colors
- Berkeley Mono font stack

### 2. Component Classes
Pre-defined components in `@layer components`:
- `.btn-terminal` - Terminal-style buttons
- `.box-terminal` - Terminal-style containers
- `.chat-message` - Chat message styling
- `.chat-input` - Chat input styling

### 3. Berkeley Mono Font
Custom font files served from `/public/fonts/`:
```css
@font-face {
  font-family: 'Berkeley Mono';
  src: url('/fonts/BerkeleyMono-Regular.woff2') format('woff2');
  font-weight: 400;
  font-style: normal;
  font-display: swap;
}
```

## Usage in Components

Components use standard Tailwind utility classes:
```html
<button class="btn-terminal hover:bg-darkgray">Click me</button>
<div class="border border-gray bg-offblack p-4">Content</div>
```

## Relationship to Psionic CDN

### Current Setup
- **Psionic**: Uses Tailwind Play CDN for simplicity
- **openagents.com**: Uses proper Vite bundling for production

### Why Keep Both?
1. **CDN in Psionic**: Good for demos and quick prototypes
2. **Vite in apps**: Better for production (tree-shaking, optimization)

### Future Considerations
- Could add build-time Tailwind support to Psionic
- Apps can choose CDN vs bundled based on needs
- Migration path is straightforward

## Build Configuration Details

### Vite Output Structure
```
public/
├── css/
│   └── client.css     # Bundled Tailwind CSS
├── js/
│   ├── client.js      # Main entry point
│   ├── chat.js        # Chat functionality
│   └── model-selector.js
└── fonts/
    └── BerkeleyMono-*.woff2
```

### Asset Handling
```typescript
assetFileNames: (assetInfo) => {
  if (assetInfo.name?.endsWith('.css')) {
    return 'css/[name][extname]'
  }
  return 'assets/[name]-[hash][extname]'
}
```

## Common Tasks

### Adding New Colors
Add to `@theme` in `main.css`:
```css
@theme {
  --color-custom: #hexcode;
}
```

### Creating Component Classes
Add to `@layer components`:
```css
@layer components {
  .my-component {
    @apply border border-gray bg-offblack p-4;
  }
}
```

### Conditional CSS Loading
```typescript
const isDev = process.env.NODE_ENV !== "production"
const cssLink = isDev ? "" : '<link rel="stylesheet" href="/css/main.css">'
```

## Performance Benefits

1. **Tree-shaking**: Only used utilities included
2. **Minification**: CSS optimized for production
3. **Caching**: Proper cache headers for static assets
4. **HMR**: Instant updates in development

## Troubleshooting

### CSS Not Loading
- Check build output in `public/css/`
- Verify link tag in production HTML
- Ensure Vite plugin is in config

### Font Loading Issues
- Verify font files in `public/fonts/`
- Check font-face declarations
- Use browser DevTools Network tab

### HMR Not Working
- Ensure Vite dev server is running
- Check for console errors
- Verify import in entry point

## Future Enhancements

1. **PostCSS Plugins**: Add autoprefixer, cssnano
2. **CSS Modules**: For component-scoped styles
3. **Critical CSS**: Inline above-fold styles
4. **Purgecss**: Further optimize bundle size

## Summary

This Tailwind + Vite setup provides:
- Production-ready CSS bundling
- Excellent developer experience with HMR
- Custom theme matching OpenAgents design
- Clear migration path from CDN approach

The implementation demonstrates best practices for integrating modern CSS tooling with Effect-based applications.