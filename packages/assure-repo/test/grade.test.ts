import { describe, expect, test } from "vite-plus/test";

import {
  buildInventory,
  coverageByArea,
  gradeSurface,
  programArea,
  type SurfaceRow,
} from "../src/index.ts";
import { repositoryRoot } from "../src/workspace.ts";

const row = (overrides: Partial<SurfaceRow>): SurfaceRow => ({
  id: "package:x",
  kind: "package",
  owningPath: "packages/x",
  title: "x",
  derivation: "derived",
  oracles: [],
  ...overrides,
});

describe("gradeSurface", () => {
  test("a surface with a test oracle is `designed`, never `observed`", () => {
    const obligation = gradeSurface(
      row({ oracles: [{ type: "test", ref: "packages/x (2 tracked test files)" }] }),
      {},
    );
    expect(obligation.state).toBe("designed");
  });

  test("a behavior-contract oracle is `designed`", () => {
    const obligation = gradeSurface(
      row({ oracles: [{ type: "behavior-contract", ref: "openagents_apps.x.v1" }] }),
      {},
    );
    expect(obligation.state).toBe("designed");
  });

  test("a product-spec-only oracle is `mapped`, not `designed`", () => {
    const obligation = gradeSurface(
      row({
        kind: "document",
        owningPath: "specs/x.product-spec.md",
        oracles: [{ type: "product-spec", ref: "pnpm run test:product-spec" }],
      }),
      {},
    );
    expect(obligation.state).toBe("mapped");
  });

  test("a no-oracle-authored gap is `inconclusive`", () => {
    const obligation = gradeSurface(
      row({ oracles: [], unverified: { reason: "no-oracle-authored", note: "gap" } }),
      {},
    );
    expect(obligation.state).toBe("inconclusive");
  });

  test("a config-only / reference-only surface is `out-of-scope`", () => {
    expect(gradeSurface(row({ unverified: { reason: "config-only", note: "" } }), {}).state).toBe(
      "out-of-scope",
    );
    expect(
      gradeSurface(row({ unverified: { reason: "reference-only", note: "" } }), {}).state,
    ).toBe("out-of-scope");
  });

  test("an explicit policy out-of-scope disposition wins", () => {
    const obligation = gradeSurface(row({ oracles: [{ type: "test", ref: "x" }] }), {
      "package:x": "retired lane",
    });
    expect(obligation.state).toBe("out-of-scope");
    expect(obligation.note).toBe("retired lane");
  });
});

describe("programArea", () => {
  test("classifies by top-level segment", () => {
    expect(programArea("packages/foo")).toBe("packages");
    expect(programArea("crates/oa-node")).toBe("crates");
    expect(programArea("apps/openagents.com/workers/api")).toBe("openagents.com/workers");
    expect(programArea("apps/pylon")).toBe("apps/pylon");
  });
});

describe("coverageByArea keeps facts independent", () => {
  test("never blends states and never fabricates observed/accepted at grade time", () => {
    const document = buildInventory(repositoryRoot());
    const report = coverageByArea(document.surfaces);
    for (const area of report) {
      const sum = Object.values(area.byState).reduce((a, b) => a + b, 0);
      expect(sum).toBe(area.total);
    }
    // Grading never sets observed or accepted.
    expect(
      document.surfaces.every(
        (s) => s.obligation?.state !== "observed" && s.obligation?.state !== "accepted",
      ),
    ).toBe(true);
  });

  test("every graded surface carries an obligation state", () => {
    const document = buildInventory(repositoryRoot());
    expect(document.surfaces.every((s) => s.obligation !== undefined)).toBe(true);
  });
});
