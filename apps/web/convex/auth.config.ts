import type { AuthConfig } from 'convex/server';

const clientId = process.env.WORKOS_CLIENT_ID;
const e2eJwksUrl = 'https://openagents.com/api/auth/e2e/jwks';
const e2eIssuer = 'https://openagents.com/e2e';

export default {
  providers: [
    {
      type: 'customJwt',
      issuer: 'https://api.workos.com/',
      algorithm: 'RS256',
      jwks: `https://api.workos.com/sso/jwks/${clientId}`,
      applicationID: clientId,
    },
    {
      type: 'customJwt',
      issuer: `https://api.workos.com/user_management/${clientId}`,
      algorithm: 'RS256',
      jwks: `https://api.workos.com/sso/jwks/${clientId}`,
    },
    // Production E2E bypass: signed JWTs minted by the Worker under a secret key.
    // This is safe to include unconditionally: tokens cannot be forged without the Worker secret,
    // and issuance requires OA_E2E_BYPASS_SECRET on the Worker route.
    {
      type: 'customJwt',
      issuer: e2eIssuer,
      algorithm: 'RS256',
      jwks: e2eJwksUrl,
    },
  ],
} satisfies AuthConfig;
