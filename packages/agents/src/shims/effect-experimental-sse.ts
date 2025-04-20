/**
 * Shim for @effect/experimental/Sse which is needed by @effect/ai-anthropic
 * This provides a minimal compatibility layer to address the import error
 * during bundling for Cloudflare Workers
 */

// Define a minimal SseConsumer interface that matches what @effect/ai-anthropic expects
export interface SseConsumer<A> {
  onOpen?: () => void;
  onMessage: (a: A) => void;
  onError?: (error: unknown) => void;
  onComplete?: () => void;
}

// Provide a minimal implementation of the SseClient interface
export class SseClient<A> {
  constructor(
    private url: URL | string,
    private options?: RequestInit
  ) {}

  // This is a stub implementation since we're not using streaming in our Effect AI test
  stream(consumer: SseConsumer<A>): () => void {
    console.warn("SSE streaming is not supported in this shim implementation");
    
    // In a real implementation, this would handle event streaming
    // For our case, we'll just call onComplete immediately
    if (consumer.onComplete) {
      setTimeout(() => consumer.onComplete?.(), 0);
    }
    
    // Return a no-op cleanup function
    return () => {};
  }
}

// Export the SseClient factory method expected by @effect/ai-anthropic
export const make = <A>(url: URL | string, options?: RequestInit): SseClient<A> => {
  return new SseClient<A>(url, options);
};

// Export a default object to satisfy the module system
export default {
  SseClient,
  make
};