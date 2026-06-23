import { spawnSync } from "node:child_process"
import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, resolve } from "node:path"

// The typed token source is re-exported through @openagentsinc/ui (a direct
// dependency) so the desktop app does not need a separate design-tokens dep.
import { themeCss } from "@openagentsinc/ui/tokens"

// #6046: StyleX is gone. The desktop webview CSS is now just:
//   1. the generated `:root { --oa-*: … }` token block (the single typed token
//      source from @openagentsinc/design-tokens), and
//   2. the Tailwind v4 output (which also compiles the plain component CSS in
//      styles.css that replaced the deleted desktop-stylex.ts StyleX module).
// There is no longer a StyleX extraction/build step or `window`-dependent
// component CSS pipeline.

const desktopRoot = process.cwd()
const tailwindCss = resolve(desktopRoot, "src/ui/styles.tailwind.css")
const outputCss = resolve(desktopRoot, "src/ui/styles.out.css")
const viewBundleOut = resolve(desktopRoot, "resources/ui")

const tailwind = spawnSync(
  "bunx",
  ["@tailwindcss/cli", "-i", "src/ui/styles.css", "-o", tailwindCss],
  {
    cwd: desktopRoot,
    stdio: "inherit",
  },
)

if (tailwind.status !== 0) {
  process.exit(tailwind.status ?? 1)
}

await rm(viewBundleOut, { recursive: true, force: true })

const result = await Bun.build({
  entrypoints: ["src/ui/main.ts"],
  outdir: viewBundleOut,
  target: "browser",
})

if (!result.success) {
  console.error(result.logs.map(log => String(log)).join("\n"))
  process.exit(1)
}

await mkdir(dirname(outputCss), { recursive: true })

const tailwindOutput = await readFile(tailwindCss, "utf8")

// The generated typed-token custom properties come first so every `var(--oa-*)`
// reference in the component CSS resolves to the canonical token value.
const tokensBlock = `/* #6046: generated from @openagentsinc/design-tokens themeCss() — single typed token source. */\n${themeCss()}`

await writeFile(outputCss, `${tokensBlock}\n${tailwindOutput}\n`)

await rm(tailwindCss, { force: true })
