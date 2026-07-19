import { createHash } from "node:crypto"
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "@effect-native/core/effect"

export const ProductSpecWorkSkillName = "productspec-work" as const
export const ProductSpecWorkSkillVersion = "0.2.0" as const
export const ProductSpecWorkSkillSha256 = "610f2171cce162ce0da79d7ef17445277744f16d202dfd90872dff215497294b" as const
export const AssuranceSpecWorkSkillName = "assurancespec-work" as const
export const AssuranceSpecWorkSkillVersion = "0.1.0" as const
export const AssuranceSpecWorkSkillSha256 = "162a663150ceeadc939feaa4420e8981c81d5db225e4bf9405a7ddbc7ec53aa1" as const
export const BuiltinSkillManifestFilename = "manifest.json" as const

const Sha256 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/))
const RelativeSkillPath = Schema.String.check(
  Schema.isPattern(/^(?!\/)(?!.*(?:^|\/)\.\.(?:\/|$))[A-Za-z0-9._/-]+$/),
)

export const BuiltinSkillManifestSchema = Schema.Struct({
  schema: Schema.Literal("openagents.desktop.builtin_skill_manifest.v1"),
  compatibilitySetVersion: Schema.Literal(5),
  skills: Schema.Array(Schema.Struct({
    name: Schema.Literals([ProductSpecWorkSkillName, AssuranceSpecWorkSkillName]),
    version: Schema.Literals([ProductSpecWorkSkillVersion, AssuranceSpecWorkSkillVersion]),
    relativePath: RelativeSkillPath,
    sha256: Sha256,
    authority: Schema.Literal("proposal_only"),
    installationScope: Schema.Literal("app_owned_extra_root"),
    registrationSurface: Schema.Literal("codex_app_server_native"),
    ambientFallback: Schema.Literal(false),
    defaultCodexSessionAllowed: Schema.Literal(true),
    defaultCodexHomeMutationAllowed: Schema.Literal(false),
  })).check(Schema.isMinLength(2), Schema.isMaxLength(2)),
})
export type BuiltinSkillManifest = typeof BuiltinSkillManifestSchema.Type

export class BuiltinProductSpecSkillError extends Error {
  readonly _tag = "BuiltinProductSpecSkillError"
  override readonly name = "BuiltinProductSpecSkillError"

  constructor(
    readonly reason: "manifest_invalid" | "manifest_digest_mismatch" | "asset_digest_mismatch" | "asset_unavailable",
    message: string,
  ) {
    super(message)
  }
}

const digest = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex")

export type VerifiedProductSpecWorkSkill = Readonly<{
  manifest: BuiltinSkillManifest
  skillPath: string
  sha256: typeof ProductSpecWorkSkillSha256
}>

export type InstalledProductSpecWorkSkill = Readonly<{
  skillRoot: string
  skillPath: string
  sha256: typeof ProductSpecWorkSkillSha256
  reconciled: boolean
}>

/**
 * Verify the product-owned asset before it can be installed into a named
 * isolated Codex home. The expected digest is compiled into the signed host,
 * so changing the adjacent manifest and skill together cannot silently move
 * the compatibility set.
 */
export const verifyBuiltinProductSpecWorkSkill = (
  builtinSkillsRoot: string,
): VerifiedProductSpecWorkSkill => {
  let manifest: BuiltinSkillManifest
  try {
    manifest = Schema.decodeUnknownSync(BuiltinSkillManifestSchema)(
      JSON.parse(readFileSync(path.join(builtinSkillsRoot, BuiltinSkillManifestFilename), "utf8")),
    )
  } catch (error) {
    throw new BuiltinProductSpecSkillError(
      "manifest_invalid",
      error instanceof Error ? error.message : "built-in skill manifest is invalid",
    )
  }

  const entry = manifest.skills.find(candidate => candidate.name === ProductSpecWorkSkillName)
  if (entry === undefined || entry.version !== ProductSpecWorkSkillVersion || entry.sha256 !== ProductSpecWorkSkillSha256) {
    throw new BuiltinProductSpecSkillError(
      "manifest_digest_mismatch",
      "productspec-work manifest digest does not match the signed compatibility set",
    )
  }

  const skillPath = path.resolve(builtinSkillsRoot, entry.relativePath)
  const relative = path.relative(path.resolve(builtinSkillsRoot), skillPath)
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new BuiltinProductSpecSkillError("manifest_invalid", "built-in skill path escapes its resource root")
  }

  let bytes: Buffer
  try {
    bytes = readFileSync(skillPath)
  } catch {
    throw new BuiltinProductSpecSkillError("asset_unavailable", "productspec-work asset is unavailable")
  }
  if (digest(bytes) !== ProductSpecWorkSkillSha256) {
    throw new BuiltinProductSpecSkillError(
      "asset_digest_mismatch",
      "productspec-work asset digest does not match the signed compatibility set",
    )
  }
  return { manifest, skillPath, sha256: ProductSpecWorkSkillSha256 }
}

export type VerifiedAssuranceSpecWorkSkill = Readonly<{
  manifest: BuiltinSkillManifest
  skillPath: string
  sha256: typeof AssuranceSpecWorkSkillSha256
}>

/** Verify the AssuranceSpec working-method asset against the signed host pin. */
export const verifyBuiltinAssuranceSpecWorkSkill = (
  builtinSkillsRoot: string,
): VerifiedAssuranceSpecWorkSkill => {
  let manifest: BuiltinSkillManifest
  try {
    manifest = Schema.decodeUnknownSync(BuiltinSkillManifestSchema)(
      JSON.parse(readFileSync(path.join(builtinSkillsRoot, BuiltinSkillManifestFilename), "utf8")),
    )
  } catch (error) {
    throw new BuiltinProductSpecSkillError(
      "manifest_invalid",
      error instanceof Error ? error.message : "built-in skill manifest is invalid",
    )
  }

  const entry = manifest.skills.find(candidate => candidate.name === AssuranceSpecWorkSkillName)
  if (entry === undefined || entry.version !== AssuranceSpecWorkSkillVersion || entry.sha256 !== AssuranceSpecWorkSkillSha256) {
    throw new BuiltinProductSpecSkillError(
      "manifest_digest_mismatch",
      "assurancespec-work manifest digest does not match the signed compatibility set",
    )
  }
  const skillPath = path.resolve(builtinSkillsRoot, entry.relativePath)
  const relative = path.relative(path.resolve(builtinSkillsRoot), skillPath)
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new BuiltinProductSpecSkillError("manifest_invalid", "built-in skill path escapes its resource root")
  }
  let bytes: Buffer
  try {
    bytes = readFileSync(skillPath)
  } catch {
    throw new BuiltinProductSpecSkillError("asset_unavailable", "assurancespec-work asset is unavailable")
  }
  if (digest(bytes) !== AssuranceSpecWorkSkillSha256) {
    throw new BuiltinProductSpecSkillError(
      "asset_digest_mismatch",
      "assurancespec-work asset digest does not match the signed compatibility set",
    )
  }
  return { manifest, skillPath, sha256: AssuranceSpecWorkSkillSha256 }
}

/**
 * Materialize the signed asset inside one named, isolated Codex home. The
 * caller must provide the ambient/default home explicitly so this function can
 * fail closed rather than ever mutating the owner's normal Codex installation.
 */
export const installBuiltinProductSpecWorkSkill = (input: Readonly<{
  builtinSkillsRoot: string
  namedCodexHome: string
  defaultCodexHome: string
}>): InstalledProductSpecWorkSkill => {
  const namedHome = path.resolve(input.namedCodexHome)
  if (namedHome === path.resolve(input.defaultCodexHome)) {
    throw new BuiltinProductSpecSkillError(
      "asset_unavailable",
      "productspec-work refuses installation into the default Codex home",
    )
  }
  const verified = verifyBuiltinProductSpecWorkSkill(input.builtinSkillsRoot)
  const skillRoot = path.join(namedHome, "skills")
  const installRoot = path.join(skillRoot, ProductSpecWorkSkillName)
  const skillPath = path.join(installRoot, "SKILL.md")
  mkdirSync(installRoot, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(installRoot, 0o700)
  if (existsSync(skillPath) && digest(readFileSync(skillPath)) === verified.sha256) {
    return { skillRoot, skillPath, sha256: verified.sha256, reconciled: true }
  }
  const pending = `${skillPath}.${process.pid}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, readFileSync(verified.skillPath), { flag: "wx", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, skillPath)
    if (process.platform !== "win32") chmodSync(skillPath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw new BuiltinProductSpecSkillError(
      "asset_unavailable",
      error instanceof Error ? error.message : "productspec-work installation failed",
    )
  }
  if (digest(readFileSync(skillPath)) !== verified.sha256) {
    throw new BuiltinProductSpecSkillError("asset_digest_mismatch", "installed productspec-work digest mismatched")
  }
  return { skillRoot, skillPath, sha256: verified.sha256, reconciled: false }
}

export type InstalledAssuranceSpecWorkSkill = Readonly<{
  skillRoot: string
  skillPath: string
  sha256: typeof AssuranceSpecWorkSkillSha256
  reconciled: boolean
}>

/** Materialize assurancespec-work only inside one named isolated Codex home. */
export const installBuiltinAssuranceSpecWorkSkill = (input: Readonly<{
  builtinSkillsRoot: string
  namedCodexHome: string
  defaultCodexHome: string
}>): InstalledAssuranceSpecWorkSkill => {
  const namedHome = path.resolve(input.namedCodexHome)
  if (namedHome === path.resolve(input.defaultCodexHome)) {
    throw new BuiltinProductSpecSkillError(
      "asset_unavailable",
      "assurancespec-work refuses installation into the default Codex home",
    )
  }
  const verified = verifyBuiltinAssuranceSpecWorkSkill(input.builtinSkillsRoot)
  const skillRoot = path.join(namedHome, "skills")
  const installRoot = path.join(skillRoot, AssuranceSpecWorkSkillName)
  const skillPath = path.join(installRoot, "SKILL.md")
  mkdirSync(installRoot, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(installRoot, 0o700)
  if (existsSync(skillPath) && digest(readFileSync(skillPath)) === verified.sha256) {
    return { skillRoot, skillPath, sha256: verified.sha256, reconciled: true }
  }
  const pending = `${skillPath}.${process.pid}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, readFileSync(verified.skillPath), { flag: "wx", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, skillPath)
    if (process.platform !== "win32") chmodSync(skillPath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw new BuiltinProductSpecSkillError(
      "asset_unavailable",
      error instanceof Error ? error.message : "assurancespec-work installation failed",
    )
  }
  if (digest(readFileSync(skillPath)) !== verified.sha256) {
    throw new BuiltinProductSpecSkillError("asset_digest_mismatch", "installed assurancespec-work digest mismatched")
  }
  return { skillRoot, skillPath, sha256: verified.sha256, reconciled: false }
}
