export type ApiError = {
  code: string;
  message: string;
  details?: Record<string, unknown> | null;
};

export type ApiOk<T> = { ok: true; data: T };
export type ApiErr = { ok: false; error: ApiError };
export type ApiResponse<T> = ApiOk<T> | ApiErr;

export function ok<T>(data: T): ApiOk<T> {
  return { ok: true, data };
}

export function err(code: string, message: string, details?: Record<string, unknown> | null): ApiErr {
  return { ok: false, error: { code, message, details: details ?? null } };
}
