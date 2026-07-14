import { canonicalJson } from "./serializer.ts"
import { sha256Digest } from "./tooling.ts"

export const canonicalArtifact = <A>(value: A): Readonly<{
  value: A
  bytes: string
  digest: `sha256:${string}`
}> => {
  const bytes = canonicalJson(value)
  return { value, bytes, digest: sha256Digest(bytes) as `sha256:${string}` }
}

export const withoutKey = <A extends Record<string, unknown>, K extends keyof A>(
  value: A,
  key: K,
): Omit<A, K> => {
  const { [key]: _removed, ...rest } = value
  return rest
}
