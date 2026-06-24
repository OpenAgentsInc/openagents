import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"

import {
  findIsolatedSceneDefinition,
  isolatedSceneUsage,
} from "./isolated-scenes/registry.js"

const sceneName = process.argv[2] ?? ""
const definition = findIsolatedSceneDefinition(sceneName)

if (definition === null) {
  console.error(`Unknown isolated scene '${sceneName}'. ${isolatedSceneUsage()}`)
  process.exit(1)
}

const tmpRoot = mkdtempSync(join(process.cwd(), ".isolated-scene-"))
const distDir = join(tmpRoot, "dist")
const htmlPath = join(tmpRoot, "index.html")

writeFileSync(
  htmlPath,
  `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${definition.title}</title>
    <style>
      :root { color-scheme: dark; background: #050505; }
      body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #050505; }
      #scene { width: ${definition.defaultWidth}px; height: ${definition.defaultHeight}px; background: #050505; }
    </style>
  </head>
  <body>
    <div id="scene"></div>
    <script type="module" src="/dist/${sceneName}.js"></script>
  </body>
</html>
`,
)

const build = await Bun.build({
  entrypoints: [definition.entryModulePath],
  outdir: distDir,
  naming: `${sceneName}.js`,
  target: "browser",
  sourcemap: "none",
  minify: false,
})

if (!build.success) {
  for (const log of build.logs) console.error(log)
  rmSync(tmpRoot, { force: true, recursive: true })
  process.exit(1)
}

const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(process.env.OA_ISOLATED_SCENE_PORT ?? "0"),
  fetch(request) {
    const url = new URL(request.url)
    if (url.pathname === "/") {
      return new Response(readFileSync(htmlPath), {
        headers: { "content-type": "text/html; charset=utf-8" },
      })
    }
    if (url.pathname === `/dist/${sceneName}.js`) {
      return new Response(readFileSync(join(distDir, `${sceneName}.js`)), {
        headers: { "content-type": "text/javascript; charset=utf-8" },
      })
    }
    return new Response("not found", { status: 404 })
  },
})

console.log(`${definition.title}: http://127.0.0.1:${server.port}/`)
console.log("Press Ctrl-C to stop.")

const shutdown = (): void => {
  server.stop(true)
  rmSync(tmpRoot, { force: true, recursive: true })
  process.exit(0)
}

process.on("SIGINT", shutdown)
process.on("SIGTERM", shutdown)

await new Promise(() => {})
