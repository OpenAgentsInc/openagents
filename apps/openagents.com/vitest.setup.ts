import '@testing-library/jest-dom'
import { vi, beforeEach } from 'vitest'

// Mock DOM methods not implemented in jsdom
Object.defineProperty(Element.prototype, 'scrollIntoView', {
  value: vi.fn(),
  writable: true,
})

// Mock clipboard API
Object.assign(navigator, {
  clipboard: {
    writeText: vi.fn(),
    readText: vi.fn(),
  },
})

// Global test setup
beforeEach(() => {
  // Clear localStorage before each test
  localStorage.clear()
  // Clear sessionStorage before each test  
  sessionStorage.clear()
})

// Mock Next.js router
const mockPush = vi.fn()
const mockBack = vi.fn()
const mockRefresh = vi.fn()

vi.mock('next/navigation', () => ({
  useRouter: () => ({
    push: mockPush,
    back: mockBack,
    refresh: mockRefresh,
  }),
  usePathname: () => '/',
  useSearchParams: () => new URLSearchParams(),
}))

// Mock Convex hooks
vi.mock('convex/react', () => ({
  useMutation: () => vi.fn(),
  useQuery: () => undefined,
  useAction: () => vi.fn(),
}))

// Export mocks for use in tests
export { mockPush, mockBack, mockRefresh }