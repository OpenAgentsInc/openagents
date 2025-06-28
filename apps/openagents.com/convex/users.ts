import { query } from "./_generated/server";
import { auth } from "./auth";

export const getCurrentUser = query({
  args: {},
  handler: async (ctx) => {
    const userId = await auth.getUserId(ctx);
    if (!userId) {
      return null;
    }
    
    const user = await ctx.db.get(userId);
    if (!user) {
      return null;
    }
    
    // GitHub OAuth data is stored in the user document
    // Extract GitHub username from email or name
    const githubUsername = user.email?.split('@')[0] || user.name?.toLowerCase().replace(/\s+/g, '') || 'user';
    
    return {
      id: user._id,
      login: githubUsername,
      name: user.name || 'OpenAgents User',
      avatar_url: user.image || `https://github.com/identicons/${githubUsername}.png`,
      email: user.email || null,
    };
  },
});

export const updateUserProfile = query({
  args: {},
  handler: async (ctx) => {
    const userId = await auth.getUserId(ctx);
    if (!userId) {
      return null;
    }
    
    const user = await ctx.db.get(userId);
    return user;
  },
});