import { describe, expect, it } from "vitest";
import {
  INTERNAL_KEY_HEADER,
  SERVICE_TOKEN_HEADER,
  USER_ID_HEADER,
  buildInternalHeaders,
  buildServiceTokenHeader,
  roundUsd,
} from "@/lib/openclawApi";

describe("roundUsd", () => {
  it("rounds to two decimals", () => {
    expect(roundUsd(1.005)).toBe(1.01);
    expect(roundUsd(12.345)).toBe(12.35);
    expect(roundUsd(12.344)).toBe(12.34);
  });
});

describe("buildInternalHeaders", () => {
  it("formats internal auth headers", () => {
    const headers = buildInternalHeaders("secret", "user_123");
    expect(headers[INTERNAL_KEY_HEADER]).toBe("secret");
    expect(headers[USER_ID_HEADER]).toBe("user_123");
  });
});

describe("buildServiceTokenHeader", () => {
  it("formats service token header", () => {
    const headers = buildServiceTokenHeader("svc_token");
    expect(headers[SERVICE_TOKEN_HEADER]).toBe("svc_token");
  });
});
