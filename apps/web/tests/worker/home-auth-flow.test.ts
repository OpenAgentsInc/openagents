import { describe, expect, it } from "vitest";

import {
  homeApiRejectedReason,
  isSixDigitCode,
  looksLikeEmail,
  normalizeEmail,
  startCodeErrorMessage,
  verifyCodeErrorMessage,
} from "../../src/effuse-app/controllers/home/authFlow";

describe("apps/web home auth flow helpers", () => {
  it("normalizes and validates email-like input", () => {
    expect(normalizeEmail("  USER@Example.COM ")).toBe("user@example.com");
    expect(looksLikeEmail("user@example.com")).toBe(true);
    expect(looksLikeEmail("user@localhost")).toBe(false);
    expect(looksLikeEmail("bad")).toBe(false);
  });

  it("validates exactly six digits for auth code", () => {
    expect(isSixDigitCode("123456")).toBe(true);
    expect(isSixDigitCode("123 456")).toBe(true);
    expect(isSixDigitCode("12345")).toBe(false);
    expect(isSixDigitCode("1234567")).toBe(false);
  });

  it("maps HomeApi rejected reasons to stable UI copy", () => {
    const invalidEmail = { _tag: "HomeApiRejectedError", reason: "invalid_email" };
    const invalidCode = { _tag: "HomeApiRejectedError", reason: "invalid_code" };
    const other = { _tag: "HomeApiRejectedError", reason: "other" };

    expect(homeApiRejectedReason(invalidEmail)).toBe("invalid_email");
    expect(homeApiRejectedReason({ _tag: "OtherError" })).toBeNull();

    expect(startCodeErrorMessage(invalidEmail)).toBe("Please enter a valid email address.");
    expect(startCodeErrorMessage(other)).toBe("Failed to send code. Try again.");

    expect(verifyCodeErrorMessage(invalidCode)).toBe("Invalid code. Please try again.");
    expect(verifyCodeErrorMessage(other)).toBe("Verification failed. Try again.");
  });
});
