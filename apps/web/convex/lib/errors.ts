export type ConvexErrorCode =
  | "BAD_REQUEST"
  | "UNAUTHORIZED"
  | "FORBIDDEN"
  | "NOT_FOUND"
  | "CONFLICT";

export const codedError = (code: ConvexErrorCode, message: string): Error =>
  new Error(`${code}|${message}`);

export const fail = (code: ConvexErrorCode, message: string): never => {
  throw codedError(code, message);
};

export const requireFound = <T>(
  value: T | null | undefined,
  code: ConvexErrorCode,
  message: string,
): T => value ?? fail(code, message);
