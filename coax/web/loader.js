// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// A realm's loading screen (INVARIANTS.md, "Coaxial pages draw everything in Coax": the one
// exception before the engine can draw). Its markup and style are written into every realm page
// by realms.ts, so it shows at the first paint, before host.js or the engine arrive; host.js then
// tells it what really loads: the engine's bytes, compiling it, the GPU, the shaders' pipelines,
// the world's asset bytes, its textures and the world's first frame. Each phase has a share of the
// bar and moves only by what was counted; nothing advances on a timer. It fades on the first frame
// that draws the world, and on a failure it says what failed and offers one action that works.
// Made realms (/realms/<slug>, written by the realm builder) have no markup of their own: the host
// puts the same in when the page names a realm.

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

// The new lander's tokens (apps/openagents-web/newland.coax, new-land/index.html): four greys and
// Arwes's electric blue for the bar only. The frame: a faint outline cut at 45° on two corners,
// bright brackets on the others.
export const LOADER_CSS = `#coaxial-load{position:fixed;inset:0;z-index:2;display:flex;align-items:center;justify-content:center;padding:0 16px;background:#000;color:#f0f0f0;font:500 11px/1.5 ui-monospace,"SF Mono",SFMono-Regular,Menlo,Consolas,monospace;letter-spacing:.14em;text-transform:uppercase;transition:opacity .35s ease-out}
#coaxial-load.gone{opacity:0;pointer-events:none}
#coaxial-load .lf{position:relative;box-sizing:border-box;width:min(360px,100%);padding:22px 22px 20px;background:rgba(240,240,240,.28);clip-path:polygon(0 0,calc(100% - 12px) 0,100% 12px,100% 100%,12px 100%,0 calc(100% - 12px))}
#coaxial-load .lf::before{content:"";position:absolute;inset:1px;background:#000;clip-path:polygon(0 0,calc(100% - 11.6px) 0,100% 11.6px,100% 100%,11.6px 100%,0 calc(100% - 11.6px))}
#coaxial-load .lf>*{position:relative}
#coaxial-load .lf>s{position:absolute;width:22px;height:13px;border:0 solid #f0f0f0}
#coaxial-load .lf>.c1{top:0;left:0;border-width:1px 0 0 1px}
#coaxial-load .lf>.c2{right:0;bottom:0;border-width:0 1px 1px 0}
#coaxial-load .ln{display:block;margin:0 0 18px;font:600 16px/1.3 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;letter-spacing:.2em;overflow-wrap:anywhere}
#coaxial-load .lr{display:grid;grid-template-columns:1fr auto;column-gap:12px}
#coaxial-load .lb{grid-column:1/3;display:block;height:2px;margin:0 0 10px;background:rgba(240,240,240,.14);overflow:hidden}
#coaxial-load .lb i{display:block;height:100%;background:hsl(180,68.07%,53.33%);transform:scaleX(0);transform-origin:0 50%;transition:transform .2s linear}
#coaxial-load .lt{color:#c7c7c7;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#coaxial-load .lp{color:#999;font-variant-numeric:tabular-nums}
#coaxial-load .le,#coaxial-load .la{display:none}
html:not(.gpu) #coaxial-load .lr,#coaxial-load[data-state=fail] .lr{display:none}
html:not(.gpu) #coaxial-load .le,#coaxial-load[data-state=fail] .le{display:block;color:#f0f0f0}
html:not(.gpu) #coaxial-load .la,#coaxial-load[data-state=fail] .la{display:inline-block;margin-top:16px;padding:8px 12px;border:1px solid rgba(240,240,240,.45);color:#f0f0f0;text-decoration:none}
#coaxial-load .la:hover,#coaxial-load .la:focus-visible{color:hsl(180,68.07%,53.33%);border-color:hsl(180,68.07%,53.33%);outline:none}
@media (prefers-reduced-motion:reduce){#coaxial-load,#coaxial-load .lb i{transition:none}}`;

/** The loading screen's markup for a realm titled `title`, whose text page is `href`. Without
 *  WebGPU (no .gpu on the page) it says so at once and links that text page. */
export function loaderMarkup(title, href) {
  const t = esc(title), h = esc(href);
  return `<!-- loader: coax/web/loader.js -->
<div id="coaxial-load" role="progressbar" aria-label="Loading ${t}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="0"><div class="lf"><s class="c1"></s><s class="c2"></s>
<b class="ln">${t}</b>
<div class="lr"><i class="lb"><i></i></i><span class="lt">ENGINE</span><span class="lp">0%</span></div>
<span class="le">NO WEBGPU IN THIS BROWSER</span>
<a class="la" href="${h}">READ ABOUT ${t}</a>
</div></div>
<!-- /loader -->`;
}

// The bar's shares, by phase; a phase the page doesn't have (no assets, no textures) has none.
const SHARE = { engine: 30, compile: 5, gpu: 3, shaders: 12, assets: 40, textures: 5, world: 5 };
const ORDER = ["engine", "compile", "gpu", "shaders", "assets", "textures", "world"];
const mb = (n) => (n / 1048576).toFixed(1);

/** Bytes of a response as they stream in, `progress(loaded, total)` told as they come (total 0:
 *  the response didn't say). */
export async function readBytes(response, progress) {
  // A gzipped file's Content-Length is its zipped size; the bucket names the unzipped one (upload.py).
  const total = Number(response.headers.get("x-goog-meta-length") || (response.headers.get("content-encoding") ? 0 : response.headers.get("content-length"))) || 0;
  if (!response.body) { const b = new Uint8Array(await response.arrayBuffer()); progress(b.length, b.length); return b; }
  const reader = response.body.getReader(), chunks = [];
  let loaded = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value); loaded += value.length;
    progress(loaded, Math.max(total, loaded));
  }
  if (chunks.length === 1) return chunks[0];
  const out = new Uint8Array(loaded);
  let at = 0; for (const c of chunks) { out.set(c, at); at += c.length; }
  return out;
}

const NONE = { on: false, engine() {}, step() {}, watch() {}, bytes() {}, textures() {}, assetDone() {}, frame() {}, fail() {} };

/** The page's loading screen, or one that does nothing on a page without one (a page that isn't a
 *  realm). `assets`: how many assets the host will load after the GPU; `textured`: any of them
 *  has textures; `scene`: the world is an indexed scene (its first frame is the first that
 *  draws it); `golden`: a golden walk, which removes it at once instead of fading. */
export function startLoading({ realm, assets = 0, textured = false, scene = false, golden = false }) {
  let el = document.getElementById("coaxial-load");
  if (!el && realm) {
    // a made realm: the same markup, named by the page's own heading
    const h1 = document.querySelector("main h1"), link = document.querySelector("main a[href]");
    const style = document.head.appendChild(document.createElement("style")); // loading screen
    style.textContent = LOADER_CSS;
    document.body.insertAdjacentHTML("afterbegin", loaderMarkup(h1?.textContent || "Realm", link?.getAttribute("href") || location.pathname)); // loading screen
    el = document.getElementById("coaxial-load");
  }
  if (!el) return NONE;
  const bar = el.querySelector(".lb i"), line = el.querySelector(".lt"), pct = el.querySelector(".lp");
  const share = { ...SHARE, assets: assets ? SHARE.assets : 0, textures: textured ? SHARE.textures : 0 };
  const whole = ORDER.reduce((s, k) => s + share[k], 0);
  const done = Object.fromEntries(ORDER.map((k) => [k, 0]));   // each phase's part done, 0 to 1
  let text = "ENGINE", shown = -1, queued = false, phase = "engine", finished = false, failed = false;
  const files = new Map(), tex = { done: 0, total: 0 };
  let left = assets, pipes = 0, pipesDone = 0, worldFrames = 0;

  const paint = () => {
    queued = false;
    if (finished && !failed) return;
    const f = ORDER.reduce((s, k) => s + share[k] * Math.min(1, done[k]), 0) / whole;
    const p = Math.max(shown, Math.floor(f * 100));   // never backwards
    if (p !== shown) { shown = p; bar.style.transform = `scaleX(${p / 100})`; pct.textContent = `${p}%`; el.setAttribute("aria-valuenow", String(p)); } // loading screen
    if (line.textContent !== text) line.textContent = text;
  };
  const later = () => { if (!queued) { queued = true; requestAnimationFrame(paint); } };
  // A phase begins: every phase before it is done.
  const enter = (k, t) => {
    const at = ORDER.indexOf(k);
    for (let i = 0; i < at; i++) done[ORDER[i]] = 1;
    phase = k; text = t; later();
  };
  const finish = () => {
    finished = true;
    if (golden) { el.remove(); return; }
    el.classList.add("gone");
    const away = () => el.remove();
    el.addEventListener("transitionend", away, { once: true });
    setTimeout(away, 600); // reduced motion has no transition to end
  };
  // While the assets come in: their bytes, then the textures decoded from them, then what's left.
  const within = () => {
    if (phase !== "assets" && phase !== "textures") return;
    let l = 0, t = 0; for (const [a, b] of files.values()) { l += a; t += b; }
    done.assets = t ? l / t : 0;
    done.textures = tex.total ? tex.done / tex.total : 0;
    if (l < t || !t) { phase = "assets"; text = t ? `ASSETS ${mb(l)} / ${mb(t)} MB` : "ASSETS"; }
    else if (textured && tex.done < tex.total) { phase = "textures"; text = `TEXTURES ${tex.done}/${tex.total}`; }
    else text = `ASSETS ${mb(l)} / ${mb(t)} MB`;
    later();
  };
  const assetsIn = () => {
    if (left > 0) return;
    enter("world", "WORLD");
  };

  return {
    on: true,
    engine(loaded, total) {
      if (finished) return;
      done.engine = total ? loaded / total : 0;
      text = total ? `ENGINE ${mb(loaded)} / ${mb(total)} MB` : `ENGINE ${mb(loaded)} MB`;
      later();
    },
    step(k) {
      if (finished) return;
      if (k === "compile") enter("compile", "COMPILING ENGINE");
      else if (k === "gpu") enter("gpu", "GPU DEVICE");
      else if (k === "shaders") enter("shaders", "SHADERS");
      else if (k === "assets") { if (assets) enter("assets", "ASSETS"); else assetsIn(); }
    },
    // Counts the device's pipelines as they're asked for and come back (SHADERS 7/9).
    watch(device) {
      const make = device.createRenderPipelineAsync.bind(device);
      device.createRenderPipelineAsync = (d) => {
        pipes += 1;
        const count = () => {
          pipesDone += 1;
          if (!finished && phase === "shaders") { done.shaders = pipesDone / pipes; text = `SHADERS ${pipesDone}/${pipes}`; later(); }
        };
        if (!finished && phase === "shaders") { text = `SHADERS ${pipesDone}/${pipes}`; later(); }
        const p = make(d); p.then(count, count); return p;
      };
    },
    bytes(key, loaded, total) {
      if (finished) return;
      files.set(key, [loaded, Math.max(total, loaded)]);
      within();
    },
    // Textures decoded: `n` more done, `total` more asked for.
    textures(n, total) {
      if (finished) return;
      tex.done += n; tex.total += total;
      within();
    },
    assetDone(failedWhat) {
      if (finished) return;
      if (failedWhat) { this.fail(failedWhat); return; }
      left -= 1;
      assetsIn();
    },
    // Each frame the host drew: the world is in when every asset is and the scene drew.
    frame(sceneDrawn) {
      if (finished || phase !== "world") return;
      worldFrames += 1;
      // a scene that draws nothing indexed for a long while still lets the visitor in
      if (!scene || sceneDrawn || worldFrames > 120) { done.world = 1; paint(); finish(); }
    },
    // What failed, plainly. No WebGPU: the realm's text page; anything else: try again.
    fail(what, nogpu = false) {
      if (finished) return;
      finished = true; failed = true;
      const le = el.querySelector(".le"), la = el.querySelector(".la");
      el.setAttribute("aria-valuenow", String(Math.max(0, shown)));
      if (nogpu) { document.documentElement.classList.remove("gpu"); el.dataset.state = "fail"; return; }   // the markup says it: no WebGPU, and the text page
      le.textContent = String(what).slice(0, 160);
      la.textContent = "TRY AGAIN";
      la.setAttribute("href", "");
      la.addEventListener("click", (e) => { e.preventDefault(); location.reload(); });
      el.dataset.state = "fail";
    },
  };
}
