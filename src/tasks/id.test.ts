import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  generateHashId,
  generateShortId,
  generateRandomId,
  generateChildId,
  parseHierarchicalId,
  isChildOf,
  getParentId,
  canHaveChildren,
  findNextChildNumber,
  MAX_HIERARCHY_DEPTH,
} from "./id.js";

describe("generateHashId", () => {
  test("generates deterministic hash from content", async () => {
    const date = new Date("2025-12-02T10:00:00Z");
    const hash1 = await Effect.runPromise(
      generateHashId("oa", "Test task", "Description", date),
    );
    const hash2 = await Effect.runPromise(
      generateHashId("oa", "Test task", "Description", date),
    );
    expect(hash1).toBe(hash2);
    expect(hash1).toHaveLength(64); // SHA-256 hex
  });

  test("different content produces different hashes", async () => {
    const date = new Date("2025-12-02T10:00:00Z");
    const hash1 = await Effect.runPromise(
      generateHashId("oa", "Task A", "", date),
    );
    const hash2 = await Effect.runPromise(
      generateHashId("oa", "Task B", "", date),
    );
    expect(hash1).not.toBe(hash2);
  });
});

describe("generateShortId", () => {
  test("generates prefix-6char format by default", async () => {
    const id = await Effect.runPromise(generateShortId("oa", "Test task"));
    expect(id).toMatch(/^oa-[a-f0-9]{6}$/);
  });

  test("respects custom length", async () => {
    const id = await Effect.runPromise(
      generateShortId("oa", "Test task", "", new Date(), 8),
    );
    expect(id).toMatch(/^oa-[a-f0-9]{8}$/);
  });

  test("uses custom prefix", async () => {
    const id = await Effect.runPromise(
      generateShortId("openagents", "Test task"),
    );
    expect(id).toMatch(/^openagents-[a-f0-9]{6}$/);
  });
});

describe("generateRandomId", () => {
  test("generates random prefix-6char format", async () => {
    const id = await Effect.runPromise(generateRandomId("oa"));
    expect(id).toMatch(/^oa-[a-f0-9]{6}$/);
  });

  test("generates unique IDs", async () => {
    const id1 = await Effect.runPromise(generateRandomId("oa"));
    const id2 = await Effect.runPromise(generateRandomId("oa"));
    expect(id1).not.toBe(id2);
  });
});

describe("generateChildId", () => {
  test("generates parent.N format", () => {
    expect(generateChildId("oa-abc123", 1)).toBe("oa-abc123.1");
    expect(generateChildId("oa-abc123", 42)).toBe("oa-abc123.42");
  });

  test("supports nested children", () => {
    expect(generateChildId("oa-abc123.1", 2)).toBe("oa-abc123.1.2");
  });
});

describe("parseHierarchicalId", () => {
  test("parses root ID (no parent)", () => {
    const result = parseHierarchicalId("oa-abc123");
    expect(result).toEqual({
      rootId: "oa-abc123",
      parentId: null,
      depth: 0,
    });
  });

  test("parses first-level child", () => {
    const result = parseHierarchicalId("oa-abc123.1");
    expect(result).toEqual({
      rootId: "oa-abc123",
      parentId: "oa-abc123",
      depth: 1,
    });
  });

  test("parses second-level child", () => {
    const result = parseHierarchicalId("oa-abc123.1.2");
    expect(result).toEqual({
      rootId: "oa-abc123",
      parentId: "oa-abc123.1",
      depth: 2,
    });
  });

  test("parses third-level child", () => {
    const result = parseHierarchicalId("oa-abc123.1.2.3");
    expect(result).toEqual({
      rootId: "oa-abc123",
      parentId: "oa-abc123.1.2",
      depth: 3,
    });
  });
});

describe("isChildOf", () => {
  test("returns true for direct child", () => {
    expect(isChildOf("oa-abc123.1", "oa-abc123")).toBe(true);
  });

  test("returns true for nested child", () => {
    expect(isChildOf("oa-abc123.1.2", "oa-abc123")).toBe(true);
    expect(isChildOf("oa-abc123.1.2", "oa-abc123.1")).toBe(true);
  });

  test("returns false for non-child", () => {
    expect(isChildOf("oa-abc123", "oa-abc123")).toBe(false);
    expect(isChildOf("oa-def456.1", "oa-abc123")).toBe(false);
  });
});

describe("getParentId", () => {
  test("returns null for root ID", () => {
    expect(getParentId("oa-abc123")).toBeNull();
  });

  test("returns parent for child ID", () => {
    expect(getParentId("oa-abc123.1")).toBe("oa-abc123");
    expect(getParentId("oa-abc123.1.2")).toBe("oa-abc123.1");
  });
});

describe("canHaveChildren", () => {
  test("root can have children", () => {
    expect(canHaveChildren("oa-abc123")).toBe(true);
  });

  test("level 1 can have children", () => {
    expect(canHaveChildren("oa-abc123.1")).toBe(true);
  });

  test("level 2 can have children", () => {
    expect(canHaveChildren("oa-abc123.1.2")).toBe(true);
  });

  test("level 3 cannot have children (at max depth)", () => {
    expect(canHaveChildren("oa-abc123.1.2.3")).toBe(false);
  });
});

describe("findNextChildNumber", () => {
  test("returns 1 for no existing children", () => {
    expect(findNextChildNumber("oa-abc123", [])).toBe(1);
    expect(findNextChildNumber("oa-abc123", ["oa-def456.1"])).toBe(1);
  });

  test("returns next sequential number", () => {
    expect(
      findNextChildNumber("oa-abc123", ["oa-abc123.1", "oa-abc123.2"]),
    ).toBe(3);
  });

  test("handles gaps", () => {
    expect(
      findNextChildNumber("oa-abc123", ["oa-abc123.1", "oa-abc123.5"]),
    ).toBe(6);
  });

  test("ignores nested children when counting", () => {
    expect(
      findNextChildNumber("oa-abc123", [
        "oa-abc123.1",
        "oa-abc123.1.1",
        "oa-abc123.1.2",
      ]),
    ).toBe(2);
  });
});

describe("MAX_HIERARCHY_DEPTH", () => {
  test("is 3", () => {
    expect(MAX_HIERARCHY_DEPTH).toBe(3);
  });
});
