import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { renderWebDesignTokens } from "../src/web.ts";

const output = fileURLToPath(new URL("../src/web.generated.css", import.meta.url));
const generated = renderWebDesignTokens();

if (process.argv.includes("--check")) {
  if (!existsSync(output)) {
    throw new Error("packages/design-tokens/src/web.generated.css is missing; run pnpm run generate");
  }
  const current = readFileSync(output, "utf8");
  if (current !== generated) {
    throw new Error("packages/design-tokens/src/web.generated.css is stale; run pnpm run generate");
  }
  console.log("design-token web projection is current");
} else {
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, generated);
  console.log(`wrote ${output}`);
}
