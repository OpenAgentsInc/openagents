// GPU time per frame (a timestamp query on every render pass) and the frames' spacing, in headless
// Chrome, for a page's water and the like (docs/verse/water.md, W11):
//   bun coax/web/frametime.ts URL [WxH] [DPR] [SECONDS] [--phone] [--eval JS] [--skip PASSES] [--vsync]
// Frames run unthrottled unless --vsync. --phone emulates a touch phone; --eval runs JS (an async
// IIFE) before measuring; --skip drops the draws of the numbered passes (0 first) to cost them.
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
const [url, size = "1280x800", dprS = "1", secS = "8"] = process.argv.slice(2)
const evalAt = process.argv.indexOf("--eval"); const script = evalAt > 0 ? process.argv[evalAt + 1] : ""
const phone = process.argv.includes("--phone")
const skipAt = process.argv.indexOf("--skip"); const skip = skipAt > 0 ? process.argv[skipAt + 1] : ""
const [w, h] = size.split("x").map(Number), dpr = Number(dprS)
const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "coax-ft-"))
const proc = Bun.spawn([chrome, "--headless=new", `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars",
  "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal", "--enable-dawn-features=allow_unsafe_apis",
  "--disable-dawn-features=timestamp_quantization", ...(process.argv.includes("--vsync") ? [] : ["--disable-frame-rate-limit", "--disable-gpu-vsync"]),
  `--window-size=${w},${h}`, "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
const inject = `(() => {
 window.__skip = new Set('${skip}'.split(',').filter(Boolean).map(Number));
 window.__gpu = []; window.__raf = []; window.__info = {};
 const req = GPUAdapter.prototype.requestDevice;
 GPUAdapter.prototype.requestDevice = function (d = {}) {
   const f = [...(d.requiredFeatures || [])]; window.__info.ts = this.features.has('timestamp-query');
   try { window.__info.adapter = JSON.stringify({ vendor: this.info?.vendor, arch: this.info?.architecture, desc: this.info?.description }); } catch {}
   if (window.__info.ts) f.push('timestamp-query'); return req.call(this, { ...d, requiredFeatures: f }); };
 const MAXP = 16;
 const enc = GPUDevice.prototype.createCommandEncoder;
 GPUDevice.prototype.createCommandEncoder = function (d) { const e = enc.call(this, d);
   if (this.features.has('timestamp-query')) { e.__dev = this; e.__n = 0; e.__qs = this.__qs ||= this.createQuerySet({ type: 'timestamp', count: MAXP * 2 }); } return e; };
 const brp = GPUCommandEncoder.prototype.beginRenderPass;
 GPUCommandEncoder.prototype.beginRenderPass = function (d) {
   if (this.__qs && this.__n < MAXP && !d.timestampWrites) { const k = this.__n++; d = { ...d, timestampWrites: { querySet: this.__qs, beginningOfPassWriteIndex: 2 * k, endOfPassWriteIndex: 2 * k + 1 } }; }
   const k0 = (this.__all = (this.__all || 0) + 1) - 1; const p = brp.call(this, d); if (window.__skip.has(k0)) { p.draw = () => {}; p.drawIndexed = () => {}; } return p; };
 const fin = GPUCommandEncoder.prototype.finish;
 GPUCommandEncoder.prototype.finish = function (d) {
   if (this.__qs && this.__n) { const dev = this.__dev, n = this.__n;
     const res = dev.createBuffer({ size: MAXP * 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
     const rd = dev.createBuffer({ size: MAXP * 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
     this.resolveQuerySet(this.__qs, 0, 2 * n, res, 0); this.copyBufferToBuffer(res, 0, rd, 0, 16 * n);
     const cb = fin.call(this, d); cb.__rd = rd; cb.__n = n; cb.__res = res; return cb; }
   return fin.call(this, d); };
 const sub = GPUQueue.prototype.submit;
 GPUQueue.prototype.submit = function (cbs) { const r = sub.call(this, cbs);
   for (const cb of cbs) if (cb.__rd) { const rd = cb.__rd, n = cb.__n, res = cb.__res;
     rd.mapAsync(GPUMapMode.READ).then(() => { const a = new BigUint64Array(rd.getMappedRange()); const per = [];
       for (let k = 0; k < n; k++) per.push(Number(a[2 * k + 1] - a[2 * k]) / 1e6);
       window.__gpu.push({ total: per.reduce((x, y) => x + y, 0), per, frame: Number(a[2 * n - 1] - a[0]) / 1e6 }); rd.unmap(); rd.destroy(); res.destroy(); }).catch(() => {}); }
   return r; };
 let last = 0; const loop = (t) => { if (last) window.__raf.push(t - last); last = t; requestAnimationFrame(loop); }; requestAnimationFrame(loop);
})();`
try {
  const portFile = join(profile, "DevToolsActivePort")
  for (let i = 0; i < 100 && !existsSync(portFile); i++) await Bun.sleep(50)
  const port = readFileSync(portFile, "utf8").split("\n")[0]
  const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
  const ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise(r => ws.addEventListener("open", r, { once: true }))
  let id = 0; const pending = new Map<number, (v: any) => void>()
  ws.addEventListener("message", e => { const m = JSON.parse(String(e.data)); pending.get(m.id)?.(m); pending.delete(m.id) })
  const send = (method: string, params: any = {}) => new Promise<any>(r => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
  await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: dpr, mobile: phone })
  await send("Page.enable"); await send("Runtime.enable")
  if (phone) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 })
  await send("Page.addScriptToEvaluateOnNewDocument", { source: inject })
  await send("Page.navigate", { url })
  await Bun.sleep(7000)
  if (script) { await send("Runtime.evaluate", { expression: script, awaitPromise: true }); await Bun.sleep(1000) }
  await send("Runtime.evaluate", { expression: "window.__gpu.length = 0; window.__raf.length = 0" })
  await Bun.sleep(Number(secS) * 1000)
  const r = await send("Runtime.evaluate", { expression: "JSON.stringify({info: window.__info, gpu: window.__gpu, raf: window.__raf, canvas: [...document.querySelectorAll('canvas')].map(c => [c.width, c.height, c.clientWidth]), dpr: devicePixelRatio, coarse: matchMedia('(pointer: coarse)').matches})", returnByValue: true })
  const dbg = await send("Runtime.evaluate", { expression: "JSON.stringify([typeof window.__gpu, typeof GPUAdapter, typeof navigator.gpu, location.href, document.documentElement.className])", returnByValue: true }); const d = JSON.parse(r.result.result.value)
  const q = (a: number[], p: number) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(p * s.length))] : NaN }
  const f = (x: number) => x.toFixed(2)
  const tot = d.gpu.map((g: any) => g.total), span = d.gpu.map((g: any) => g.frame)
  const passes = d.gpu.length ? d.gpu[0].per.length : 0
  const perPass = Array.from({ length: passes }, (_, k) => f(q(d.gpu.filter((g: any) => g.per.length === passes).map((g: any) => g.per[k]), 0.5)))
  console.log(JSON.stringify({ url, size, dpr, pageDpr: d.dpr, coarse: d.coarse, canvas: d.canvas, info: d.info, frames: d.gpu.length,
    gpu_ms: { p50: f(q(tot, 0.5)), p95: f(q(tot, 0.95)), max: f(q(tot, 1)) }, gpu_span_p50: f(q(span, 0.5)), per_pass_p50: perPass,
    raf_ms: { n: d.raf.length, p50: f(q(d.raf, 0.5)), p95: f(q(d.raf, 0.95)) } }))
  ws.close()
} finally { proc.kill(); rmSync(profile, { recursive: true, force: true }) }
