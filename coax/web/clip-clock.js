// Firm reason: 1 (headless page/worker clock fixture; no realm loads this script).
// A fixed step alone does not freeze wind/weather: their schedules use epoch time.
(() => {
  if (globalThis.coaxialClip !== true) return;
  const epoch = 1794009600000;
  globalThis.clipClockProof = { epoch, workers: 0 };
  Date.now = () => epoch;
  const NativeBlob = Blob;
  globalThis.Blob = class extends NativeBlob {
    constructor(parts = [], options) {
      const list = Array.from(parts);
      const room = list.some(part => typeof part === 'string' && part.includes('clipMs = null'));
      if (room) clipClockProof.workers++;
      super(room ? [`Date.now=()=>${epoch};\n`, ...list] : list, options);
    }
  };
})();
