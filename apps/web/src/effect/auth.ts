import { Context, Effect, FiberRef, Layer, Schema } from 'effect';
import { TelemetryService } from './telemetry';
import { RequestContextService } from './requestContext';

export class AuthServiceError extends Schema.TaggedError<AuthServiceError>()('AuthServiceError', {
  operation: Schema.String,
  error: Schema.Defect,
}) {}

export class AuthSessionUser extends Schema.Class<AuthSessionUser>('AuthSessionUser')({
  id: Schema.String,
  email: Schema.NullOr(Schema.String),
  firstName: Schema.NullOr(Schema.String),
  lastName: Schema.NullOr(Schema.String),
}) {}

export class AuthSession extends Schema.Class<AuthSession>('AuthSession')({
  userId: Schema.NullOr(Schema.String),
  sessionId: Schema.NullOr(Schema.String),
  user: Schema.NullOr(AuthSessionUser),
}) {}

type AuthSessionResponse = {
  readonly ok: boolean;
  readonly userId: string | null;
  readonly sessionId: string | null;
  readonly token: string | null;
  readonly user: {
    readonly id: string;
    readonly email: string | null;
    readonly firstName: string | null;
    readonly lastName: string | null;
  } | null;
};

export type AuthServiceApi = {
  /** Best-effort session lookup (never performs redirects). */
  readonly getSession: () => Effect.Effect<AuthSession, AuthServiceError, RequestContextService>;

  /**
   * Stable auth scope for cache keying.
   * MUST be low-churn and MUST NOT include expiring access tokens.
   */
  readonly sessionScopeKey: () => Effect.Effect<string, never, RequestContextService>;

  /** Retrieve an access token suitable for Convex `setAuth` / HTTP auth. */
  readonly getAccessToken: (options: {
    readonly forceRefreshToken: boolean;
  }) => Effect.Effect<string | null, AuthServiceError, RequestContextService>;
};

export class AuthService extends Context.Tag('@openagents/web/AuthService')<AuthService, AuthServiceApi>() {}

type CachedAuthState = {
  readonly session: AuthSession;
  readonly token: string | null;
  readonly fetchedAtMs: number;
};

const CLIENT_CACHE_TTL_MS = 5_000;
let clientCache: CachedAuthState | null = null;

const serverCache = FiberRef.unsafeMake<CachedAuthState | null>(null);

const emptySession = (): AuthSession =>
  AuthSession.make({
    userId: null,
    sessionId: null,
    user: null,
  });

const normalizeSessionFromResponse = (raw: AuthSessionResponse): { session: AuthSession; token: string | null } => {
  const userId = typeof raw.userId === 'string' ? raw.userId : null;
  const sessionId = typeof raw.sessionId === 'string' ? raw.sessionId : null;
  const token = typeof raw.token === 'string' ? raw.token : null;
  const user =
    raw.user && typeof raw.user === 'object'
      ? AuthSessionUser.make({
          id: String(raw.user.id),
          email: raw.user.email == null ? null : String(raw.user.email),
          firstName: raw.user.firstName == null ? null : String(raw.user.firstName),
          lastName: raw.user.lastName == null ? null : String(raw.user.lastName),
        })
      : null;

  return {
    session: AuthSession.make({ userId, sessionId, user }),
    token,
  };
};

const fetchClientAuthState = Effect.fn('AuthService.fetchClientAuthState')(function* (options: {
  readonly forceRefreshToken: boolean;
}) {
  const now = Date.now();
  if (!options.forceRefreshToken && clientCache && now - clientCache.fetchedAtMs < CLIENT_CACHE_TTL_MS) {
    return clientCache;
  }

  const response = yield* Effect.tryPromise({
    try: () =>
      fetch('/api/auth/session', {
        method: 'GET',
        cache: 'no-store',
        headers: { accept: 'application/json' },
      }),
    catch: (error) => AuthServiceError.make({ operation: 'fetchClientAuthState.fetch', error }),
  });

  if (!response.ok) {
    yield* AuthServiceError.make({
      operation: 'fetchClientAuthState.http',
      error: new Error(`HTTP ${response.status}`),
    });
  }

  const json = yield* Effect.tryPromise({
    try: () => response.json() as Promise<unknown>,
    catch: (error) => AuthServiceError.make({ operation: 'fetchClientAuthState.json', error }),
  });

  if (!json || typeof json !== 'object' || (json as any).ok !== true) {
    yield* AuthServiceError.make({
      operation: 'fetchClientAuthState.shape',
      error: new Error('invalid /api/auth/session response'),
    });
  }

  const normalized = normalizeSessionFromResponse(json as AuthSessionResponse);
  const next: CachedAuthState = { ...normalized, fetchedAtMs: now };
  clientCache = next;
  return next;
});

type WorkOsAuthKit = {
  readonly withAuth: (request: Request) => Promise<{
    readonly auth: {
      readonly user: {
        readonly id: string;
        readonly email?: string | null;
        readonly firstName?: string | null;
        readonly lastName?: string | null;
      } | null;
      readonly accessToken?: string;
      readonly sessionId?: string;
    };
  }>;
};

let getWorkosAuthKit: (() => Promise<WorkOsAuthKit>) | undefined;
if ((import.meta as any).env?.SSR) {
  let cached: WorkOsAuthKit | null = null;
  getWorkosAuthKit = async () => {
    if (cached) return cached;
    const [{ createAuthService }, { WebCookieSessionStorage }] = await Promise.all([
      import('@workos/authkit-session'),
      import('../auth/sessionCookieStorage'),
    ]);
    cached = createAuthService<Request, Response>({
      sessionStorageFactory: (config) => new WebCookieSessionStorage(config),
    }) as unknown as WorkOsAuthKit;
    return cached;
  };
}

const fetchServerAuthState = Effect.fn('AuthService.fetchServerAuthState')(function* (request: Request) {
  const cached = yield* FiberRef.get(serverCache);
  if (cached) return cached;

  const getAuthKit = getWorkosAuthKit;
  if (!getAuthKit) {
    // Should never happen (SSR-only), but degrade safely.
    return { session: emptySession(), token: null, fetchedAtMs: Date.now() };
  }

  const authkit = yield* Effect.tryPromise({
    try: () => getAuthKit(),
    catch: (error) => AuthServiceError.make({ operation: 'fetchServerAuthState.init', error }),
  });

  const result = yield* Effect.tryPromise({
    try: () => authkit.withAuth(request),
    catch: (error) => AuthServiceError.make({ operation: 'fetchServerAuthState.withAuth', error }),
  });

  const user = result.auth.user
    ? AuthSessionUser.make({
        id: result.auth.user.id,
        email: result.auth.user.email ?? null,
        firstName: result.auth.user.firstName ?? null,
        lastName: result.auth.user.lastName ?? null,
      })
    : null;

  const session = AuthSession.make({
    userId: user?.id ?? null,
    sessionId: typeof result.auth.sessionId === 'string' ? result.auth.sessionId : null,
    user,
  });
  const token = user && typeof result.auth.accessToken === 'string' ? result.auth.accessToken : null;
  const next: CachedAuthState = { session, token, fetchedAtMs: Date.now() };
  yield* FiberRef.set(serverCache, next);
  return next;
});

export const AuthServiceLive = Layer.effect(
  AuthService,
  Effect.gen(function* () {
    const telemetry = yield* TelemetryService;

    const getSession = Effect.fn('AuthService.getSession')(function* () {
      const ctx = yield* RequestContextService;
      if (ctx._tag === 'Server') {
        const state = yield* fetchServerAuthState(ctx.request);
        return state.session;
      }
      if (ctx._tag === 'MissingServerRequest') {
        yield* telemetry.withNamespace('auth').log('warn', 'missing_request_context', {});
        return emptySession();
      }
      const state = yield* fetchClientAuthState({ forceRefreshToken: false });
      return state.session;
    });

    const getAccessToken = Effect.fn('AuthService.getAccessToken')(function* (options: {
      readonly forceRefreshToken: boolean;
    }) {
      const ctx = yield* RequestContextService;
      if (ctx._tag === 'Server') {
        const state = yield* fetchServerAuthState(ctx.request);
        return state.token;
      }
      if (ctx._tag === 'MissingServerRequest') {
        return null;
      }
      const state = yield* fetchClientAuthState({ forceRefreshToken: options.forceRefreshToken });
      return state.token;
    });

    const sessionScopeKey = Effect.fn('AuthService.sessionScopeKey')(function* () {
      const session = yield* getSession().pipe(Effect.catchAll(() => Effect.succeed(emptySession())));
      if (session.userId) return `user:${session.userId}`;
      if (session.sessionId) return `session:${session.sessionId}`;
      return 'anon';
    });

    return AuthService.of({ getSession, getAccessToken, sessionScopeKey });
  }),
);
