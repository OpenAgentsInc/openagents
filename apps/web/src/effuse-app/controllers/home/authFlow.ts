const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

export const normalizeEmail = (raw: string): string => raw.trim().toLowerCase();

/** Basic email check: non-empty, has @, has domain with at least one dot. */
export function looksLikeEmail(value: string): boolean {
  const s = value.trim();
  if (!s) return false;
  const at = s.indexOf("@");
  if (at <= 0 || at === s.length - 1) return false;
  const after = s.slice(at + 1);
  return after.includes(".") && !after.startsWith(".") && !after.endsWith(".");
}

/** Exactly 6 digits. */
export function isSixDigitCode(value: string): boolean {
  return /^[0-9]{6}$/.test(value.replace(/\s+/g, ""));
}

export const homeApiRejectedReason = (error: unknown): string | null => {
  const rec = asRecord(error);
  if (!rec) return null;
  if (rec._tag !== "HomeApiRejectedError") return null;
  return typeof rec.reason === "string" ? rec.reason : null;
};

export const startCodeErrorMessage = (error: unknown): string =>
  homeApiRejectedReason(error) === "invalid_email"
    ? "Please enter a valid email address."
    : "Failed to send code. Try again.";

export const verifyCodeErrorMessage = (error: unknown): string =>
  homeApiRejectedReason(error) === "invalid_code"
    ? "Invalid code. Please try again."
    : "Verification failed. Try again.";
