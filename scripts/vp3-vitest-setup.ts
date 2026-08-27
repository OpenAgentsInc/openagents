import { expect } from "vite-plus/test";
import { delimiter, resolve } from "node:path";

process.env.PATH = `${resolve(import.meta.dirname, "../node_modules/vite-plus/bin")}${delimiter}${process.env.PATH ?? ""}`;

expect.extend({
  toContainAllValues(received: unknown, expected: unknown) {
    const pass =
      Array.isArray(received) &&
      Array.isArray(expected) &&
      expected.every((value) => received.includes(value));
    return {
      pass,
      message: () =>
        `expected ${JSON.stringify(received)} ${pass ? "not " : ""}to contain all values ${JSON.stringify(expected)}`,
    };
  },
  toBeAbsent(received: unknown) {
    const pass = received === null || received === undefined;
    return {
      pass,
      message: () => `expected ${String(received)} ${pass ? "not " : ""}to be absent`,
    };
  },
  toBeFunction(received: unknown) {
    const pass = typeof received === "function";
    return {
      pass,
      message: () => `expected ${typeof received} ${pass ? "not " : ""}to be a function`,
    };
  },
  toBeString(received: unknown) {
    const pass = typeof received === "string";
    return {
      pass,
      message: () => `expected ${typeof received} ${pass ? "not " : ""}to be a string`,
    };
  },
  toStartWith(received: unknown, expected: unknown) {
    const pass =
      typeof received === "string" && typeof expected === "string" && received.startsWith(expected);
    return {
      pass,
      message: () =>
        `expected ${JSON.stringify(received)} ${pass ? "not " : ""}to start with ${JSON.stringify(expected)}`,
    };
  },
  toEndWith(received: unknown, expected: unknown) {
    const pass =
      typeof received === "string" && typeof expected === "string" && received.endsWith(expected);
    return {
      pass,
      message: () =>
        `expected ${JSON.stringify(received)} ${pass ? "not " : ""}to end with ${JSON.stringify(expected)}`,
    };
  },
});
