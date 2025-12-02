import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { PartialToolArgsParser } from "./partialToolArgs.js";

describe("PartialToolArgsParser", () => {
  test("decodes when buffer becomes valid JSON", () => {
    const parser = new PartialToolArgsParser(S.Struct({ value: S.String }));

    const first = parser.append('{"value": "he');
    expect(first.decoded).toBeNull();
    expect(first.error).not.toBeNull();

    const second = parser.append("llo\"}");
    expect(second.error).toBeNull();
    expect(second.decoded).toEqual({ value: "hello" });
    expect(parser.value).toEqual({ value: "hello" });
  });

  test("keeps last good value when new chunk invalid", () => {
    const parser = new PartialToolArgsParser(S.Struct({ value: S.String }));
    parser.append('{"value": "ok"}');

    const result = parser.append(", broken");
    expect(result.decoded).toEqual({ value: "ok" });
    expect(result.error).not.toBeNull();
  });
});
