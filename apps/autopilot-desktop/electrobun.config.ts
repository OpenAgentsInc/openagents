export default {
  app: {
    name: "Autopilot Desktop",
    identifier: "com.openagents.autopilot.desktop",
    version: "0.0.1"
  },
  build: {
    bun: {
      entrypoint: "src/bun/index.ts"
    },
    views: {
      "autopilot-desktop": {
        entrypoint: "src/ui/main.ts"
      }
    },
    copy: {
      "src/ui/index.html": "views/autopilot-desktop/index.html"
    }
  }
};
