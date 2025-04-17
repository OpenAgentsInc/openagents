/**
 * Polyfills for browser APIs needed by React 19's server components
 */

// Polyfill for MessageChannel if it doesn't exist in the environment
if (typeof MessageChannel === 'undefined') {
  // @ts-ignore
  globalThis.MessageChannel = class MessageChannel {
    port1: any;
    port2: any;
    
    constructor() {
      this.port1 = {
        postMessage: () => {},
        onmessage: null,
        close: () => {}
      };
      this.port2 = {
        postMessage: () => {},
        onmessage: null,
        close: () => {}
      };
    }
  };
  
  // @ts-ignore
  globalThis.MessagePort = class MessagePort {
    onmessage: any = null;
    
    postMessage() {}
    close() {}
  };
}

// Add any other polyfills needed for server.edge.js or server.browser.js here