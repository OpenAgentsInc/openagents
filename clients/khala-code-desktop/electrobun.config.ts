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
    views: {
      "khala-code-desktop": {
        entrypoint: "resources/ui/main.js",
      },
    },
    copy: {
      "resources/ui/main.css": "views/khala-code-desktop/main.css",
      "src/ui/fonts/BerkeleyMono-Bold.woff2":
        "views/khala-code-desktop/fonts/BerkeleyMono-Bold.woff2",
      "src/ui/fonts/BerkeleyMono-Regular.woff2":
        "views/khala-code-desktop/fonts/BerkeleyMono-Regular.woff2",
      "src/ui/index.html": "views/khala-code-desktop/index.html",
    },
  },
}
