// JSON-action parser/validator tests (fakes, no network): the protocol the
// Khala driver relies on must parse strict JSON, extract from fences/prose, and
// FAIL HONESTLY on unparseable/invalid output (never a fabricated default).

import { describe, expect, test } from "bun:test";
import { extractFirstJsonObject, KhalaActionParseError, parseKhalaAction } from "./khala-action";

describe("extractFirstJsonObject", () => {
  test("extracts a bare object", () => {
    expect(extractFirstJsonObject('{"action":"done"}')).toBe('{"action":"done"}');
  });
  test("extracts the first balanced object from prose + fence", () => {
    const text = 'Here is my action:\n```json\n{"action":"navigate","url":"/login"}\n```\ntrailing';
    expect(extractFirstJsonObject(text)).toBe('{"action":"navigate","url":"/login"}');
  });
  test("respects nested braces and strings with braces", () => {
    const text = '{"action":"assert","label":"a {b}","check":{"kind":"url-includes","value":"/x"}}';
    expect(extractFirstJsonObject(text)).toBe(text);
  });
  test("returns undefined when no object is present", () => {
    expect(extractFirstJsonObject("no json here")).toBeUndefined();
  });
});

describe("parseKhalaAction (valid actions)", () => {
  test("navigate", () => {
    expect(parseKhalaAction('{"action":"navigate","url":"/login"}')).toEqual({ action: "navigate", url: "/login" });
  });
  test("click", () => {
    expect(parseKhalaAction('{"action":"click","selector":"button.submit"}')).toEqual({
      action: "click",
      selector: "button.submit",
    });
  });
  test("type carries text", () => {
    expect(parseKhalaAction('{"action":"type","selector":"#email","text":"a@b.c"}')).toEqual({
      action: "type",
      selector: "#email",
      text: "a@b.c",
    });
  });
  test("readText with optional selector", () => {
    expect(parseKhalaAction('{"action":"readText"}')).toEqual({ action: "readText" });
  });
  test("waitFor with a condition", () => {
    expect(
      parseKhalaAction('{"action":"waitFor","condition":{"kind":"text-visible","value":"Log in"},"timeoutMs":5000}'),
    ).toEqual({ action: "waitFor", condition: { kind: "text-visible", value: "Log in" }, timeoutMs: 5000 });
  });
  test("assert with a check", () => {
    expect(parseKhalaAction('{"action":"assert","label":"stays at /login","check":{"kind":"url-includes","value":"/login"}}')).toEqual(
      { action: "assert", label: "stays at /login", check: { kind: "url-includes", value: "/login" } },
    );
  });
  test("terminal_run with args", () => {
    expect(parseKhalaAction('{"action":"terminal_run","command":"echo","args":["hi"]}')).toEqual({
      action: "terminal_run",
      command: "echo",
      args: ["hi"],
    });
  });
  test("done with a verdict", () => {
    expect(parseKhalaAction('{"action":"done","verdict":"pass","summary":"ok"}')).toEqual({
      action: "done",
      verdict: "pass",
      summary: "ok",
    });
  });
  test("fail with a reason", () => {
    expect(parseKhalaAction('{"action":"fail","reason":"blocked"}')).toEqual({ action: "fail", reason: "blocked" });
  });
  test("extracts from a markdown fence", () => {
    expect(parseKhalaAction('```json\n{"action":"done","verdict":"pass"}\n```')).toEqual({
      action: "done",
      verdict: "pass",
    });
  });
});

describe("parseKhalaAction (honest failures)", () => {
  test("throws on non-JSON", () => {
    expect(() => parseKhalaAction("I will now navigate to /login.")).toThrow(KhalaActionParseError);
  });
  test("throws on an unknown action verb", () => {
    expect(() => parseKhalaAction('{"action":"frobnicate"}')).toThrow(KhalaActionParseError);
  });
  test("throws on a missing required field", () => {
    expect(() => parseKhalaAction('{"action":"navigate"}')).toThrow(KhalaActionParseError);
  });
  test("throws on an invalid done verdict", () => {
    expect(() => parseKhalaAction('{"action":"done","verdict":"maybe"}')).toThrow(KhalaActionParseError);
  });
  test("throws on an invalid waitFor condition kind", () => {
    expect(() => parseKhalaAction('{"action":"waitFor","condition":{"kind":"sleep","value":"5"}}')).toThrow(
      KhalaActionParseError,
    );
  });
  test("the error carries a truncated raw for debugging (no fabricated success)", () => {
    try {
      parseKhalaAction("garbage");
      throw new Error("should have thrown");
    } catch (error) {
      expect(error).toBeInstanceOf(KhalaActionParseError);
      expect((error as KhalaActionParseError).raw).toContain("garbage");
    }
  });
});
