/**
 * Empty shim for child_process in browser environments
 * 
 * Since we've now updated the code to conditionally import Node.js modules
 * only in server environments, this shim is very minimal and serves as a placeholder
 * to satisfy imports in browser environments.
 */

// Just provide empty placeholder functions that do nothing
export const spawn = () => {
  console.warn('child_process methods are not available in browser environments');
  return { on: () => {}, stdout: { on: () => {} }, stderr: { on: () => {} }, kill: () => {} };
};

export const exec = () => {
  console.warn('child_process methods are not available in browser environments');
};

export const execFile = () => {
  console.warn('child_process methods are not available in browser environments');
};

// Export an empty object for any direct module imports
export default {
  spawn, exec, execFile
};