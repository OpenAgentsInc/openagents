# OpenAuth: Self-hosted edge authentication for modern applications

OpenAuth is a **universal, standards-based OAuth 2.0 authentication server** built by the SST team that deploys seamlessly to Cloudflare Workers, providing a self-hosted alternative to expensive SaaS auth providers. The framework runs on **Hono** for edge compatibility, stores minimal data in **Cloudflare KV**, and delivers sub-millisecond authentication globally with **zero cold starts**. For your OpenAgents project, OpenAuth can be deployed at auth.openagents.com to handle authentication for both web and native applications, integrating cleanly with Convex through JWT validation while maintaining OAuth 2.0 compliance.

## Edge-native authentication architecture

OpenAuth represents a paradigm shift in authentication infrastructure by combining the centralized auth server model of Auth0 or Clerk with the deployment flexibility of serverless functions. Built on the **Hono web framework**, it achieves universal deployment across Node.js, Bun, AWS Lambda, and Cloudflare Workers without modification. The architecture deliberately avoids user management abstractions, instead providing **callback-based user resolution** that integrates with any existing database or user system.

The framework implements the complete **OAuth 2.0 specification** including authorization code flow with PKCE, refresh tokens, and dynamic client registration. Unlike traditional auth libraries that embed into applications, OpenAuth runs as a standalone service that can authenticate multiple applications across your ecosystem. It ships with **prebuilt UI components** using themes inspired by Vercel, Supabase, and terminal interfaces, though developers can opt out entirely for custom implementations.

Provider support spans both third-party OAuth providers like **GitHub and Google** as well as built-in flows for **password and PIN-based authentication**. The password provider includes complete registration, login, forgot password, and email verification flows. All providers use a type-safe configuration system with **Valibot validation**, ensuring runtime safety and excellent developer experience through TypeScript inference.

## Cloudflare Workers deployment blueprint

Deploying OpenAuth to Cloudflare Workers requires minimal configuration but understanding the specifics ensures optimal performance. Start by creating a new worker with **wrangler** and installing the OpenAuth packages along with the Cloudflare storage adapter. The core implementation typically requires **less than 50 lines of code**, making it remarkably concise for a complete auth server.

The critical deployment configuration lives in **wrangler.toml**, where you'll define KV namespace bindings, environment variables, and custom domain routing. For production deployments on Cloudflare's **$5/month paid plan**, you get unlimited requests, enhanced KV quotas, and 50ms CPU time per request. The free tier supports **100,000 requests daily** with basic KV operations, sufficient for development and small applications.

Cloudflare KV serves as OpenAuth's storage backend, handling refresh tokens, password hashes, and temporary verification codes with **sub-10ms read latency** for hot keys. The storage adapter requires only a namespace binding, with OpenAuth managing all data operations internally. For auth.openagents.com deployment, configure a **custom domain route** in wrangler.toml, and Cloudflare automatically provisions SSL certificates with no additional configuration.

Cookie configuration for subdomain authentication requires setting the domain to **.openagents.com** to enable cross-subdomain cookie sharing. OpenAuth automatically configures secure cookie settings including **httpOnly, secure, and sameSite attributes**. CORS policies need explicit origin lists for your application domains, with credentials enabled for cookie-based authentication flows.

## Security architecture and threat mitigation

OpenAuth's security model builds on **OWASP authentication guidelines** and OAuth 2.0 security best practices. The framework implements CSRF protection through unpredictable state parameters, validates all tokens with constant-time comparison algorithms, and stores passwords using **cryptographically strong salted hashes**. Rate limiting should be implemented at the Cloudflare layer to prevent brute force attacks, with stricter limits on sensitive endpoints like token exchange.

Token management follows security best practices with **short-lived JWTs** for access tokens (typically 1 hour) and secure refresh token storage in KV. The framework supports token rotation on refresh, reducing the window for token compromise. All authentication flows require **TLS encryption**, enforced by Cloudflare's edge network by default.

For desktop and mobile applications in the OpenAgents ecosystem, implement the **authorization code flow with PKCE** to prevent code interception attacks. PKCE eliminates the need for client secrets in public clients while maintaining security through code verifiers and challenges. The flow works seamlessly with custom URI schemes in desktop applications and embedded secure browsers in mobile apps.

Production deployments should configure comprehensive security headers including **Strict-Transport-Security, X-Frame-Options, and Content-Security-Policy**. Monitor authentication metrics through Cloudflare Analytics, tracking failed login attempts, token validation errors, and unusual geographic patterns. Implement alerting for anomalous authentication patterns that might indicate credential stuffing or brute force attempts.

## Convex integration and real-time authentication

Integrating OpenAuth with Convex requires configuring OpenAuth as a **custom JWT provider** in Convex's authentication system. Convex validates JWTs on every function call, extracting the subject claims that OpenAuth embeds during the success callback. The integration maintains real-time authentication state across your application through Convex's WebSocket connections.

Configure Convex to accept OpenAuth tokens by adding your OpenAuth domain to the providers list in **convex/auth.config.ts**. OpenAuth's JWT tokens include standard claims (iss, sub, aud, exp) plus custom subject data you define during user authentication. In Convex functions, access the authenticated user through **ctx.auth.getUserIdentity()**, which returns the validated token claims.

The authentication flow for OpenAgents would start with users authenticating through OpenAuth at auth.openagents.com, receiving JWT access tokens, then using those tokens to authenticate Convex queries and mutations. For the Claude Code wrapper use case, implement **machine-to-machine authentication** using the client credentials flow, allowing the desktop and mobile apps to authenticate without user interaction for background operations.

Session synchronization between OpenAuth and Convex happens automatically through token validation. When access tokens expire, client applications should use refresh tokens to obtain new access tokens from OpenAuth, maintaining seamless authentication without user intervention. Implement **token refresh logic** in your client SDK to handle the refresh flow transparently.

## Production deployment strategies

Successful production deployment of OpenAuth on Cloudflare Workers requires attention to operational excellence beyond basic configuration. Implement **blue-green deployments** using Cloudflare's percentage-based routing to test new versions with minimal risk. Use separate KV namespaces for staging and production environments, preventing test data from affecting production users.

Monitor key performance indicators including **authentication success rates, token validation latency, and KV operation costs**. Cloudflare's built-in analytics provide request-level data, but consider implementing custom metrics using Analytics Engine for business-specific tracking. Set up **cost alerts** for KV operations, as write operations cost $5 per million requests.

For high availability, deploy to multiple custom domains like auth.openagents.com and auth-backup.openagents.com, using Cloudflare's load balancing to distribute traffic. While Cloudflare's global network provides inherent redundancy, having multiple endpoints ensures availability during configuration changes or routing updates.

Implement comprehensive **audit logging** through Cloudflare's logging services, capturing authentication events, token exchanges, and administrative actions. Store logs in R2 or external systems for long-term retention and compliance requirements. Regular security assessments should include penetration testing of authentication flows, review of JWT signing keys, and validation of CORS policies.

The combination of OpenAuth's standards-based architecture with Cloudflare Workers' edge infrastructure provides a **cost-effective, globally scalable authentication solution** that maintains sovereignty over user data while delivering performance comparable to major SaaS providers. For the OpenAgents project, this architecture enables secure authentication across web, desktop, and mobile platforms while maintaining the flexibility to evolve with changing requirements.
