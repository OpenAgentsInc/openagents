// Firm reason: 1 (browser input clock and Chrome capture bridge).
globalThis.coaxialClip = true;
(() => {
  let wrapped, recording = false, elapsed = 0, next = 50;
  Object.defineProperty(globalThis, 'coaxialClipStep', {
    configurable: true,
    get: () => wrapped,
    set: (step) => {
      wrapped = async (frames = 1, fps = 30) => {
        if (!recording) return step(frames, fps);
        if (!Number.isInteger(frames) || frames < 1 || !Number.isFinite(fps) || fps <= 0)
          throw Error('Invalid clip step');
        const ms = 1000 / fps;
        let result;
        while (frames > 0) {
          const batch = Math.min(frames, Math.max(1, Math.ceil((next - elapsed) / ms - 1e-8)));
          result = await step(batch, fps);
          frames -= batch;
          elapsed += batch * ms;
          while (elapsed + 1e-6 >= next) {
            await new Promise(resolve => {
              globalThis.coaxRecordAck = resolve;
              globalThis.coaxRecordFrame('frame');
            });
            next += 50;
          }
        }
        return result;
      };
    },
  });
  globalThis.coaxRecordStart = () => {
    if (globalThis.coaxialClipAudio && !globalThis.coaxialClipMark?.())
      throw Error('The player has no offline audio timeline');
    recording = true;
  };
})();
