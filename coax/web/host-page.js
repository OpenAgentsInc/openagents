// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Split from coax/web/host.js (#13848): page/visit environment facts, storage IDs, and lifecycle beacons.

export const randomId = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");

// `store` gives the storage: naming it can throw too, in a sandboxed page with no origin (a realm on
// the realm origin, #13432), so it is asked for inside the try.
export function kept(store, key) {
  try { const s = store(), v = s.getItem(key); if (v) return [v, false]; const n = randomId(); s.setItem(key, n); return [n, true]; }
  catch { return [randomId(), true]; }   // storage refused (a private window, a sandboxed frame): this page only
}

// The realm's style, zones and clock (docs/metaverse/art-styles.md, The style field).
export function styleFacts(script) {
  const d = script?.dataset || {}, out = { style: d.style || "" };
  for (const z of (d.zones || "").split(/\s+/)) { const [id, st, v] = z.split(":"); if (id && st) out[`zone.${id}`] = st; if (id && st && v) out[`zone.${id}.variant`] = v; }
  // an anime realm's variant (data-variant); the URL's ?variant= picks another for shots
  const pick = new URLSearchParams(location.search).get("variant");
  if (d.variant || pick) out.variant = pick && /^[a-z]{1,16}$/.test(pick) ? pick : d.variant || "";
  const pin = new URLSearchParams(location.search).get("clock");
  out.clock = pin !== null && /^(0(\.\d{1,4})?|1(\.0{1,4})?)$/.test(pin) ? `fixed:${pin}` : pin === "cycle" || pin === "real" ? pin : d.clock || "";
  // the visitor's local time of day, 0 midnight to 1, for a clock that is real
  const t = new Date();
  out.day = ((t.getHours() * 3600 + t.getMinutes() * 60 + t.getSeconds()) / 86400).toFixed(5);
  return out;
}

export function buildPageFacts({ script, agentPin, storedQuality, gpuFacts, player, realmId, slot }) {
  const [tab, fresh] = kept(() => sessionStorage, "coaxial.tab"), [visitor] = kept(() => localStorage, "coaxial.visitor");
  let tz = ""; try { tz = Intl.DateTimeFormat().resolvedOptions().timeZone || ""; } catch {}
  const nav = performance.getEntriesByType?.("navigation")?.[0], fcp = performance.getEntriesByName?.("first-contentful-paint")?.[0];
  const clean = (v) => String(v ?? "").replace(/[\r\n]/g, " ");
  return Object.entries({ tab, fresh: fresh ? 1 : 0, visitor, now: Date.now(), nav: performance.now().toFixed(1), url: location.href,
    ref: document.referrer, ua: navigator.userAgent, lang: agentPin?.locale || navigator.language, tz: agentPin?.timeZone || tz, sw: screen.width, sh: screen.height,
    vw: innerWidth, vh: innerHeight, dpr: devicePixelRatio || 1, coarse: matchMedia("(pointer: coarse)").matches ? 1 : 0,
    touch: navigator.maxTouchPoints || 0, gpu: navigator.gpu ? 1 : 0, hidden: document.hidden ? 1 : 0,
    cores: navigator.hardwareConcurrency || 0, memory: navigator.deviceMemory || 0, quality: storedQuality(),
    settled: (() => { try { return localStorage.getItem("verse.quality.settled") || ""; } catch { return ""; } })(), ...gpuFacts,
    frame_hz: globalThis.coaxialPreferredFrameRate || 0, minimum_hz: globalThis.coaxialMinimumFrameRate || 0,
    sound: (() => { try { return localStorage.getItem("verse.sound") || ""; } catch { return ""; } })(),
    settings: (() => { try { return localStorage.getItem("verse.settings") || ""; } catch { return ""; } })(),
    gyro: (() => { try { return localStorage.getItem("verse.gyro") || ""; } catch { return ""; } })(),
    focused: document.hasFocus() ? 1 : 0,
    clip: globalThis.coaxialClip === true ? 1 : 0,
    webdriver: navigator.webdriver ? 1 : 0, fcp: fcp ? Math.round(fcp.startTime) : 0, dcl: nav ? Math.round(nav.domContentLoadedEventEnd) : 0,
    player: player()?.id || "", account: player()?.account || "", realm: realmId, slot, voice: script?.dataset.voice || "", ruleset: script?.dataset.ruleset || "",
    fork: script?.dataset.fork || "", rules: script?.dataset.rules || "", ...styleFacts(script) })
    .map(([k, v]) => `${k}=${clean(v)}`).join("\n");
}

// What stopped the page, which the page's own analytics (analytics.coax) can't say because its engine
// may be gone or never came: a load that failed (load-fail), a lost GPU device (gpu-lost) and an engine
// or renderer that stopped (crash).
export function sendLifeBeacon(realmId, app, ev, msg, more = null) {
  if (location.protocol === "file:" || !navigator.sendBeacon) return;
  const [tab] = kept(() => sessionStorage, "coaxial.tab");
  if (!/^[0-9a-f]{8,64}$/.test(tab)) return;
  const body = { sid: `c${tab}`, events: [{ ev, page: realmId || app, ct: Date.now(), p: { msg: String(msg || "").slice(0, 80), webgpu: !!navigator.gpu, ...(more || {}) } }] };
  if (navigator.webdriver) body.auto = "webdriver";
  try { navigator.sendBeacon("/api/analytics/events", new Blob([JSON.stringify(body)], { type: "text/plain" })); } catch {}
}
