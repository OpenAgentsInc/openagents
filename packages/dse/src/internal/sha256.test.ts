import { describe, expect, test } from "vite-plus/test";

import { sha256, sha256Hex } from "./sha256.js";

describe("sha256", () => {
  test("matches the FIPS 180-4 known vectors", () => {
    expect(sha256Hex("")).toBe("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    expect(sha256Hex("abc")).toBe(
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
    expect(sha256Hex("The quick brown fox jumps over the lazy dog")).toBe(
      "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
    );
  });

  test("hashes a multi-block message correctly", () => {
    expect(sha256Hex("a".repeat(1000))).toBe(
      "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3",
    );
  });

  test("is a pure function of the bytes", () => {
    const bytes = new TextEncoder().encode("determinism");
    expect(sha256(bytes)).toBe(sha256(bytes));
  });
});
