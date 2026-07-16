import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  SCHEMA_RELEASE,
  SDK_AUTHORITY,
  UPSTREAM_ASSETS,
  UPSTREAM_COMMIT,
  WIRE_VERSION,
} from "./source.ts";

const packageRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(packageRoot, "upstream", SCHEMA_RELEASE);
const digest = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");

for (const [name, authority] of Object.entries(UPSTREAM_ASSETS)) {
  const observed = digest(readFileSync(resolve(upstreamRoot, name)));
  if (observed !== authority.sha256)
    throw new Error(`${name}: sha256 ${observed} != ${authority.sha256}`);
}

const sdkSchemaPath = fileURLToPath(
  import.meta.resolve("@agentclientprotocol/sdk/schema/schema.json"),
);
const sdkSchemaDigest = digest(readFileSync(sdkSchemaPath));
if (sdkSchemaDigest !== SDK_AUTHORITY.schemaSha256) {
  throw new Error(
    `SDK ${SDK_AUTHORITY.version} schema ${sdkSchemaDigest} is not the pinned unstable artifact`,
  );
}
const sdkPackage = JSON.parse(
  readFileSync(resolve(sdkSchemaPath, "..", "..", "package.json"), "utf8"),
) as { name: string; version: string };
if (sdkPackage.name !== SDK_AUTHORITY.package || sdkPackage.version !== SDK_AUTHORITY.version) {
  throw new Error(`SDK package identity drift: ${sdkPackage.name}@${sdkPackage.version}`);
}

const source = JSON.parse(readFileSync(resolve(upstreamRoot, "SOURCE.json"), "utf8")) as {
  protocol: string;
  release: string;
  wireVersion: number;
  upstreamCommit: string;
  releaseUrl: string;
  assets: unknown;
  sdk: unknown;
};
const expectedSource = {
  protocol: "Agent Client Protocol",
  release: SCHEMA_RELEASE,
  wireVersion: WIRE_VERSION,
  upstreamCommit: UPSTREAM_COMMIT,
  releaseUrl: `https://github.com/agentclientprotocol/agent-client-protocol/releases/tag/${SCHEMA_RELEASE}`,
  assets: UPSTREAM_ASSETS,
  sdk: SDK_AUTHORITY,
};
if (JSON.stringify(source) !== JSON.stringify(expectedSource))
  throw new Error("SOURCE.json authority drift");

const temporary = mkdtempSync(resolve(tmpdir(), "openagents-acp-generated-"));
const generatedFiles = [
  "src/generated/stable-types.ts",
  "src/generated/unstable-types.ts",
  "src/generated/methods.ts",
  "src/generated/definitions.ts",
  "manifests/stable.json",
  "manifests/unstable.json",
] as const;

try {
  execFileSync(
    process.execPath,
    [resolve(import.meta.dirname, "generate.ts"), "--output-root", temporary],
    { stdio: "pipe" },
  );
  for (const file of generatedFiles) {
    const committed = readFileSync(resolve(packageRoot, file));
    const regenerated = readFileSync(resolve(temporary, file));
    if (!committed.equals(regenerated))
      throw new Error(
        `${file}: generated drift; run pnpm --dir packages/agent-client-protocol generate`,
      );
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}

console.log("Agent Client Protocol authority is pinned, SDK-separated, and generated drift-free.");
