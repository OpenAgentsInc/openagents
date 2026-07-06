# Khala Mobile OpenAuth PKCE Session Contract

Status: server-side auth and sync credential bridge implemented for #8468 and
#8469 on 2026-07-05. #8470 owns the Expo UI plus Tailnet contract retirement.

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

6. Mobile obtains Khala Sync credentials with:

```http
POST /api/mobile/session
Authorization: Bearer <openauth access token>
```

The response is the existing app credential shape:

```json
{
  "ownerUserId": "github:12345",
  "syncToken": "<current openauth access token>"
}
```

`syncToken` is deliberately the current OpenAuth mobile access token. The
existing `/api/sync/*` routes already resolve that bearer as the signed-in human
actor and then apply the normal Khala Sync scope-read/write gates:
`scope.user.<ownerUserId>` for the owner, plus owned thread scopes via the
existing resolver. A foreign `scope.user.*` remains denied before storage reads.

7. Mobile refreshes with the OpenAuth token endpoint when the access token
   expires.
8. After refresh, mobile should call `POST /api/mobile/session` again and
   replace its stored `syncToken` with the refreshed access token.
9. Mobile signs out with:

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
writeback grant, and not a separate Khala Sync authority. `/api/sync/*` accepts
the current access token only through the standard human actor path, and then
still checks exact Khala Sync scopes.

Regression coverage:

- `apps/openagents.com/workers/api/src/auth/mobile-session.test.ts`
- `apps/openagents.com/workers/api/src/openagents-openapi-routes.test.ts`
