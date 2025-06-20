import { config } from "dotenv"
import { beforeAll } from "vitest"

// Load environment variables before tests
beforeAll(() => {
  config({ path: ".env.test" })
  // Fall back to regular .env if .env.test doesn't exist
  if (!process.env.DATABASE_HOST) {
    config()
  }
})
