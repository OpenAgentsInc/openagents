import { issuer } from "@openauthjs/openauth";
import { CloudflareKvStorage } from "@openauthjs/openauth/storage/cloudflare-kv";
import { GithubProvider } from "@openauthjs/openauth/provider/github";
import { subjects, type User } from "./subjects";

// User management functions
async function getUser(claims: { sub: string; github_id: string; email: string; name?: string; avatar?: string; username: string }): Promise<User> {
  // Validate required fields
  if (!claims.sub || !claims.github_id || !claims.email || !claims.username) {
    throw new Error("Missing required user claims");
  }

  // For now, we'll create a user on-the-fly
  // In a real implementation, you might want to store this in a database
  // TODO: Implement persistent user storage
  return {
    id: claims.sub,
    email: claims.email,
    name: claims.name,
    avatar: claims.avatar,
    githubId: claims.github_id,
    githubUsername: claims.username,
  };
}

export default issuer({
  providers: {
    github: GithubProvider({
      clientID: (() => {
        const clientId = process.env.GITHUB_CLIENT_ID;
        if (!clientId) throw new Error("GITHUB_CLIENT_ID environment variable is required");
        return clientId;
      })(),
      clientSecret: (() => {
        const clientSecret = process.env.GITHUB_CLIENT_SECRET;
        if (!clientSecret) throw new Error("GITHUB_CLIENT_SECRET environment variable is required");
        return clientSecret;
      })(),
      scopes: ["user:email"],
    }),
  },
  storage: CloudflareKvStorage({
    namespace: "AUTH_STORAGE",
  }),
  subjects,
  success: async (ctx, value) => {
    // Handle successful GitHub authentication
    if (value.provider === "github") {
      try {
        const user = await getUser({
          sub: value.sub,
          github_id: value.claims.sub,
          email: value.claims.email,
          name: value.claims.name,
          avatar: value.claims.avatar_url,
          username: value.claims.login,
        });

        return ctx.subject("user", user);
      } catch (error) {
        console.error("Failed to create user from GitHub claims:", error);
        throw new Error("User creation failed");
      }
    }

    console.error("Authentication attempted with unsupported provider:", value.provider);
    throw new Error(`Unsupported provider: ${value.provider}`);
  },
  cookie: {
    // Configure for subdomain sharing
    domain: process.env.COOKIE_DOMAIN || undefined,
    secure: true,
    httpOnly: true,
    sameSite: "lax",
  },
});
