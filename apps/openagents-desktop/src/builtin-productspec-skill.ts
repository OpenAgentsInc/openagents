import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "@effect-native/core/effect"

export const ProductSpecWorkSkillName = "productspec-work" as const
export const ProductSpecWorkSkillVersion = "0.1.0" as const
export const ProductSpecWorkSkillSha256 = "3858748c8d3ef533f6da3e0568788b93807ef63eb484771edebd7ff67c63e10e" as const
export const BuiltinSkillManifestFilename = "manifest.json" as const

const Sha256 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/))
const RelativeSkillPath = Schema.String.check(
  Schema.isPattern(/^(?!\/)(?!.*(?:^|\/)\.\.(?:\/|$))[A-Za-z0-9._/-]+$/),
)

export const BuiltinSkillManifestSchema = Schema.Struct({
  schema: Schema.Literal("openagents.desktop.builtin_skill_manifest.v1"),
  compatibilitySetVersion: Schema.Literal(1),
  skills: Schema.Array(Schema.Struct({
    name: Schema.Literal(ProductSpecWorkSkillName),
    version: Schema.Literal(ProductSpecWorkSkillVersion),
    relativePath: RelativeSkillPath,
    sha256: Sha256,
    authority: Schema.Literal("proposal_only"),
    installationScope: Schema.Literal("named_isolated_codex_home"),
    registrationSurface: Schema.Literal("codex_app_server_native"),
    ambientFallback: Schema.Literal(false),
    defaultCodexHomeAllowed: Schema.Literal(false),
  })).check(Schema.isMinLength(1), Schema.isMaxLength(1)),
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

  const entry = manifest.skills[0]!
  if (entry.sha256 !== ProductSpecWorkSkillSha256) {
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
