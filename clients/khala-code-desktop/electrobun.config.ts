// The webview is built by Vite (build:ui = `vite build`) into ./dist, with
// Tailwind/Basecoat @apply compiled and fonts + scene assets self-contained
// under dist/assets/. Electrobun copies that dist into views/khala-code-desktop/
// and the bun entry loads views://khala-code-desktop/index.html.
export default {
  app: {
    name: "Khala Code",
    identifier: "com.openagents.khala.code.desktop",
    version: "0.1.0",
  },
  build: {
    bun: {
      entrypoint: "src/bun/index.ts",
    },
    mac: {
      icons: "resources/AppIcon.iconset",
    },
    win: {
      icon: "resources/khala-code-app-icon.png",
    },
    linux: {
      icon: "resources/khala-code-app-icon.png",
    },
    views: {},
    copy: {
      "dist/index.html": "views/khala-code-desktop/index.html",
      "dist/assets/": "views/khala-code-desktop/assets/",
    },
  },
}
