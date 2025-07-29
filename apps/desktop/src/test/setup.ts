import { vi } from 'vitest'
import '@testing-library/jest-dom/vitest'

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

// Mock window.matchMedia for CSS media queries (if window exists)
if (typeof window !== 'undefined') {
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
}

// Mock localStorage if it doesn't exist
if (typeof localStorage === 'undefined') {
  const store: Record<string, string> = {}
  global.localStorage = {
    getItem: vi.fn((key: string) => store[key] || null),
    setItem: vi.fn((key: string, value: string) => { 
      store[key] = value 
    }),
    removeItem: vi.fn((key: string) => { 
      delete store[key] 
    }),
    clear: vi.fn(() => {
      Object.keys(store).forEach(key => delete store[key])
    }),
    length: 0,
    key: vi.fn(),
  } as any
}

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