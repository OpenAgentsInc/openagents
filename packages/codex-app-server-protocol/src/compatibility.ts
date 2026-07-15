export const bundledCodexVersion = "0.144.1" as const;
export const bundledCodexTarget = "aarch64-apple-darwin" as const;
export const bundledCodexExecutableSha256 =
  "29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a" as const;

export type CodexBinaryCompatibility =
  | { readonly _tag: "Compatible"; readonly manifest: "bundled-0.144.1" }
  | {
      readonly _tag: "Incompatible";
      readonly reason:
        | "malformed-version"
        | "unsupported-target"
        | "unverified-hash"
        | "version-mismatch";
    };

export function evaluateCodexBinaryCompatibility(input: {
  readonly version: string;
  readonly target: string;
  readonly sha256: string;
}): CodexBinaryCompatibility {
  if (!/^\d+\.\d+\.\d+$/.test(input.version)) {
    return { _tag: "Incompatible", reason: "malformed-version" };
  }
  if (input.version !== bundledCodexVersion) {
    return { _tag: "Incompatible", reason: "version-mismatch" };
  }
  if (input.target !== bundledCodexTarget) {
    return { _tag: "Incompatible", reason: "unsupported-target" };
  }
  if (input.sha256 !== bundledCodexExecutableSha256) {
    return { _tag: "Incompatible", reason: "unverified-hash" };
  }
  return { _tag: "Compatible", manifest: "bundled-0.144.1" };
}
