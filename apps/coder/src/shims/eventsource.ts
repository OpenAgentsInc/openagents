/**
 * EventSource polyfill for Electron renderer process
 * This shim replaces the Node.js EventSource with a browser-compatible version
 */

// Use the browser's native EventSource if available
const EventSourceShim = window.EventSource;

export default EventSourceShim;