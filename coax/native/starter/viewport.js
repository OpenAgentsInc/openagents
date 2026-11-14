// Firm reason: 1 (WebKit viewport and touch SDK effects for the native frame).
const frameStyle = document.createElement("style");
frameStyle.textContent = `/* The native window owns the frame; only its content scrolls. */
html, body {
  width: 100%;
  height: 100%;
  overflow: hidden;
  overscroll-behavior: none;
  touch-action: pan-y;
}
body { position: fixed; inset: 0; box-sizing: border-box; }
#coaxial-root {
  height: var(--native-height, 100%);
  max-height: 100%;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: none;
  touch-action: pan-y;
}
`;
document.head.append(frameStyle);
// iOS can ignore user-scalable=no; cancel its gesture events as well.
for (const name of ["gesturestart", "gesturechange", "gestureend"]) {
  document.addEventListener(name, event => event.preventDefault(), {passive: false});
}
document.addEventListener("touchmove", event => {
  if (event.touches.length > 1) event.preventDefault();
}, {passive: false});
// Keep fields reachable when the keyboard reduces the visible window.
const fit = () => document.documentElement.style.setProperty(
  "--native-height", `${globalThis.visualViewport?.height || innerHeight}px`);
globalThis.visualViewport?.addEventListener("resize", fit);
addEventListener("resize", fit);
fit();
