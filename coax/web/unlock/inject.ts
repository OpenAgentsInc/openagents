// Deploy step: adds hidden presentations to a staged site, from the
// COAXIAL_UNLOCKS environment variable. Each entry is
//   PHRASE=RELAY,EVENT_ID,RECIPIENT_PUBKEY
// separated by newlines or semicolons. The phrase never reaches the page:
// index.html gets a salted check and an encrypted pointer per phrase, plus
// the gate script; the opener module is a separate, content-named file the
// gate loads only after a match.
//   bun inject.ts STAGE     (STAGE holds site/ and assets/, as deploy.sh stages them)
import { createHash } from "node:crypto"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { makeEntry, type Pointer } from "./gate.ts"

export function parseUnlocks(text: string): { phrase: string; pointer: Pointer }[] {
  return text.split(/[\n;]/).map((l) => l.trim()).filter((l) => l && !l.startsWith("#")).map((line) => {
    const eq = line.indexOf("=")
    const [relay, id, recipient] = line.slice(eq + 1).split(",").map((s) => s.trim())
    if (eq < 1 || !/^wss?:\/\//.test(relay ?? "") || !/^[0-9a-f]{64}$/.test(id ?? "") || !/^[0-9a-f]{64}$/.test(recipient ?? ""))
      throw new Error("COAXIAL_UNLOCKS entries are PHRASE=RELAY,EVENT_ID,RECIPIENT_PUBKEY")
    return { phrase: line.slice(0, eq), pointer: { relay, id, recipient } }
  })
}

async function bundle(entry: string, stage: string, name: string): Promise<string> {
  const out = await Bun.build({ entrypoints: [join(import.meta.dir, entry)], target: "browser", minify: true })
  if (!out.success) throw new Error(out.logs.join("\n"))
  const bytes = new Uint8Array(await out.outputs[0].arrayBuffer())
  const named = `coaxial/${name}.${createHash("sha256").update(bytes).digest("hex").slice(0, 12)}.js`
  for (const root of ["assets", "site"]) {
    mkdirSync(dirname(join(stage, root, named)), { recursive: true })
    writeFileSync(join(stage, root, named), bytes)
  }
  return "/" + named
}

if (import.meta.main) {
  const stage = process.argv[2]
  const unlocks = parseUnlocks(process.env.COAXIAL_UNLOCKS ?? "")
  if (unlocks.length) {
    const [gate, module] = await Promise.all([bundle("gate.ts", stage, "gate"), bundle("page.ts", stage, "unlock")])
    const entries = await Promise.all(unlocks.map((u) => makeEntry(u.phrase, u.pointer)))
    const page = join(stage, "site", "index.html")
    const config = JSON.stringify({ module, entries }).replace(/</g, "\\u003c")
    const html = readFileSync(page, "utf8").replace("</body>",
      `<script type="application/json" id="coaxial-unlocks">${config}</script>\n<script type="module" src="${gate}"></script>\n</body>`)
    writeFileSync(page, html)
    console.log(`  unlocks  ${unlocks.length}`)
  }
}
