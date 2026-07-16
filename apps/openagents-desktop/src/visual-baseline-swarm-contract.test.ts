import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import {
  QA_DESKTOP_VISUAL_LANE,
  QA_DESKTOP_VISUAL_RECEIPT_SCHEMA,
} from "./visual-baseline-contract.ts";

const root = path.resolve(import.meta.dirname, "../../..");

describe("QA-1 Desktop visual lane contract", () => {
  test("publishes an executable root command and stable receipt identity", () => {
    const packageJson = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8")) as {
      scripts?: Record<string, string>;
    };
    const contract = JSON.parse(
      readFileSync(path.join(root, "docs/qa/swarm/desktop-visual-lane.json"), "utf8"),
    ) as {
      lane?: string;
      receiptSchema?: string;
      runFromRepositoryRoot?: string;
    };
    expect(packageJson.scripts?.["qa:swarm:desktop"]).toBe(
      "pnpm --dir apps/openagents-desktop run qa:visual",
    );
    expect(contract.runFromRepositoryRoot).toBe("pnpm run qa:swarm:desktop");
    expect(contract.receiptSchema).toBe(QA_DESKTOP_VISUAL_RECEIPT_SCHEMA);
    expect(contract.lane).toBe(QA_DESKTOP_VISUAL_LANE);
  });
});
