// Shared utilities
export function formatTimestamp(timestamp: number): string {
  return new Date(timestamp).toISOString();
}

export function generateId(): string {
  return Math.random().toString(36).substring(2) + Date.now().toString(36);
}

// Platform detection utilities
export * from './platform';

// Add more shared utilities as needed