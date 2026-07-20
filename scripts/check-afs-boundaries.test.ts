import { describe, expect, test } from "vite-plus/test";

import { importSpecifiers, inspectAfsBoundaries } from "./check-afs-boundaries.ts";

describe("AFS package boundaries", () => {
  test("the AFS root packages have no boundary, subpath-export, or cycle violation", () => {
    expect(inspectAfsBoundaries()).toEqual([]);
  });

  test("importSpecifiers extracts static, re-export, and dynamic module specifiers", () => {
    const source = [
      `import { A } from "node:fs";`,
      `import type { B } from "@openagentsinc/agent-runtime-schema";`,
      `export * from "./local.js";`,
      `const c = await import("electron");`,
    ].join("\n");
    expect(importSpecifiers(source)).toEqual([
      "node:fs",
      "@openagentsinc/agent-runtime-schema",
      "./local.js",
      "electron",
    ]);
  });
});
