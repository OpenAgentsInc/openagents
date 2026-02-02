import type { Doc } from "../_generated/dataModel";

const ADMIN_EMAILS = [
  ...(process.env.CONVEX_ADMIN_EMAILS ?? "").split(","),
  ...(process.env.ADMIN_EMAILS ?? "").split(","),
  process.env.ADMIN_EMAIL ?? "",
]
  .map((email) => email.trim().toLowerCase())
  .filter(Boolean);

export const isAdminEmail = (email?: string | null): boolean => {
  if (!email) {
    return false;
  }
  if (ADMIN_EMAILS.length === 0) {
    return false;
  }
  return ADMIN_EMAILS.includes(email.trim().toLowerCase());
};

export const isAdminUser = (user: Doc<"users"> | null): boolean =>
  isAdminEmail(user?.email ?? undefined);
