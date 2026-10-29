// Serves a staged site (deploy.sh with COAXIAL_STAGE_TO: the exact bytes a
// release uploads) on this machine for golden-check.sh's walk, the way the
// bucket answers: a file by its path, a directory's page at its bare path
// (x.page) or as x/index.html. The API isn't here: reads (GET) go to
// staging's, where the goldens are taken (production's worlds hold what
// players placed: a golden never sees that), and anything sent (analytics,
// presence) is answered 204 and dropped, so a check never counts as a visit
// or writes anything.
//
//   bun golden-serve.ts SITE_DIR PORT
// Open it as http://golden.localhost:PORT: a secure context for WebGPU, but
// not a hostname host.js treats as development (no live channel, no notes).
import { existsSync, statSync } from "node:fs"
import { join, normalize } from "node:path"

const API = "https://staging.openagents.com"
const types: Record<string, string> = { wasm: "application/wasm", wgsl: "text/plain; charset=utf-8", md: "text/markdown; charset=utf-8",
  html: "text/html; charset=utf-8", js: "text/javascript; charset=utf-8", svg: "image/svg+xml", jpg: "image/jpeg", json: "application/json",
  txt: "text/plain; charset=utf-8", xml: "application/xml; charset=utf-8" }
const isFile = (p: string) => { try { return statSync(p).isFile() } catch { return false } }

export function resolve(root: string, pathname: string): { file: string; type: string } | null {
  const rel = normalize(decodeURIComponent(pathname)).replace(/^\/+|\/+$/g, "")
  if (rel.startsWith("..")) return null
  const at = join(root, rel)
  const html = types.html
  if (rel && isFile(at)) return { file: at, type: types[rel.split(".").pop() ?? ""] ?? "application/octet-stream" }
  if (rel && isFile(at + ".page")) return { file: at + ".page", type: html }
  if (isFile(join(at, "index.html"))) return { file: join(at, "index.html"), type: html }
  return null
}

if (import.meta.main) {
  const [site, port] = [process.argv[2], Number(process.argv[3] ?? 0)]
  if (!site || !existsSync(site)) { console.error("usage: bun golden-serve.ts SITE_DIR PORT"); process.exit(2) }
  const server = Bun.serve({ port, hostname: "127.0.0.1", async fetch(req) {
    const url = new URL(req.url)
    if (url.pathname.startsWith("/api/")) {
      console.log(`api ${req.method} ${url.pathname}${url.search}`)
      if (req.method !== "GET") return new Response(null, { status: 204 })
      const r = await fetch(API + url.pathname + url.search).catch(() => null)
      return r ? new Response(r.body, { status: r.status, headers: { "content-type": r.headers.get("content-type") ?? "application/json" } }) : new Response(null, { status: 502 })
    }
    const hit = resolve(site, url.pathname)
    if (!hit) { const nf = resolve(site, "/404.html"); return new Response(nf ? Bun.file(nf.file) : "not found", { status: 404, headers: { "content-type": types.html } }) }
    return new Response(Bun.file(hit.file), { headers: { "content-type": hit.type, "cache-control": "no-cache" } })
  } })
  console.log(`golden-serve ${site} on http://golden.localhost:${server.port}`)
}
