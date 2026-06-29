import {
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_SOURCE,
} from "./src/shared/apple-fm-packaging.js"

export default {
  app: {
    name: "Khala",
    identifier: "com.openagents.khala.desktop",
    version: "0.1.0",
  },
  build: {
    bun: {
      entrypoint: "src/bun/index.ts",
    },
    views: {
      "khala-desktop": {
        entrypoint: "resources/ui/main.js",
      },
    },
    copy: {
      "src/ui/index.html": "views/khala-desktop/index.html",
      [APPLE_FM_BRIDGE_ELECTROBUN_COPY_SOURCE]: APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
    },
  },
}
