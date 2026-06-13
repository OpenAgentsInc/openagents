import { stat } from "node:fs/promises"
import { join } from "node:path"

export type RequiredArtifactSpec = {
  label: string
  relativePath: string
}

export async function checkRequiredArtifacts(
  cwd: string,
  specs: RequiredArtifactSpec[],
): Promise<{ satisfied: boolean; missingRefs: string[] }> {
  const missingRefs: string[] = []

  for (const spec of specs) {
    const segments = spec.relativePath.split("/")
    const isTraversal = spec.relativePath.startsWith("/") || segments.includes("..")
    if (isTraversal) {
      missingRefs.push(`artifact.required.${spec.label}`)
      continue
    }
    try {
      await stat(join(cwd, spec.relativePath))
    } catch {
      missingRefs.push(`artifact.required.${spec.label}`)
    }
  }

  return { satisfied: missingRefs.length === 0, missingRefs }
}
