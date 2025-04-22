// This file is used to set up fs mocks BEFORE any ESM imports happen
// It must use .js extension and CommonJS format for proper hoisting

// Export mock functions to be used in tests
export const existsSyncMock = vi.fn();
export const mkdirSyncMock = vi.fn();
export const writeFileSyncMock = vi.fn();
export const readFileSyncMock = vi.fn();

// Set up the complete mock object
export const fsMock = {
  existsSync: existsSyncMock,
  mkdirSync: mkdirSyncMock,
  writeFileSync: writeFileSyncMock,
  readFileSync: readFileSyncMock
};

// Apply the mock - must happen before any imports that use fs
export function setupFsMock() {
  vi.mock('node:fs', () => fsMock);
}

// This runs the setup automatically when the file is imported
import { vi } from 'vitest';
setupFsMock();