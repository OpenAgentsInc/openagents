// A page's own share card (docs/web/og-tags.md): a page whose app script has data-card=URL (a
// realm made at /realms/new, coax/web/api/migrations/0071_made_realm_share.sql) asks the server
// whether this visitor should take its picture (GET URL: "want" or "no", its owner when it has none
// or it changed since). If so, once the scene is drawn and has settled, one frame is copied in the
// same task it was drawn in (host.js frame(): a WebGPU canvas keeps its picture only until it is
// shown), drawn at 1200x630 under the page's title and line in the site's card look, and sent back
// as a JPEG under 300 KB (POST URL). A browser without WebGPU sends the title and line alone.
// While the owner stays, it asks again every minute, so a change they make is pictured too.
// Browser code: it needs the canvas and Blob APIs, so it is JavaScript like the rest of the host.
const W = 1200, H = 630, MAX = 300 * 1024, SETTLE_MS = 3000, AGAIN_MS = 60000;

export function startCard({ url, key, snapshot, ready }) {
  if (!url || typeof document === "undefined") return;
  const headers = () => (key() ? { "X-Verse-Player": key() } : {});
  let busy = false;
  const round = async () => {
    if (busy || document.hidden) return;
    busy = true;
    try {
      const want = await fetch(url, { headers: headers(), cache: "no-store" }).then((r) => (r.ok ? r.text() : "no"));
      if (want.trim() !== "want") return;
      const until = Date.now() + 60000;
      while (!ready() && Date.now() < until) await sleep(500);
      await sleep(SETTLE_MS);
      const shot = ready() ? await snapshot() : null;
      const frame = shot && !blank(shot) ? shot : null;
      const blob = await jpeg(compose(frame, meta("og:title") || document.title, meta("og:description")));
      if (!blob || blob.size > MAX) return;
      await fetch(url, { method: "POST", headers: { ...headers(), "Content-Type": "image/jpeg" }, body: blob });
    } catch {
      // a card is a convenience: the page goes on without one
    } finally {
      busy = false;
    }
  };
  setTimeout(round, 1000);
  setInterval(round, AGAIN_MS);
}

// A frame the browser couldn't copy comes back black: then the card is the title and line alone.
function blank(c) {
  try {
    const t = document.createElement("canvas"); t.width = 32; t.height = 18;
    const g = t.getContext("2d", { willReadFrequently: true });
    g.drawImage(c, 0, 0, 32, 18);
    const px = g.getImageData(0, 0, 32, 18).data;
    let peak = 0;
    for (let i = 0; i < px.length; i += 4) peak = Math.max(peak, px[i], px[i + 1], px[i + 2]);
    return peak < 12;
  } catch { return true; }
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const meta = (p) => document.querySelector(`meta[property="${p}"]`)?.getAttribute("content") || "";

// The card: the frame filling it (cropped to 1200x630 from its middle), a dark band at the foot
// with the corner marks, OPENAGENTS / VERSE, the name in thin capitals and the line.
export function compose(frame, title, line) {
  const c = document.createElement("canvas");
  c.width = W; c.height = H;
  const g = c.getContext("2d");
  g.fillStyle = "#000"; g.fillRect(0, 0, W, H);
  const band = frame ? 180 : H;
  if (frame) {
    const fw = frame.width, fh = frame.height, s = Math.max(W / fw, (H - band + 60) / fh);
    const dw = fw * s, dh = fh * s;
    g.drawImage(frame, (W - dw) / 2, (H - band + 60 - dh) / 2, dw, dh);
  } else {
    // no frame: stars on black, as the site's line-art cards
    g.fillStyle = "rgba(255,255,255,0.35)";
    for (let i = 0; i < 90; i++) { const x = (i * 7919) % W, y = (i * 104729) % H; g.fillRect(x, y, 1.5, 1.5); }
  }
  const top = H - band;
  if (frame) {
    const fade = g.createLinearGradient(0, top - 60, 0, top);
    fade.addColorStop(0, "rgba(0,0,0,0)"); fade.addColorStop(1, "#000");
    g.fillStyle = fade; g.fillRect(0, top - 60, W, 60);
    g.fillStyle = "#000"; g.fillRect(0, top, W, band);   // opaque: the HUD under it never shows through
  }
  g.strokeStyle = "rgba(255,255,255,0.9)"; g.lineWidth = 2;
  const mark = (x, y, dx, dy) => { g.beginPath(); g.moveTo(x, y + dy * 32); g.lineTo(x, y); g.lineTo(x + dx * 54, y); g.stroke(); };
  mark(20, top + 12, 1, 1); mark(W - 20, H - 12, -1, -1);
  const sans = 'ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';
  const small = frame ? top + 36 : 110;
  g.fillStyle = "rgba(255,255,255,0.7)"; g.font = `400 15px ${sans}`;
  spaced(g, "OPENAGENTS  /  VERSE", 62, small, 2.5);
  g.textAlign = "right"; spaced(g, "OPENAGENTS.COM", W - 62, small, 2.5, true); g.textAlign = "left";
  // the name: thin capitals drawn as outlines, the line-art look
  let size = 58;
  const name = String(title || "").toUpperCase();
  g.font = `200 ${size}px ${sans}`;
  while (size > 28 && g.measureText(name).width + name.length * 6 > W - 124) { size -= 2; g.font = `200 ${size}px ${sans}`; }
  g.strokeStyle = "#fff"; g.lineWidth = 1.6;
  spaced(g, name, 62, frame ? top + 98 : 250, 6, false, true);
  g.fillStyle = "rgba(205,210,225,0.9)"; g.font = `400 22px ${sans}`;
  g.fillText(fit(g, String(line || ""), W - 124), 64, frame ? top + 146 : 320);
  return c;
}

function spaced(g, text, x, y, gap, right = false, outline = false) {
  let w = 0;
  for (const ch of text) w += g.measureText(ch).width + gap;
  const align = g.textAlign;
  g.textAlign = "left";
  let at = right ? x - w : x;
  for (const ch of text) {
    if (outline) g.strokeText(ch, at, y); else g.fillText(ch, at, y);
    at += g.measureText(ch).width + gap;
  }
  g.textAlign = align;
}
function fit(g, text, width) {
  if (g.measureText(text).width <= width) return text;
  let t = text;
  while (t.length > 1 && g.measureText(t + "...").width > width) t = t.slice(0, t.lastIndexOf(" ") > 0 ? t.lastIndexOf(" ") : t.length - 1);
  return t.replace(/[,;:.]$/, "") + "...";
}

// A JPEG under 300 KB: lower quality until it fits.
async function jpeg(canvas) {
  for (const q of [0.86, 0.78, 0.68, 0.55, 0.4]) {
    const b = await new Promise((r) => canvas.toBlob(r, "image/jpeg", q));
    if (b && b.size <= MAX) return b;
  }
  return null;
}
