// Firm reason: 1 (browser host: the glue between the page's DOM, a same-origin frame and Coax's wasm)
// Action 18: open another Coaxial page in place while this one still runs, as Character Create
// does while the character walks to Everglade (apps/openagents-web/charactercreator-walk.coax).
// The page is loaded for real, by its own host.js, in a full-window frame under this page's
// canvas (which is opaque, so nothing of it shows): its engine, shaders, packs, textures and room
// seat, up to its first drawn frame. Then its world loop is paused until it is shown. Progress
// goes back as message 18, "STATE LOADED TOTAL" in percent: 0 under way, 1 up (drawn, and seated
// when it has a room), 2 failed. Action 1 to the same page then hands over in place (enterPage):
// the frame cross-fades in over this page, takes the input, the address and the title, and this
// page's engine stops. Nothing is loaded twice and no loading screen shows between the two.

const zones = new Map();   // pathname: { frame, to, state, pct, paused, report, timer }
const FADE_MS = 900;       // the cross-fade (charactercreator-walk.coax CC_WALK_FADE)

// A frame is up when its world drew and its loading screen has gone, and, for a page with a room
// seat (data-slot on its script), its room (host.js coaxial.room, everglade-net.coax net_probe 0)
// is open or has said it is down.
function look(z) {
  let win = null, doc = null;
  try { win = z.frame.contentWindow; doc = z.frame.contentDocument; } catch { return [2, 0]; }
  if (!win || !doc || !doc.documentElement) return [0, 0];
  const load = doc.getElementById("coaxial-load");
  if (load?.dataset.state === "fail") return [2, Number(load.getAttribute("aria-valuenow")) || 0];
  const tag = doc.querySelector("script[data-app]");
  if (z.loaded && !tag) return [2, 0];
  const pct = load && !load.classList.contains("gone") ? Number(load.getAttribute("aria-valuenow")) || 0 : 100;
  const c = win.coaxial, drawn = !!c?.sceneReady && pct >= 100, room = c?.room?.() ?? -1;
  if (drawn && (!tag?.dataset.slot || room === 2 || room === 3)) return [1, 100];
  return [0, Math.min(pct, 99)];
}
function poll(z) {
  if (z.state !== 0) return;
  const [state, pct] = look(z);
  z.pct = Math.max(z.pct, pct);
  if (state === 1 && !z.paused) {
    // Up: hold its world still (sockets still deliver) until it is shown, so the walk keeps the GPU.
    try { z.frame.contentWindow.coaxial.setWorldPaused(true); z.paused = true; } catch {}
  }
  z.state = state;
  if (state !== 0) clearInterval(z.timer);
  z.report(`${state} ${state === 1 ? 100 : z.pct} 100`);
}

export function warmPage(path, report) {
  const to = new URL(path, location.href);
  if (to.origin !== location.origin || !path.startsWith("/") || window.top !== window) { report("2 0 0"); return; }
  const had = zones.get(to.pathname);
  if (had) { had.report = report; report(`${had.state} ${had.state === 1 ? 100 : had.pct} 100`); return; }
  const frame = document.createElement("iframe"); // zone frame
  frame.dataset.zone = "1";
  frame.title = to.pathname;
  frame.tabIndex = -1;
  frame.inert = true;
  frame.setAttribute("aria-hidden", "true");
  frame.allow = "fullscreen; autoplay; microphone; camera; clipboard-read; clipboard-write; gamepad; screen-wake-lock";
  // Under this page's opaque canvas (earlier in the body, the same fixed box) until it is shown.
  frame.style.cssText = "position:fixed;inset:0;width:100%;height:100%;border:0;margin:0;padding:0;background:#000;pointer-events:none"; // zone frame
  const z = { frame, to, state: 0, pct: 0, paused: false, loaded: false, report, timer: 0 };
  frame.addEventListener("load", () => { z.loaded = true; poll(z); });
  frame.src = to.href;
  document.body.prepend(frame);
  zones.set(to.pathname, z);
  z.timer = setInterval(() => poll(z), 150);
  report("0 0 100");
}

/** Action 1 to a page opened by warmPage: show it in place and return true (`stop` then retires
 *  this page's engine, once the cross-fade is over). False when there is no such page, or it
 *  failed: the caller navigates as usual. A page not yet up is shown as it is, its own loading
 *  screen and all, rather than started over. */
export function enterPage(path, stop) {
  const to = new URL(path, location.href), z = zones.get(to.pathname);
  if (!z) return false;
  clearInterval(z.timer);
  let win = null;
  try { win = z.frame.contentWindow; } catch {}
  if (z.state === 2 || !win) { zones.delete(to.pathname); z.frame.remove(); return false; }
  zones.delete(to.pathname);
  const frame = z.frame;
  try { if (z.paused) win.coaxial.setWorldPaused(false); } catch {}
  frame.inert = false;
  frame.removeAttribute("aria-hidden");
  frame.tabIndex = 0;
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  frame.style.opacity = "0"; // zone frame
  frame.style.zIndex = "3"; // zone frame
  frame.style.pointerEvents = "auto"; // zone frame
  let done = false;
  const over = () => {
    if (done) return;
    done = true;
    stop();
    // The page is the frame's now: its address (Back comes here again, as a new load), its title, its input.
    try { history.pushState(null, "", win.location.href); } catch {}
    try { document.title = z.frame.contentDocument.title; } catch {}
    addEventListener("popstate", () => location.reload());
    try { win.addEventListener("hashchange", () => history.replaceState(null, "", win.location.href)); } catch {}
    frame.focus();
    try { win.focus(); } catch {}
  };
  requestAnimationFrame(() => requestAnimationFrame(() => {
    if (still) { frame.style.opacity = "1"; over(); return; } // zone frame
    frame.style.transition = `opacity ${FADE_MS}ms ease-in-out`; // zone frame
    frame.style.opacity = "1"; // zone frame
    frame.addEventListener("transitionend", over, { once: true });
    setTimeout(over, FADE_MS + 200);
  }));
  return true;
}
