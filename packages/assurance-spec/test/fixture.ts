import { cpSync, mkdirSync, mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

export const repoRoot = resolve(import.meta.dir, "../../..")

export const MVP_SPEC = "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md"
export const MVP_SUBJECT = "docs/mvp/openagents-codex-workroom-mvp.product-spec.md"

/** A disposable root containing the checked-in MVP AssuranceSpec pair. */
export const makeFixtureRoot = (): string => {
  const root = mkdtempSync(join(tmpdir(), "assurance-at1-"))
  mkdirSync(join(root, "docs/mvp"), { recursive: true })
  cpSync(join(repoRoot, MVP_SPEC), join(root, MVP_SPEC))
  cpSync(join(repoRoot, MVP_SUBJECT), join(root, MVP_SUBJECT))
  return root
}
