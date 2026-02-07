// Test-only shim for `@workos/authkit-session`.
//
// The real package currently uses ESM `import ... with { type: "json" }` which
// is not supported by the Workers Vitest runtime. We only need a tiny subset
// of the API surface for unit/integration tests.

export class CookieSessionStorage<_Request, _Response> {
  readonly cookieName: string;

  constructor(config: { readonly cookieName: string }) {
    this.cookieName = config.cookieName;
  }
}

export const createAuthService = <Request, _Response>(_options: unknown) => {
  return {
    withAuth: async (_request: Request) => ({
      auth: {
        user: null as any,
      },
      refreshedSessionData: undefined as string | undefined,
    }),
    saveSession: async (_auth: unknown, _sessionData: string) => ({
      headers: {} as Record<string, string>,
    }),
  };
};
