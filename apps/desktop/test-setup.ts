import "@testing-library/jest-dom";
import { vi } from "vitest";

// Extend window interface for Tauri runtime
declare global {
  interface Window {
    __TAURI__: {
      invoke: typeof vi.fn
    }
  }
}

// Mock Tauri API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock window.__TAURI__
window.__TAURI__ = {
  invoke: vi.fn(),
};

// Mock matchMedia
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // deprecated
    removeListener: vi.fn(), // deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});