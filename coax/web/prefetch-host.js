// Firm reason: 1 (browser host: the glue between the page's fetch, its HTTP cache and Coax's wasm)
// Action 18: warm another Coaxial page while this one still runs, as Character Create does while
// the character walks to Everglade (apps/openagents-web/charactercreator-walk.coax). It reads that
// page's host script tag and fetches the files host.js would load first for it (the engine, its
// shaders, the render package and the asset packs) into the browser's HTTP cache, so the page's
// own loader finds them there when it starts. Nothing fetched is kept or run here: this is not a
// second loader, only the first load moved earlier. Progress goes back as message 18,
// "STATE LOADED TOTAL": 0 under way, 1 every file fetched, 2 the page or a file failed.

const warming = new Map();

// The URLs host.js reads at start for a page's script tag (host.js: wasmUrl, wgslUrl, renderUrl,
// parseAssets). Sounds, private packs and local files load later or not at all, so they are left.
export function pageFiles(tag) {
  const d = tag.dataset, app = d.app || "app", files = [d.wasm || `/build/${app}.wasm`, d.wgsl || `/${app}.wgsl`];
  if (d.render) files.push(d.render);
  if (d.roomWasm) files.push(d.roomWasm);   // a deployed Everglade names its offline room (deploy.sh)
  for (const entry of (d.assets || "").split(/\s+/).filter(Boolean)) {
    const k = entry.indexOf(":"), kind = entry.slice(0, k), rest = entry.slice(k + 1);
    if (k <= 0 || ["sound", "sounds", "private", "local"].includes(kind)) continue;
    files.push(kind === "pack" ? rest : rest.split("|")[0]);
  }
  return files.filter((u) => u.startsWith("/") && !u.startsWith("//"));
}

export function warmPage(path, report) {
  const to = new URL(path, location.href);
  if (to.origin !== location.origin || !path.startsWith("/")) { report("2 0 0"); return; }
  if (warming.has(to.pathname)) { warming.get(to.pathname).then(report); return; }
  const job = run(to, report);
  warming.set(to.pathname, job);
  job.then(report);
}

async function run(to, report) {
  let loaded = 0, total = 0, told = 0;
  const tell = (force) => { const now = performance.now(); if (force || now - told > 150) { told = now; report(`0 ${loaded} ${total}`); } };
  try {
    report("0 0 0");
    const page = await fetch(to.href, { credentials: "same-origin" });
    if (!page.ok) return "2 0 0";
    const doc = new DOMParser().parseFromString(await page.text(), "text/html");
    const tag = doc.querySelector("script[data-app]");
    if (!tag) return "2 0 0";
    const files = [...new Set(pageFiles(tag))];
    const sizes = new Array(files.length).fill(0);
    const one = async (url, i) => {
      const r = await fetch(new URL(url, location.href).href, { credentials: "same-origin" });
      if (!r.ok || !r.body) throw new Error(`${url}: ${r.status}`);
      sizes[i] = Number(r.headers.get("content-length")) || 0; total = sizes.reduce((a, b) => a + b, 0); tell(true);
      // The body must be read to the end for the cache to keep it; the bytes themselves are dropped.
      const reader = r.body.getReader();
      for (;;) { const { done, value } = await reader.read(); if (done) break; loaded += value.length; if (loaded > total) total = loaded; tell(false); }
    };
    await Promise.all(files.map(one));
    return `1 ${loaded} ${Math.max(total, loaded)}`;
  } catch {
    return `2 ${loaded} ${total}`;
  }
}
