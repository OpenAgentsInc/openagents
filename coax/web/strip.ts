// Removes every comment from what the site ships, after deploy.sh stages it:
// the sources keep their comments, browsers never see them.
//   bun strip.ts STAGE_DIR
// HTML comments, CSS comments in <style>, JS comments in inline scripts and
// .js files (Bun's transpiler, whitespace only: no renaming), WGSL comments.
// Markdown, JSON-LD and images pass through untouched.
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { join } from "node:path"

const js = new Bun.Transpiler({ loader: "js", minifyWhitespace: true })
const script = (code: string) => js.transformSync(code).trim()
const css = (code: string) => code.replace(/\/\*[\s\S]*?\*\//g, "")
// WGSL has no string literals, so a comment can't hide inside one.
export const wgsl = (code: string) => code.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "").replace(/\n\s*\n+/g, "\n")
export function html(text: string): string {
  return text
    .replace(/<!--[\s\S]*?-->\n?/g, "")
    .replace(/(<style[^>]*>)([\s\S]*?)(<\/style>)/g, (_, a, body, b) => a + css(body) + b)
    .replace(/(<script(?![^>]*\bsrc=)(?![^>]*application\/(?:ld\+)?json)[^>]*>)([\s\S]*?)(<\/script>)/g,
      (_, a, body, b) => a + (body.trim() ? script(body) : body) + b)
}

function walk(dir: string, out: string[] = []) {
  for (const f of readdirSync(dir)) {
    const p = join(dir, f)
    if (statSync(p).isDirectory()) walk(p, out); else out.push(p)
  }
  return out
}

if (import.meta.main) {
  const stage = process.argv[2]
  if (!stage) { console.error("usage: bun strip.ts STAGE_DIR"); process.exit(2) }
  let n = 0
  for (const p of walk(stage)) {
    const text = () => readFileSync(p, "utf8")
    let next: string | null = null
    if (p.endsWith(".html") || p.endsWith(".page")) next = html(text())
    else if (p.endsWith(".js")) next = script(text()) + "\n"
    else if (p.endsWith(".wgsl")) next = wgsl(text())
    if (next !== null) { writeFileSync(p, next); n++ }
  }
  console.log(`  strip    ${n} files without comments`)
}
