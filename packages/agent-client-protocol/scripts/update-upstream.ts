import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import {
  SCHEMA_RELEASE,
  SDK_AUTHORITY,
  UPSTREAM_ASSETS,
  UPSTREAM_COMMIT,
  WIRE_VERSION,
} from "./source.ts";

const packageRoot = resolve(import.meta.dirname, "..");
const upstreamRoot = resolve(packageRoot, "upstream", SCHEMA_RELEASE);

await mkdir(upstreamRoot, { recursive: true });

await Promise.all(
  Object.entries(UPSTREAM_ASSETS).map(async ([name, authority]) => {
    const response = await fetch(authority.url, { redirect: "follow" });
    if (!response.ok) throw new Error(`${name}: upstream fetch failed with ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    const digest = createHash("sha256").update(bytes).digest("hex");
    if (digest !== authority.sha256)
      throw new Error(`${name}: sha256 ${digest} != ${authority.sha256}`);
    await writeFile(resolve(upstreamRoot, name), bytes);
  }),
);

const source = {
  protocol: "Agent Client Protocol",
  release: SCHEMA_RELEASE,
  wireVersion: WIRE_VERSION,
  upstreamCommit: UPSTREAM_COMMIT,
  releaseUrl: `https://github.com/agentclientprotocol/agent-client-protocol/releases/tag/${SCHEMA_RELEASE}`,
  assets: UPSTREAM_ASSETS,
  sdk: SDK_AUTHORITY,
};
await writeFile(resolve(upstreamRoot, "SOURCE.json"), `${JSON.stringify(source, null, 2)}\n`);

console.log(`Updated and verified ${SCHEMA_RELEASE} upstream authority.`);
