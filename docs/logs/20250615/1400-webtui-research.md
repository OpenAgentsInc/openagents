# Creating a CSS-only npm package with vendored WebTUI styles

WebTUI is a modular CSS library that brings terminal interface aesthetics to the browser. To create a deployable @openagentsinc/ui package that vendors WebTUI's CSS without React components, you'll need to implement a careful vendoring strategy, configure appropriate build tools, and structure exports for easy consumption.

## WebTUI vendoring strategy

WebTUI uses a modular architecture with components (badge, button, checkbox, dialog, input, etc.) and theme plugins (Catppuccin, Nord, Gruvbox). The library leverages CSS `@layer` for predictable style precedence and provides light/dark mode support. For effective vendoring, create a selective extraction script that copies only the CSS components you need while maintaining proper license attribution.

**Recommended vendoring script:**
```typescript
// scripts/vendor-webtui.ts
import fs from 'fs-extra';
import path from 'path';

const COMPONENTS = [
  'base',
  'components/button',
  'components/input',
  'utils/box'
];

async function vendorWebTUI() {
  const vendorDir = path.join('src', 'vendor', 'webtui');
  await fs.ensureDir(vendorDir);

  for (const component of COMPONENTS) {
    const srcPath = path.join('node_modules', '@webtui/css', 'dist', `${component}.css`);
    const destPath = path.join(vendorDir, `${component}.css`);

    if (await fs.pathExists(srcPath)) {
      let css = await fs.readFile(srcPath, 'utf8');
      css = `/*! Vendored from WebTUI CSS Library - MIT License */\n${css}`;

      await fs.ensureDir(path.dirname(destPath));
      await fs.writeFile(destPath, css);
    }
  }
}
```

## Package structure and organization

Structure your CSS-only npm package following established patterns from successful packages like normalize.css and animate.css:

```
@openagentsinc/ui/
├── src/                        # Source files
│   ├── vendor/
│   │   └── webtui/            # Vendored WebTUI CSS
│   ├── components/            # Organized by component
│   │   ├── buttons.css
│   │   ├── inputs.css
│   │   └── tables.css
│   ├── themes/               # Theme variations
│   └── index.css             # Main entry point
├── dist/                     # Built output
│   ├── index.css            # Complete unminified CSS
│   ├── index.min.css        # Production minified version
│   ├── components/          # Individual component files
│   └── themes/              # Theme files
├── package.json
└── LICENSE
```

## Build configuration with modern tooling

**Rollup configuration** provides optimal tree-shaking and smaller bundles for libraries:

```javascript
// rollup.config.js
import postcss from 'rollup-plugin-postcss';
import copy from 'rollup-plugin-copy';

export default {
  input: 'src/index.css',
  output: {
    file: 'dist/index.css'
  },
  plugins: [
    postcss({
      extract: true,
      minimize: false,
      use: ['sass'],
      plugins: [require('autoprefixer')]
    }),
    // Create minified version
    postcss({
      extract: 'index.min.css',
      minimize: true,
      use: ['sass']
    }),
    copy({
      targets: [
        { src: 'src/components/*.css', dest: 'dist/components' },
        { src: 'src/themes/*.css', dest: 'dist/themes' }
      ]
    })
  ]
};
```

**Alternative Vite configuration** for faster builds:

```javascript
// vite.config.ts
import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.css'),
      formats: ['es']
    },
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        assetFileNames: (assetInfo) => {
          if (assetInfo.name === 'style.css') return 'index.css';
          return assetInfo.name;
        }
      }
    }
  }
});
```

## Package.json configuration for CSS exports

Configure your package.json to support multiple import patterns and modern tooling:

```json
{
  "name": "@openagentsinc/ui",
  "version": "1.0.0",
  "description": "Terminal-style CSS components based on WebTUI",
  "main": "dist/index.css",
  "style": "dist/index.css",
  "sass": "src/index.scss",
  "files": [
    "dist/",
    "src/",
    "LICENSE",
    "README.md"
  ],
  "exports": {
    ".": {
      "style": "./dist/index.css",
      "sass": "./src/index.scss",
      "default": "./dist/index.css"
    },
    "./dist/*.css": {
      "import": "./dist/*.css",
      "require": "./dist/*.css"
    },
    "./components/*": "./dist/components/*",
    "./themes/*": "./dist/themes/*",
    "./package.json": "./package.json"
  },
  "scripts": {
    "vendor": "tsx scripts/vendor-webtui.ts",
    "build": "npm run vendor && npm run build:css",
    "build:css": "rollup -c",
    "dev": "rollup -c -w",
    "prepublishOnly": "npm run build"
  },
  "keywords": ["css", "terminal", "tui", "webtui"],
  "license": "MIT",
  "devDependencies": {
    "@rollup/plugin-node-resolve": "^15.0.0",
    "rollup": "^4.0.0",
    "rollup-plugin-postcss": "^4.0.0",
    "sass": "^1.70.0",
    "tsx": "^4.0.0"
  }
}
```

## Making CSS easily importable

Support multiple import patterns to accommodate different build tools and consumer preferences:

**1. Main entry point import:**
```javascript
// Imports complete CSS bundle
import '@openagentsinc/ui';
// or
import '@openagentsinc/ui/dist/index.css';
```

**2. Component-specific imports:**
```javascript
// Import only what you need
import '@openagentsinc/ui/components/buttons.css';
import '@openagentsinc/ui/components/inputs.css';
```

**3. Theme imports:**
```javascript
// Apply specific themes
import '@openagentsinc/ui/themes/catppuccin.css';
```

**4. Sass/SCSS imports for customization:**
```scss
// For build-time customization
@import '@openagentsinc/ui/src/index.scss';
```

**5. PostCSS imports:**
```css
@import '@openagentsinc/ui';
```

## TypeScript support for CSS modules

Even though this is a CSS-only package, provide TypeScript support for better developer experience:

```typescript
// types/index.d.ts
declare module '@openagentsinc/ui' {
  const styles: string;
  export default styles;
}

declare module '@openagentsinc/ui/components/*' {
  const styles: string;
  export default styles;
}
```

## License compliance and documentation

Since WebTUI is MIT licensed, maintain proper attribution:

```css
/*!
 * @openagentsinc/ui v1.0.0
 * Terminal-style CSS components
 *
 * Contains portions from:
 * - WebTUI CSS Library (MIT) - https://webtui.ironclad.sh
 *
 * Licensed under MIT License
 */
```

Include a comprehensive README.md documenting:
- Installation instructions
- Import patterns for different bundlers
- Available components and themes
- Customization options
- Browser support
- Migration guide from WebTUI

## Production deployment checklist

Before publishing to npm:
1. **Vendor only necessary components** to minimize bundle size
2. **Provide both minified and unminified versions** for development and production
3. **Include source maps** for debugging
4. **Test imports** with major bundlers (Webpack, Vite, Rollup, Parcel)
5. **Document all import patterns** clearly
6. **Set up automated builds** with GitHub Actions or similar
7. **Use semantic versioning** for updates
8. **Include a CHANGELOG.md** for version history

This approach creates a professional CSS-only npm package that vendors WebTUI's terminal aesthetics while providing flexibility for consumers to import and use the styles with any framework. The modular structure allows developers to import only what they need, while the build configuration ensures compatibility with modern tooling and bundlers.
