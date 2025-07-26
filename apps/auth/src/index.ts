import { issuer } from "@openauthjs/openauth";
import { CloudflareKvStorage } from "@openauthjs/openauth/storage/cloudflare-kv";
import { GithubProvider } from "@openauthjs/openauth/provider/github";
import { subjects, type User } from "./subjects";

// User management functions
async function getUser(claims: { sub: string; github_id: string; email: string; name?: string; avatar?: string; username: string }): Promise<User> {
  // For now, we'll create a user on-the-fly
  // In a real implementation, you might want to store this in a database
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
      clientID: process.env.GITHUB_CLIENT_ID!,
      clientSecret: process.env.GITHUB_CLIENT_SECRET!,
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
      const user = await getUser({
        sub: value.sub,
        github_id: value.claims.sub,
        email: value.claims.email,
        name: value.claims.name,
        avatar: value.claims.avatar_url,
        username: value.claims.login,
      });

      return ctx.subject("user", user);
    }

    throw new Error("Invalid provider");
  },
  cookie: {
    // Configure for subdomain sharing
    domain: process.env.COOKIE_DOMAIN || undefined,
    secure: true,
    httpOnly: true,
    sameSite: "lax",
  },
});
