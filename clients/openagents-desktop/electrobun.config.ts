export default {
  app: {
    name: "OpenAgents",
    identifier: "com.openagents.desktop",
    version: "0.1.0",
  },
  build: {
    bun: {
      entrypoint: "src/bun/index.ts",
    },
    views: {
      "openagents-desktop": {
        entrypoint: "resources/ui/main.js",
      },
    },
    copy: {
      "resources/ui/main.css": "views/openagents-desktop/main.css",
      "src/ui/index.html": "views/openagents-desktop/index.html",
    },
  },
}
