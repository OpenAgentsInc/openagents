import { spawnSync } from "node:child_process"
import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, resolve } from "node:path"
import { createStylexBunPlugin } from "@stylexjs/unplugin/bun"

type StylexBunOptions = NonNullable<
  Parameters<typeof createStylexBunPlugin>[0]
> & {
  externalPackages?: ReadonlyArray<string>
}

const desktopRoot = process.cwd()
const tailwindCss = resolve(desktopRoot, "src/ui/styles.tailwind.css")
const stylexCss = resolve(desktopRoot, "src/ui/styles.stylex.css")
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

const stylexOptions: StylexBunOptions = {
  dev: false,
  runtimeInjection: false,
  useCSSLayers: true,
  bunDevCssOutput: stylexCss,
  externalPackages: ["@openagentsinc/ui", "@openagentsinc/autopilot-ui"],
}

const result = await Bun.build({
  entrypoints: ["src/ui/main.ts"],
  outdir: viewBundleOut,
  target: "browser",
  plugins: [createStylexBunPlugin(stylexOptions)],
})

if (!result.success) {
  console.error(result.logs.map(log => String(log)).join("\n"))
  process.exit(1)
}

await mkdir(dirname(outputCss), { recursive: true })

const [tailwindOutput, stylexOutput] = await Promise.all([
  readFile(tailwindCss, "utf8"),
  readFile(stylexCss, "utf8").catch(() => ""),
])

await writeFile(
  outputCss,
  `${tailwindOutput}\n\n/* StyleX generated component CSS */\n${stylexOutput}`,
)

await Promise.all([
  rm(tailwindCss, { force: true }),
  rm(stylexCss, { force: true }),
])
