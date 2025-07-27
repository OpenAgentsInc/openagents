import { vi } from "vitest";

// Mock Convex runtime environment
global.Convex = {
  syscall: vi.fn(),
  asyncSyscall: vi.fn(),
};

// Add any global test setup here
beforeEach(() => {
  vi.clearAllMocks();
});