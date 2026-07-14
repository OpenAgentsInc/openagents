#!/usr/bin/env node
import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { basename, join, resolve } from "node:path"
import { build as vpPack } from "vite-plus/pack"

import { publicCliArtifacts, sourceEntriesFor } from "./public-cli-artifact-catalog.mjs"

const repositoryRoot = resolve(import.meta.dirname, "..")
const filterAt = process.argv.indexOf("--filter")
const filter = filterAt === -1 ? undefined : process.argv[filterAt + 1]
const stageAt = process.argv.indexOf("--stage-dir")
const stageRoot = stageAt === -1 ? undefined : resolve(process.argv[stageAt + 1])

const selected = publicCliArtifacts.filter((record) => filter === undefined || record.name === filter || record.root === filter)
if (selected.length === 0) throw new Error(`no public CLI artifact matches ${filter}`)

const outputForSource = (source) => `./dist/${source.replace(/^src\//, "").replace(/\.[cm]?[jt]sx?$/, ".mjs")}`
const typesForSource = (source) => outputForSource(source).replace(/\.mjs$/, ".d.mts")

for (const record of selected) {
  const packageRoot = join(repositoryRoot, record.root)
  const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"))
  if (manifest.name !== record.name) throw new Error(`${record.root}: catalog name drift`)
  const entries = sourceEntriesFor(record, manifest)
  await vpPack({
    cwd: packageRoot,
    root: packageRoot,
    entry: entries,
    outDir: "dist",
    clean: true,
    dts: record.eagerDts ? { eager: true } : true,
    format: "esm",
    platform: "node",
    target: "node24",
    sourcemap: true,
    deps: {
      alwaysBundle: [/^@openagentsinc\//],
      onlyBundle: false,
      dts: record.externalInternalDts
        ? { neverBundle: [/^@openagentsinc\//] }
        : { alwaysBundle: [/^@openagentsinc\//] },
    },
  })
  for (const target of Object.values(record.bin)) {
    if (!target.startsWith("dist/")) continue
    const path = join(packageRoot, target)
    if (!existsSync(path)) throw new Error(`${record.name}: missing built bin ${target}`)
    const body = readFileSync(path, "utf8").replace(/^#![^\n]*\n/, "")
    writeFileSync(path, `#!/usr/bin/env node\n${body}`)
    chmodSync(path, 0o755)
  }

  if (stageRoot === undefined) continue
  const destination = join(stageRoot, record.name.replace(/^@/, "").replaceAll("/", "__"))
  rmSync(destination, { recursive: true, force: true })
  mkdirSync(destination, { recursive: true })
  cpSync(join(packageRoot, "dist"), join(destination, "dist"), { recursive: true })
  for (const asset of record.assets ?? []) {
    const source = join(packageRoot, asset)
    if (existsSync(source)) cpSync(source, join(destination, asset), { recursive: true })
  }
  for (const asset of ["README.md", "LICENSE"]) {
    const source = join(packageRoot, asset)
    if (existsSync(source) && !existsSync(join(destination, asset))) cpSync(source, join(destination, asset))
  }

  const stagedExports = {}
  const builtSources = new Set(Object.values(entries))
  for (const [subpath, target] of Object.entries(manifest.exports ?? {})) {
    if (typeof target === "string" && /^\.\/src\/.*\.[cm]?[jt]sx?$/.test(target) && builtSources.has(target.replace(/^\.\//, ""))) {
      const source = target.replace(/^\.\//, "")
      stagedExports[subpath] = { types: typesForSource(source), import: outputForSource(source) }
    } else if (subpath === "./qa") {
      stagedExports[subpath] = { types: "./dist/byo.d.mts", import: "./dist/byo.mjs" }
    }
  }
  const dependencies = Object.fromEntries(Object.entries(manifest.dependencies ?? {}).filter(([name, version]) => !name.startsWith("@openagentsinc/") && !String(version).startsWith("workspace:")))
  for (const dependency of record.typeDependencies ?? []) {
    const dependencyManifest = JSON.parse(readFileSync(join(repositoryRoot, dependency.root, "package.json"), "utf8"))
    if (dependencyManifest.name !== dependency.name || dependencyManifest.private === true) {
      throw new Error(`${record.name}: invalid public type dependency ${dependency.name}`)
    }
    dependencies[dependency.name] = dependencyManifest.version
  }
  const staged = {
    name: manifest.name,
    version: manifest.version,
    description: manifest.description,
    type: "module",
    engines: { node: ">=24.10" },
    bin: record.bin,
    ...(Object.keys(stagedExports).length === 0 ? {} : { exports: stagedExports }),
    files: ["dist", ...(record.assets ?? []).filter((asset) => !["README.md", "LICENSE"].includes(asset))],
    dependencies,
    optionalDependencies: manifest.optionalDependencies,
    peerDependencies: manifest.peerDependencies,
    publishConfig: { access: "public" },
    license: manifest.license,
    repository: manifest.repository,
  }
  writeFileSync(join(destination, "package.json"), `${JSON.stringify(staged, null, 2)}\n`)
  console.log(`${record.name}: staged ${basename(destination)}`)
}
