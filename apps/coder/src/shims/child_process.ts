/**
 * Shimmed version of child_process for browser environments
 * 
 * This provides mock implementations of child_process functions
 * to prevent runtime errors in the browser
 */

export const spawn = () => {
  console.warn('child_process.spawn is not available in browser environments');
  // Return a dummy object that matches the basic shape of a ChildProcess
  return {
    on: () => {},
    stdout: { on: () => {} },
    stderr: { on: () => {} },
    kill: () => {}
  };
};

export const exec = () => {
  console.warn('child_process.exec is not available in browser environments');
  // Invoke callback with error
  if (arguments.length > 1 && typeof arguments[arguments.length - 1] === 'function') {
    const callback = arguments[arguments.length - 1];
    callback(new Error('child_process.exec is not available in browser environments'));
  }
};

export const execFile = () => {
  console.warn('child_process.execFile is not available in browser environments');
  // Invoke callback with error
  if (arguments.length > 1 && typeof arguments[arguments.length - 1] === 'function') {
    const callback = arguments[arguments.length - 1];
    callback(new Error('child_process.execFile is not available in browser environments'));
  }
};

export default {
  spawn,
  exec,
  execFile
};