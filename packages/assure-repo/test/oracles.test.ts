import { describe, expect, test } from "vite-plus/test";

import { codeSurfaceOracles, surfaceTestFiles } from "../src/index.ts";

const tracked = [
  "packages/x/src/index.ts",
  "packages/x/src/index.test.ts",
  "packages/y/src/runtime.node-suite.ts",
  "packages/y/src/runtime.ts",
  "packages/z/src/thing.ts",
  "packages/z/README.md",
];

describe("surfaceTestFiles", () => {
  test("recognizes *.test.ts", () => {
    expect(surfaceTestFiles("packages/x", tracked)).toEqual(["packages/x/src/index.test.ts"]);
  });

  test("recognizes the *.node-suite.ts / *.suite.ts test convention", () => {
    // runtime-platform and sqlite-runtime ship node --test suites named this way;
    // the detector must not report those packages as untested.
    expect(surfaceTestFiles("packages/y", tracked)).toEqual([
      "packages/y/src/runtime.node-suite.ts",
    ]);
  });

  test("finds no test files for a package with none", () => {
    expect(surfaceTestFiles("packages/z", tracked)).toEqual([]);
  });
});

describe("codeSurfaceOracles", () => {
  test("binds a test oracle when a node-suite is present and none otherwise", () => {
    expect(codeSurfaceOracles("packages/y", tracked, []).some((o) => o.type === "test")).toBe(true);
    expect(codeSurfaceOracles("packages/z", tracked, [])).toEqual([]);
  });
});
