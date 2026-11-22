// Firm reason: 1 (thin browser input glue for Fated Dragon; all gameplay and physics are in Coax).
const BITS = {
  KeyW: 1, KeyS: 2, KeyA: 4, KeyD: 8, Space: 16,
  ArrowLeft: 32, ArrowRight: 64, ArrowUp: 128, ArrowDown: 256,
  Equal: 512, NumpadAdd: 512, Minus: 1024, NumpadSubtract: 1024,
  ShiftLeft: 2048, ShiftRight: 2048, KeyE: 4096,
};
let held = 0, sent = -1;
const say = (kind, text) => { try { window.coaxial?.send?.(kind, String(text)); } catch {} };
const flush = () => {
  if (!window.coaxial?.ready || held === sent) return;
  sent = held;
  say(104, held);
};
addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const b = BITS[e.code] || 0;
  if (!b) return;
  e.preventDefault();
  if (e.repeat) return;
  held |= b;
  flush();
});
addEventListener("keyup", (e) => {
  const b = BITS[e.code] || 0;
  if (!b) return;
  held &= ~b;
  flush();
});
addEventListener("blur", () => { held = 0; sent = -1; });
addEventListener("wheel", (e) => {
  if (!window.coaxial?.ready) return;
  e.preventDefault();
  const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * 320 : e.deltaY;
  say(108, Math.round(dy));
}, { passive: false });
const hideHint = () => document.getElementById("fd-hint")?.classList.add("dim");
addEventListener("keydown", hideHint, { once: true });
addEventListener("pointerdown", hideHint, { once: true });
const syncReady = () => {
  if (window.coaxial?.ready) { flush(); return; }
  requestAnimationFrame(syncReady);
};
requestAnimationFrame(syncReady);
