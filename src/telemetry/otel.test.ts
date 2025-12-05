import { describe, expect, it } from "bun:test";
import { endSpan, recordTokenUsage, recordToolCall, recordVerification, startSpan } from "./otel.js";

describe("telemetry/otel", () => {
  it("creates and ends spans safely when no OTEL exporter is configured", () => {
    const span = startSpan("test.span", { foo: "bar" });
    endSpan(span);
    expect(span).toBeDefined();
  });

  it("records counters without throwing", () => {
    expect(() => recordToolCall("read", true)).not.toThrow();
    expect(() => recordTokenUsage({ model: "test-model", promptTokens: 10, completionTokens: 5 })).not.toThrow();
    expect(() => recordVerification("typecheck", true)).not.toThrow();
  });
});
