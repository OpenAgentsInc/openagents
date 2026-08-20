import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  REPOSITORY_CONTRACT_NAME,
  REPOSITORY_CONTRACT_SHA256,
  REPOSITORY_CONTRACT_VERSION,
} from "../src/api-contract.js";

describe("Phoenix repository API contract", () => {
  it("pins the versioned artifact by SHA-256", async () => {
    const path = fileURLToPath(new URL("../contracts/repositories-v1.json", import.meta.url));
    const bytes = await readFile(path);
    const digest = createHash("sha256").update(bytes).digest("hex");
    const contract = JSON.parse(bytes.toString("utf8")) as {
      readonly contract: string;
      readonly version: number;
    };

    expect(digest).toBe(REPOSITORY_CONTRACT_SHA256);
    expect(contract.contract).toBe(REPOSITORY_CONTRACT_NAME);
    expect(contract.version).toBe(REPOSITORY_CONTRACT_VERSION);
  });
});
