import { existsSync } from "node:fs"
import { join } from "node:path"
import { fileURLToPath } from "node:url"

import {
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
} from "./src/shared/apple-fm-packaging.js"

const desktopRoot = fileURLToPath(new URL(".", import.meta.url))
const helperBuildSource =
  "../../apps/pylon/swift/foundation-bridge/.build/release/foundation-bridge"
const helperWrapperSource = "../../apps/pylon/bin/foundation-bridge"
export const appleFmBridgeCopySource = existsSync(join(desktopRoot, helperBuildSource))
  ? helperBuildSource
  : helperWrapperSource

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
      [appleFmBridgeCopySource]: APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
    },
  },
}
