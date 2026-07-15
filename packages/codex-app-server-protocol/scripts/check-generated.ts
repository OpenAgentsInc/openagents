import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const expected = {
  "current-source": {
    ref: "1bbdb32789e1f79932df44941236ea3658f6e965",
    counts: [126, 1, 11, 72],
  },
  "bundled-0.144.1": {
    ref: "44918ea10c0f99151c6710411b4322c2f5c96bea",
    counts: [125, 1, 11, 69],
  },
} as const;

for (const [lane, contract] of Object.entries(expected)) {
  const generated = resolve(import.meta.dirname, "..", "src", "_generated", lane);
  const schema = readFileSync(resolve(generated, "schema.gen.ts"), "utf8");
  const meta = readFileSync(resolve(generated, "meta.gen.ts"), "utf8");
  const manifest = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "..", "manifests", `${lane}.json`), "utf8"),
  ) as {
    generatedSchemaSha256: string;
    members: Array<Record<string, unknown>>;
  };
  if (!schema.includes(`Upstream protocol ref: ${contract.ref}`)) {
    throw new Error(`${lane}: generated ref does not match ${contract.ref}`);
  }
  const digest = createHash("sha256").update(schema).digest("hex");
  if (digest !== manifest.generatedSchemaSha256) {
    throw new Error(`${lane}: schema changed without a reviewed manifest update`);
  }
  const constantNames = [
    "CLIENT_REQUEST_METHODS",
    "CLIENT_NOTIFICATION_METHODS",
    "SERVER_REQUEST_METHODS",
    "SERVER_NOTIFICATION_METHODS",
  ];
  const observed = constantNames.map((name) => {
    const block = new RegExp(`export const ${name} = \\{([\\s\\S]*?)\\n\\} as const;`).exec(
      meta,
    )?.[1];
    if (block === undefined) throw new Error(`${lane}: missing ${name}`);
    return [...block.matchAll(/^\s+"[^"]+":/gmu)].length;
  });
  if (JSON.stringify(observed) !== JSON.stringify(contract.counts)) {
    throw new Error(
      `${lane}: protocol count drift ${observed.join("/")} != ${contract.counts.join("/")}`,
    );
  }
  const keys = manifest.members.map((member) => `${member.direction}:${member.method}`);
  if (new Set(keys).size !== keys.length) throw new Error(`${lane}: duplicate manifest member`);
  for (const member of manifest.members) {
    for (const field of [
      "direction",
      "stability",
      "generation",
      "paramsSchema",
      "decodeState",
      "handlerState",
      "nativeProjection",
      "productSurface",
      "authorityClass",
      "fixture",
      "realBinaryProof",
    ]) {
      if (!(field in member)) throw new Error(`${lane}: ${member.method} lacks ${field}`);
    }
  }
}

console.log("Codex app-server generated contract is reviewed and drift-free.");
