/**
 * Polyfill Buffer for browser (gray-matter and other deps that expect Node's Buffer).
 * Import this before any module that uses Buffer (e.g. content.ts).
 */
import { Buffer } from 'buffer';

if (typeof globalThis.Buffer === 'undefined') {
  (globalThis as Record<string, unknown>).Buffer = Buffer;
}
