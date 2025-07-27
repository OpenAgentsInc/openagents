export default {
  providers: [
    {
      type: "customJwt",
      applicationID: "openagents-desktop", // Unique identifier for this app
      issuer: process.env.OPENAUTH_DOMAIN || "http://localhost:8787", // Must match JWT iss field exactly
      jwks: `${process.env.OPENAUTH_DOMAIN || "http://localhost:8787"}/.well-known/jwks.json`, // OpenAuth JWKS endpoint
      algorithm: "RS256" // OpenAuth uses RS256 algorithm
    },
  ],
};