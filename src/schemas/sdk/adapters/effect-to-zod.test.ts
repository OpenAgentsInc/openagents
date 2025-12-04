import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { effectSchemaToZod } from "./effect-to-zod.js";

describe("effect-to-zod", () => {
  test("converts struct with optional fields", () => {
    const schema = S.Struct({
      name: S.String,
      age: S.optional(S.Number),
    });

    const zodSchema = effectSchemaToZod(schema);
    const parsed = zodSchema.parse({ name: "Ada" });
    expect(parsed).toEqual({ name: "Ada" });
    expect(() => zodSchema.parse({ name: 123 })).toThrow();
  });

  test("converts unions and literals", () => {
    const schema = S.Union(S.Literal("a"), S.Literal("b"));
    const zodSchema = effectSchemaToZod(schema);
    expect(zodSchema.parse("a")).toBe("a");
    expect(() => zodSchema.parse("c")).toThrow();
  });

  test("converts arrays and tuples", () => {
    const arraySchema = effectSchemaToZod(S.Array(S.Number));
    expect(arraySchema.parse([1, 2, 3])).toEqual([1, 2, 3]);

    const tupleSchema = effectSchemaToZod(S.Tuple(S.String, S.Number));
    expect(tupleSchema.parse(["x", 2])).toEqual(["x", 2]);
    expect(() => tupleSchema.parse(["x"])).toThrow();
  });

  test("converts records/index signatures", () => {
    const recordSchema = effectSchemaToZod(S.Record({ key: S.String, value: S.Number }));
    const parsed = recordSchema.parse({ one: 1, two: 2 });
    expect(parsed).toEqual({ one: 1, two: 2 });
  });

  test("supports template literals as strings", () => {
    const schema = S.TemplateLiteral(S.Literal("id-"), S.Literal("123"));
    const zodSchema = effectSchemaToZod(schema);
    expect(zodSchema.parse("id-123")).toBe("id-123");
    expect(zodSchema.parse("wrong")).toBe("wrong");
  });
});
