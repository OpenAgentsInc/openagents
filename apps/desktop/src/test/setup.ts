import '@testing-library/jest-dom'
import { vi } from 'vitest'

// Mock Tauri API for testing
const mockTauri = {
  invoke: vi.fn(),
  listen: vi.fn(),
  emit: vi.fn(),
  convertFileSrc: vi.fn(),
}

// Mock @tauri-apps/api modules
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockTauri.invoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockTauri.listen,
  emit: mockTauri.emit,
}))

vi.mock('@tauri-apps/plugin-opener', () => ({
  open: vi.fn(),
}))

// Mock window.matchMedia for CSS media queries
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation(query => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // deprecated
    removeListener: vi.fn(), // deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
})

// Mock ResizeObserver
global.ResizeObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}))

// Mock IntersectionObserver
global.IntersectionObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}))

// Export mocks for use in tests
export { mockTauri }