import { test, expect } from "bun:test";

import { canonicalJson } from "../src/internal/canonicalJson.js";

test("canonicalJson sorts object keys", () => {
  expect(canonicalJson({ b: 1, a: 2 })).toBe('{"a":2,"b":1}');
});

test("canonicalJson omits undefined object fields", () => {
  expect(canonicalJson({ a: 1, b: undefined })).toBe('{"a":1}');
});

test("canonicalJson turns non-finite numbers into null", () => {
  expect(canonicalJson({ a: Infinity, b: NaN })).toBe('{"a":null,"b":null}');
});

test("canonicalJson rejects top-level undefined", () => {
  expect(() => canonicalJson(undefined)).toThrow();
});

