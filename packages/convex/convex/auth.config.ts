export default {
  providers: [
    {
      type: "customJwt",
      applicationID: "Ov23lirHI1DWTzZ1zT1u", // Must match JWT aud field (GitHub OAuth App Client ID)
      issuer: process.env.OPENAUTH_DOMAIN || "https://auth.openagents.com", // Must match JWT iss field exactly
      jwks: `${process.env.OPENAUTH_DOMAIN || "https://auth.openagents.com"}/.well-known/jwks.json`, // OpenAuth JWKS endpoint
      algorithm: "ES256" // OpenAuth uses ES256 algorithm (not RS256)
    },
  ],
};