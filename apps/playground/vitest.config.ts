import { mergeConfig, type UserConfigExport } from "vitest/config"
import shared from "../../vitest.shared.js"

const config: UserConfigExport = {
  test: {
    environment: 'jsdom',
    setupFiles: ['./test/setup.ts']
  }
}

export default mergeConfig(shared, config)