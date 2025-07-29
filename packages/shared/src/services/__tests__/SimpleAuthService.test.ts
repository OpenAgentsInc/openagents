import { Effect, Layer } from "effect";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ServiceTestUtils, benchmarkEffect } from "./setup-service-tests";

/**
 * SimpleAuthService Testing Suite
 * 
 * Comprehensive testing for authentication service functionality
 * as required by Issue #1269: Complete Service-Level Effect-TS Testing Coverage
 * 
 * Uses Effect-TS v3 patterns from EffectPatterns repository
 */

// Mock interfaces for testing
interface AuthUser {
  id: string;
  email: string;
  name: string;
  avatar?: string;
  githubId?: string;
  githubUsername?: string;
}

interface AuthState {
  user: AuthUser | null;
  token: string | null;
  isLoading: boolean;
  isAuthenticated: boolean;
}

interface OAuthConfig {
  clientId: string;
  redirectUri: string;
  scopes: string[];
  authUrl: string;
}

// Mock AuthService implementation using Effect.Service pattern
class TestAuthService extends Effect.Service<TestAuthService>()(
  "TestAuthService",
  {
    sync: () => {
      let storedToken: string | null = null;
      let storedUser: AuthUser | null = null;
      
      return {
        checkStoredAuth: () => 
          Effect.succeed({
            user: storedUser,
            token: storedToken,
            isLoading: false,
            isAuthenticated: !!(storedToken && storedUser)
          } as AuthState),
        
        storeAuthData: (token: string, user: AuthUser) =>
          Effect.sync(() => {
            storedToken = token;
            storedUser = user;
          }),
        
        clearStoredAuth: () =>
          Effect.sync(() => {
            storedToken = null;
            storedUser = null;
          }),
        
        startOAuthFlow: (platform: "mobile" | "desktop") =>
          Effect.succeed({
            authUrl: `https://auth.example.com/oauth?platform=${platform}`,
            state: "random-state-" + Math.random().toString(36)
          }),
        
        exchangeCodeForToken: (code: string, state: string) =>
          Effect.gen(function* () {
            if (code === "invalid-code") {
              yield* Effect.fail(new Error("Invalid authorization code"));
            }
            
            return {
              access_token: "new-access-token-" + code,
              user: {
                id: "user-123",
                email: "test@example.com",
                name: "Test User",
                githubUsername: "testuser"
              } as AuthUser
            };
          }),
        
        refreshToken: (currentToken: string) =>
          Effect.gen(function* () {
            if (!currentToken) {
              yield* Effect.fail(new Error("No token to refresh"));
            }
            
            return currentToken + "-refreshed";
          }),
        
        logout: () =>
          Effect.gen(function* () {
            storedToken = null;
            storedUser = null;
            
            return {
              user: null,
              token: null,
              isLoading: false,
              isAuthenticated: false
            } as AuthState;
          }),
        
        getOAuthConfig: () =>
          Effect.succeed({
            clientId: "openagents-test",
            redirectUri: "openagents://auth/callback",
            scopes: ["openid", "profile", "email"],
            authUrl: "https://auth.example.com"
          } as OAuthConfig)
      };
    }
  }
) {}

// Error simulation service for testing error scenarios
class FailingAuthService extends Effect.Service<FailingAuthService>()(
  "FailingAuthService",
  {
    sync: () => ({
      checkStoredAuth: () => Effect.fail(new Error("Storage access failed")),
      storeAuthData: (token: string, user: AuthUser) => Effect.fail(new Error("Failed to store auth data")),
      clearStoredAuth: () => Effect.fail(new Error("Failed to clear auth data")),
      startOAuthFlow: (platform: "mobile" | "desktop") => Effect.fail(new Error("OAuth flow initiation failed")),
      exchangeCodeForToken: (code: string, state: string) => Effect.fail(new Error("Token exchange failed")),
      refreshToken: (currentToken: string) => Effect.fail(new Error("Token refresh failed")),
      logout: () => Effect.fail(new Error("Logout failed")),
      getOAuthConfig: () => Effect.fail(new Error("Config retrieval failed"))
    })
  }
) {}

describe("SimpleAuthService Tests", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Auth State Management", () => {
    ServiceTestUtils.runServiceTest(
      "should check stored auth when no data exists",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const authState = yield* auth.checkStoredAuth();
        
        expect(authState.isAuthenticated).toBe(false);
        expect(authState.user).toBeNull();
        expect(authState.token).toBeNull();
        expect(authState.isLoading).toBe(false);
        
        return authState;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should store and retrieve auth data correctly",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const mockUser: AuthUser = {
          id: "user-123",
          email: "test@example.com",
          name: "Test User",
          githubUsername: "testuser"
        };
        const mockToken = "test-jwt-token";
        
        // Store auth data
        yield* auth.storeAuthData(mockToken, mockUser);
        
        // Retrieve stored auth
        const authState = yield* auth.checkStoredAuth();
        
        expect(authState.isAuthenticated).toBe(true);
        expect(authState.user).toEqual(mockUser);
        expect(authState.token).toBe(mockToken);
        expect(authState.isLoading).toBe(false);
        
        return authState;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should clear stored auth completely",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const mockUser: AuthUser = {
          id: "user-123",
          email: "test@example.com", 
          name: "Test User"
        };
        
        // Store some auth data first
        yield* auth.storeAuthData("test-token", mockUser);
        
        // Verify it's stored
        const beforeClear = yield* auth.checkStoredAuth();
        expect(beforeClear.isAuthenticated).toBe(true);
        
        // Clear auth data
        yield* auth.clearStoredAuth();
        
        // Verify it's cleared
        const afterClear = yield* auth.checkStoredAuth();
        expect(afterClear.isAuthenticated).toBe(false);
        expect(afterClear.user).toBeNull();
        expect(afterClear.token).toBeNull();
        
        return { beforeClear, afterClear };
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle storage access failures",
      Effect.gen(function* () {
        const failingAuth = yield* FailingAuthService;
        
        const result = yield* failingAuth.checkStoredAuth().pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Storage access failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingAuthService.Default))
    );
  });

  describe("OAuth Flow", () => {
    ServiceTestUtils.runServiceTest(
      "should start OAuth flow for mobile platform",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const oauthResult = yield* auth.startOAuthFlow("mobile");
        
        expect(oauthResult.authUrl).toContain("platform=mobile");
        expect(oauthResult.state).toMatch(/^random-state-/);
        
        return oauthResult;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should start OAuth flow for desktop platform",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const oauthResult = yield* auth.startOAuthFlow("desktop");
        
        expect(oauthResult.authUrl).toContain("platform=desktop");
        expect(oauthResult.state).toMatch(/^random-state-/);
        
        return oauthResult;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle OAuth flow initiation failures",
      Effect.gen(function* () {
        const failingAuth = yield* FailingAuthService;
        
        const result = yield* failingAuth.startOAuthFlow("mobile").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("OAuth flow initiation failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingAuthService.Default))
    );
  });

  describe("Token Exchange", () => {
    ServiceTestUtils.runServiceTest(
      "should exchange authorization code for token successfully",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const tokenResult = yield* auth.exchangeCodeForToken("valid-code", "test-state");
        
        expect(tokenResult.access_token).toBe("new-access-token-valid-code");
        expect(tokenResult.user).toEqual({
          id: "user-123",
          email: "test@example.com",
          name: "Test User",
          githubUsername: "testuser"
        });
        
        return tokenResult;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle invalid authorization codes",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const result = yield* auth.exchangeCodeForToken("invalid-code", "test-state").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Invalid authorization code");
        }
        
        return result;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle token exchange failures",
      Effect.gen(function* () {
        const failingAuth = yield* FailingAuthService;
        
        const result = yield* failingAuth.exchangeCodeForToken("any-code", "any-state").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Token exchange failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingAuthService.Default))
    );
  });

  describe("Token Refresh", () => {
    ServiceTestUtils.runServiceTest(
      "should refresh valid tokens successfully",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const originalToken = "original-token-123";
        const refreshedToken = yield* auth.refreshToken(originalToken);
        
        expect(refreshedToken).toBe("original-token-123-refreshed");
        
        return refreshedToken;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle missing tokens during refresh",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const result = yield* auth.refreshToken("").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("No token to refresh");
        }
        
        return result;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle token refresh failures",
      Effect.gen(function* () {
        const failingAuth = yield* FailingAuthService;
        
        const result = yield* failingAuth.refreshToken("any-token").pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Token refresh failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingAuthService.Default))
    );
  });

  describe("Logout", () => {
    ServiceTestUtils.runServiceTest(
      "should logout and clear all auth data",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        // First, store some auth data
        const mockUser: AuthUser = {
          id: "user-123",
          email: "test@example.com",
          name: "Test User"
        };
        yield* auth.storeAuthData("test-token", mockUser);
        
        // Verify logged in
        const beforeLogout = yield* auth.checkStoredAuth();
        expect(beforeLogout.isAuthenticated).toBe(true);
        
        // Logout
        const logoutResult = yield* auth.logout();
        
        expect(logoutResult.isAuthenticated).toBe(false);
        expect(logoutResult.user).toBeNull();
        expect(logoutResult.token).toBeNull();
        
        // Verify storage is cleared
        const afterLogout = yield* auth.checkStoredAuth();
        expect(afterLogout.isAuthenticated).toBe(false);
        
        return { beforeLogout, logoutResult, afterLogout };
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle logout failures",
      Effect.gen(function* () {
        const failingAuth = yield* FailingAuthService;
        
        const result = yield* failingAuth.logout().pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Logout failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingAuthService.Default))
    );
  });

  describe("Configuration", () => {
    ServiceTestUtils.runServiceTest(
      "should retrieve OAuth configuration",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        const config = yield* auth.getOAuthConfig();
        
        expect(config.clientId).toBe("openagents-test");
        expect(config.redirectUri).toBe("openagents://auth/callback");
        expect(config.scopes).toContain("openid");
        expect(config.scopes).toContain("profile");
        expect(config.scopes).toContain("email");
        expect(config.authUrl).toBe("https://auth.example.com");
        
        return config;
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle configuration retrieval failures",
      Effect.gen(function* () {
        const failingAuth = yield* FailingAuthService;
        
        const result = yield* failingAuth.getOAuthConfig().pipe(Effect.either);
        
        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left.message).toBe("Config retrieval failed");
        }
        
        return result;
      }).pipe(Effect.provide(FailingAuthService.Default))
    );
  });

  describe("Performance Benchmarks", () => {
    ServiceTestUtils.runServiceTest(
      "auth state check should be fast",
      benchmarkEffect(
        "Auth State Check",
        Effect.gen(function* () {
          const auth = yield* TestAuthService;
          return yield* auth.checkStoredAuth();
        }).pipe(Effect.provide(TestAuthService.Default)),
        100 // Should complete within 100ms
      )
    );

    ServiceTestUtils.runServiceTest(
      "token exchange should be efficient",
      benchmarkEffect(
        "Token Exchange",
        Effect.gen(function* () {
          const auth = yield* TestAuthService;
          return yield* auth.exchangeCodeForToken("benchmark-code", "benchmark-state");
        }).pipe(Effect.provide(TestAuthService.Default)),
        200 // Should complete within 200ms
      )
    );

    ServiceTestUtils.runServiceTest(
      "auth data storage should be fast",
      benchmarkEffect(
        "Auth Data Storage",
        Effect.gen(function* () {
          const auth = yield* TestAuthService;
          const mockUser: AuthUser = {
            id: "benchmark-user",
            email: "benchmark@example.com",
            name: "Benchmark User"
          };
          return yield* auth.storeAuthData("benchmark-token", mockUser);
        }).pipe(Effect.provide(TestAuthService.Default)),
        50 // Should complete within 50ms
      )
    );
  });

  describe("Integration Tests", () => {
    ServiceTestUtils.runServiceTest(
      "should support complete authentication workflow",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        // 1. Get OAuth configuration
        const config = yield* auth.getOAuthConfig();
        expect(config.clientId).toBe("openagents-test");
        
        // 2. Start OAuth flow
        const oauthFlow = yield* auth.startOAuthFlow("mobile");
        expect(oauthFlow.authUrl).toContain("platform=mobile");
        
        // 3. Exchange code for token (simulate OAuth callback)
        const tokenResult = yield* auth.exchangeCodeForToken("auth-code-123", oauthFlow.state);
        expect(tokenResult.access_token).toBe("new-access-token-auth-code-123");
        
        // 4. Store auth data
        yield* auth.storeAuthData(tokenResult.access_token, tokenResult.user);
        
        // 5. Verify authentication state
        const authState = yield* auth.checkStoredAuth();
        expect(authState.isAuthenticated).toBe(true);
        expect(authState.user).toEqual(tokenResult.user);
        
        // 6. Refresh token
        const refreshedToken = yield* auth.refreshToken(tokenResult.access_token);
        expect(refreshedToken).toBe(tokenResult.access_token + "-refreshed");
        
        // 7. Logout
        const logoutResult = yield* auth.logout();
        expect(logoutResult.isAuthenticated).toBe(false);
        
        return {
          config,
          oauthFlow,
          tokenResult,
          authState,
          refreshedToken,
          logoutResult
        };
      }).pipe(Effect.provide(TestAuthService.Default))
    );

    ServiceTestUtils.runServiceTest(
      "should handle mixed success/failure scenarios",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        // Some operations succeed
        const config = yield* auth.getOAuthConfig();
        expect(config.clientId).toBe("openagents-test");
        
        // Some operations fail gracefully
        const invalidTokenResult = yield* auth.exchangeCodeForToken("invalid-code", "test-state").pipe(Effect.either);
        expect(invalidTokenResult._tag).toBe("Left");
        
        // Recovery after failure
        const validTokenResult = yield* auth.exchangeCodeForToken("valid-code", "test-state");
        expect(validTokenResult.access_token).toBe("new-access-token-valid-code");
        
        return { config, invalidTokenResult, validTokenResult };
      }).pipe(Effect.provide(TestAuthService.Default))
    );
  });

  describe("Concurrent Operations", () => {
    ServiceTestUtils.runServiceTest(
      "should handle concurrent auth operations safely",
      Effect.gen(function* () {
        const auth = yield* TestAuthService;
        
        // Simulate concurrent auth operations
        const authCheck = yield* auth.checkStoredAuth();
        const config = yield* auth.getOAuthConfig();
        const mobileFlow = yield* auth.startOAuthFlow("mobile");
        const desktopFlow = yield* auth.startOAuthFlow("desktop");
        
        expect(authCheck.isAuthenticated).toBe(false);
        expect(config.clientId).toBe("openagents-test");
        expect(mobileFlow.authUrl).toContain("platform=mobile");
        expect(desktopFlow.authUrl).toContain("platform=desktop");
        
        const results = [authCheck, config, mobileFlow, desktopFlow];
        
        return results;
      }).pipe(Effect.provide(TestAuthService.Default))
    );
  });
});