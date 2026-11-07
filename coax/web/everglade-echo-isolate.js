// Firm reason: 1 (headless draw-call transport for an explicitly labelled diagnostic; never loaded by a realm).
// Keep room state, spells, lights, animation and resource residency identical while suppressing
// only the admitted Echo profile's body/groom ranges. This cannot qualify a hero FPS result.
(() => {
  const draw = GPURenderPassEncoder.prototype.drawIndexed;
  GPURenderPassEncoder.prototype.drawIndexed = function (...args) {
    if (args[2] === 2999985 || args[2] === 4470303) return;
    return draw.apply(this, args);
  };
})();
