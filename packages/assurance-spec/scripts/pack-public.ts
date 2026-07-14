#!/usr/bin/env node
import { cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { resolve } from "node:path"
import { createHash } from "node:crypto"
import { spawnSync } from "node:child_process"

type PackageJson = Record<string, unknown> & {
  name: string
  version: string
  dependencies?: Record<string, string>
  devDependencies?: Record<string, string>
  files?: string[]
}

export type PublicTarballReceipt = Readonly<{
  distribution_receipt_format_version: "0.1"
  packages: ReadonlyArray<Readonly<{
    name: string
    version: string
    filename: string
    sha256: `sha256:${string}`
  }>>
  publish_order: ReadonlyArray<string>
  npm_publication: "owner_authentication_required"
  github_hosted_ci: false
}>

const sha256File = (path: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`

const readJson = <A>(path: string): A => JSON.parse(readFileSync(path, "utf8")) as A

const copyPackageFiles = (source: string, stage: string, packageJson: PackageJson): void => {
  mkdirSync(stage, { recursive: true })
  for (const entry of packageJson.files ?? []) {
    cpSync(resolve(source, entry), resolve(stage, entry), { recursive: true })
  }
}

const pack = (stage: string, out: string): string => {
  const result = spawnSync("npm", ["pack", "--json", "--pack-destination", out], {
    cwd: stage,
    encoding: "utf8",
  })
  if (result.status !== 0) throw new Error(`npm_pack_failed:${result.stderr.trim()}`)
  const report = JSON.parse(result.stdout) as Array<{ filename: string }>
  const filename = report[0]?.filename
  if (filename === undefined) throw new Error("npm_pack_missing_filename")
  return filename
}

export const buildPublicTarballs = (
  repositoryRoot: string,
  outputDirectory: string,
): PublicTarballReceipt => {
  const workspace = readFileSync(resolve(repositoryRoot, "pnpm-workspace.yaml"), "utf8")
  const effectVersion = workspace.match(/^  effect:\s*["']?([^"'\n]+)["']?$/m)?.[1]
  if (effectVersion === undefined) throw new Error("missing_effect_catalog_version")
  mkdirSync(outputDirectory, { recursive: true })
  const staging = mkdtempSync(resolve(tmpdir(), "openagents-assurance-pack-"))

  const productSource = resolve(repositoryRoot, "packages/product-spec")
  const productPackage = readJson<PackageJson>(resolve(productSource, "package.json"))
  const productStage = resolve(staging, "product-spec")
  copyPackageFiles(productSource, productStage, productPackage)
  writeFileSync(resolve(productStage, "package.json"), `${JSON.stringify({
    ...productPackage,
    dependencies: { effect: effectVersion },
    devDependencies: undefined,
    scripts: undefined,
  }, null, 2)}\n`)
  const productFilename = pack(productStage, outputDirectory)

  const assuranceSource = resolve(repositoryRoot, "packages/assurance-spec")
  const assurancePackage = readJson<PackageJson>(resolve(assuranceSource, "package.json"))
  const assuranceStage = resolve(staging, "assurance-spec")
  copyPackageFiles(assuranceSource, assuranceStage, assurancePackage)
  writeFileSync(resolve(assuranceStage, "package.json"), `${JSON.stringify({
    ...assurancePackage,
    dependencies: {
      "@openagentsinc/product-spec": productPackage.version,
      effect: effectVersion,
    },
    devDependencies: undefined,
    scripts: undefined,
  }, null, 2)}\n`)
  const assuranceFilename = pack(assuranceStage, outputDirectory)

  const receipt: PublicTarballReceipt = {
    distribution_receipt_format_version: "0.1",
    packages: [
      {
        name: productPackage.name,
        version: productPackage.version,
        filename: productFilename,
        sha256: sha256File(resolve(outputDirectory, productFilename)),
      },
      {
        name: assurancePackage.name,
        version: assurancePackage.version,
        filename: assuranceFilename,
        sha256: sha256File(resolve(outputDirectory, assuranceFilename)),
      },
    ],
    publish_order: [productPackage.name, assurancePackage.name],
    npm_publication: "owner_authentication_required",
    github_hosted_ci: false,
  }
  writeFileSync(resolve(outputDirectory, "distribution-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`)
  return receipt
}

if (import.meta.main) {
  const outputFlag = process.argv.indexOf("--out")
  const out = outputFlag === -1 ? undefined : process.argv[outputFlag + 1]
  if (out === undefined) {
    console.error("usage: node --import tsx scripts/pack-public.ts --out <directory>")
    process.exit(2)
  }
  const root = resolve(import.meta.dirname, "../../..")
  const receipt = buildPublicTarballs(root, resolve(out))
  console.log(JSON.stringify(receipt, null, 2))
}
