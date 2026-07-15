import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

type Direction =
  | "client-notification"
  | "client-request"
  | "server-notification"
  | "server-request";

const roots = {
  current: {
    stable: process.env.CODEX_CURRENT_STABLE_TYPES,
    full: process.env.CODEX_CURRENT_FULL_TYPES,
  },
  bundled: {
    stable: process.env.CODEX_BUNDLED_STABLE_TYPES,
    full: process.env.CODEX_BUNDLED_FULL_TYPES,
  },
};

for (const [lane, paths] of Object.entries(roots)) {
  if (!paths.stable || !paths.full) throw new Error(`Missing TypeScript roots for ${lane}`);
}

const files: ReadonlyArray<readonly [string, Direction]> = [
  ["ClientRequest.ts", "client-request"],
  ["ClientNotification.ts", "client-notification"],
  ["ServerRequest.ts", "server-request"],
  ["ServerNotification.ts", "server-notification"],
];

function methods(root: string) {
  const result = new Map<Direction, Array<string>>();
  for (const [file, direction] of files) {
    const source = readFileSync(resolve(root, file), "utf8");
    const values = [...source.matchAll(/"method"\s*:\s*"([^"]+)"/g)].map((match) => match[1]!);
    result.set(direction, [...new Set(values)].toSorted());
  }
  return result;
}

function sha256(path: string) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function writeLane(
  file: string,
  identity: Record<string, unknown>,
  stableRoot: string,
  fullRoot: string,
  generatedLane: string,
) {
  const stable = methods(stableRoot);
  const full = methods(fullRoot);
  const members = files.flatMap(([, direction]) => {
    const stableSet = new Set(stable.get(direction));
    return (full.get(direction) ?? []).map((method) => ({
      method,
      direction,
      stability:
        direction === "client-request" && !stableSet.has(method)
          ? "experimental-gated"
          : "stable-or-runtime-declared",
      generation: ["getConversationSummary", "gitDiffToRemote", "getAuthStatus"].includes(method)
        ? "deprecated-compatibility"
        : direction === "server-notification" &&
            ["rawResponseItem/completed", "rawResponse/completed"].includes(method)
          ? "runtime-compatibility"
          : "upstream-generated",
      paramsSchema: `${direction}:${method}:params`,
      resultSchema: direction.endsWith("request") ? `${direction}:${method}:result` : null,
      errorSchema: direction.endsWith("request") ? "JSONRPCError" : null,
      decodeState: "generated",
      handlerState: "not-admitted",
      nativeProjection: "pending-cap-02",
      productSurface: "none",
      authorityClass: "owner-local",
      fixture: `${direction}:${method}`,
      realBinaryProof:
        generatedLane === "bundled-0.144.1" ? "inventory-generated" : "source-generated",
    }));
  });
  const counts = Object.fromEntries(
    files.map(([, direction]) => [direction, full.get(direction)?.length ?? 0]),
  );
  const stableCounts = Object.fromEntries(
    files.map(([, direction]) => [direction, stable.get(direction)?.length ?? 0]),
  );
  const schemaPath = resolve(
    import.meta.dirname,
    "..",
    "src",
    "_generated",
    generatedLane,
    "schema.gen.ts",
  );
  const manifest = {
    schemaVersion: 1,
    identity,
    experimentalApiDefault: false,
    counts,
    stableCounts,
    requestPartition: {
      generatedStable: 87,
      deprecatedCompatibility: 3,
      experimentalGated: counts["client-request"] - 90,
    },
    generatedSchemaSha256: sha256(schemaPath),
    members,
  };
  writeFileSync(
    resolve(import.meta.dirname, "..", "manifests", file),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

writeLane(
  "current-source.json",
  { kind: "git-source", commit: "1bbdb32789e1f79932df44941236ea3658f6e965" },
  roots.current.stable!,
  roots.current.full!,
  "current-source",
);
writeLane(
  "bundled-0.144.1.json",
  {
    kind: "desktop-binary",
    version: "0.144.1",
    sourceCommit: "44918ea10c0f99151c6710411b4322c2f5c96bea",
    target: "aarch64-apple-darwin",
    executableSha256: "29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a",
  },
  roots.bundled.stable!,
  roots.bundled.full!,
  "bundled-0.144.1",
);
