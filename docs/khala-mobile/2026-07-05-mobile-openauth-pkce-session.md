# Khala Mobile OpenAuth PKCE Session Contract

Status: implemented server-side for #8468 on 2026-07-05. This doc covers the
native auth primitive only; #8469 owns issuing Khala Sync tokens from the
verified user session, and #8470 owns the Expo UI plus Tailnet contract
retirement.

## Native Client

- Client id: `openagents-khala-mobile` by default, overrideable through
  `OPENAUTH_MOBILE_CLIENT_ID`.
- Redirect URI: `khala://auth`.
- Provider: GitHub only for the MVP mobile client.
- OAuth response: authorization code with PKCE S256.
- Required authorization request fields:
  - `client_id=<mobile client id>`
  - `provider=github`
  - `response_type=code`
  - `redirect_uri=khala://auth`
  - `code_challenge_method=S256`
  - `code_challenge=<base64url 43..128 chars>`

The mobile client is public. It must not send or embed client secrets, admin
tokens, agent tokens, GitHub tokens, prompts, code, repo contents, or payment
material.

## Token And Session Flow

1. Mobile opens the OpenAuth issuer authorization URL in the system browser or
   in-app browser using the fields above.
2. OpenAuth completes GitHub login and redirects to `khala://auth?code=...`.
3. Mobile exchanges the code at the issuer token endpoint with the original
   PKCE verifier.
4. Mobile stores the returned access and refresh tokens in the platform secure
   credential store.
5. Mobile verifies the user session with:

```http
GET /api/mobile/auth/session
Authorization: Bearer <openauth access token>
```

The route returns the verified OpenAuth user projection. It is cookie-free and
uses the same subject verification path as browser sessions.

6. Mobile refreshes with the OpenAuth token endpoint when the access token
   expires.
7. Mobile signs out with:

```http
DELETE /api/mobile/auth/session
Authorization: Bearer <current openauth access token>
X-OpenAgents-Refresh-Token: <current openauth refresh token>
```

The refresh token may also be sent as JSON `{ "refreshToken": "..." }`.
Sign-out stores a server-side access-token revocation marker through the token
expiry window and removes the matching OpenAuth refresh token when supplied.

## Boundary

This session is a human OpenAuth user session for native APIs. It is not an
`oa_agent_` registered-agent credential, not an admin credential, not a GitHub
writeback grant, and not a Khala Sync token by itself. Downstream routes must
still check their own authorization and exact scopes.

Regression coverage:

- `apps/openagents.com/workers/api/src/auth/mobile-session.test.ts`
- `apps/openagents.com/workers/api/src/openagents-openapi-routes.test.ts`
