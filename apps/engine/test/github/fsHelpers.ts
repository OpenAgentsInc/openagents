// Custom helpers file for fs mocking tests
import { vi } from "@effect/vitest"

// Create mock functions
export const existsSyncMock = vi.fn()
export const mkdirSyncMock = vi.fn()
export const writeFileSyncMock = vi.fn()
export const readFileSyncMock = vi.fn()

// Setup the mock object with our functions
export const fsMock = {
  existsSync: existsSyncMock,
  mkdirSync: mkdirSyncMock,
  writeFileSync: writeFileSyncMock,
  readFileSync: readFileSyncMock
}

// Apply the mocks
vi.mock("node:fs", () => fsMock)
