/**
 * Global type declarations for the agents package
 */

// Extend the global scope to include our custom properties
declare global {
  // For our SSE shim registration tracking
  var __EFFECT_SSE_SHIM_REGISTERED: boolean;
}

export {};