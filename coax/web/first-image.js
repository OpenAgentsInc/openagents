// Firm reason: 1 (browser glue observing the host's admitted scene and GPU completion)
// Installed before navigation. A loading clear is not a first world image.
(() => {
  let value, queue, reported = false;
  const submit = GPUQueue.prototype.submit;
  GPUQueue.prototype.submit = function (...args) { queue = this; return submit.apply(this, args); };
  Object.defineProperty(globalThis, 'coaxial', {
    configurable: true, get: () => value,
    set(next) {
      value = next;
      if (!next || typeof next !== 'object') return;
      let ready = next.sceneReady;
      Object.defineProperty(next, 'sceneReady', {
        configurable: true, enumerable: true, get: () => ready,
        set(on) {
          ready = on;
          if (!on || reported || !queue) return;
          reported = true;
          queue.onSubmittedWorkDone().then(() => {
            globalThis.coaxFirstImage(JSON.stringify({epoch: performance.timeOrigin + performance.now(), ms: performance.now()}));
          });
        }
      });
    }
  });
})();
