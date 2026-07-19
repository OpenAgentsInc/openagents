import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, test } from "vite-plus/test";

import { buildInventory } from "../src/inventory.ts";
import {
  serializeSurfaceInventory,
  SURFACE_INVENTORY_PATH,
  validateSurfaceInventory,
} from "../src/schema.ts";
import { repositoryRoot } from "../src/workspace.ts";

const root = repositoryRoot();

describe("buildInventory against the real repository", () => {
  test("produces zero silent surfaces", () => {
    const document = buildInventory(root);
    const validation = validateSurfaceInventory(document);
    const silent = validation.issues.filter((i) => i.kind === "silent_surface");
    expect(silent).toEqual([]);
    expect(validation.ok).toBe(true);
  });

  test("every surface has an oracle ref or an explicit unverified reason", () => {
    const document = buildInventory(root);
    for (const surface of document.surfaces) {
      const bound = surface.oracles.length > 0;
      const tagged = surface.unverified !== undefined;
      expect(bound || tagged).toBe(true);
      expect(bound && tagged).toBe(false);
    }
  });

  test("is deterministic: two builds serialize identically", () => {
    const first = serializeSurfaceInventory(buildInventory(root));
    const second = serializeSurfaceInventory(buildInventory(root));
    expect(first).toBe(second);
  });

  test("enumerates the expected surface kinds", () => {
    const document = buildInventory(root);
    const kinds = new Set(document.surfaces.map((s) => s.kind));
    for (const kind of [
      "package",
      "app",
      "crate",
      "public-endpoint",
      "release-pipeline",
      "document",
    ]) {
      expect(kinds.has(kind as never)).toBe(true);
    }
  });
});

describe("committed inventory freshness guard", () => {
  test("the committed artifact matches a fresh regeneration (run generate:assure-repo if this fails)", () => {
    const path = join(root, SURFACE_INVENTORY_PATH);
    expect(existsSync(path)).toBe(true);
    const committed = readFileSync(path, "utf8");
    const expected = serializeSurfaceInventory(buildInventory(root));
    expect(committed).toBe(expected);
  });

  test("the committed artifact itself validates", () => {
    const path = join(root, SURFACE_INVENTORY_PATH);
    const committed = JSON.parse(readFileSync(path, "utf8"));
    const validation = validateSurfaceInventory(committed);
    expect(validation.ok).toBe(true);
  });
});
