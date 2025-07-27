import { mutation, query } from "./_generated/server";
import { v } from "convex/values";
import type { Id } from "./_generated/dataModel";

// GitHub webhook event types we want to track for APM
const TRACKED_GITHUB_EVENTS = [
  "issues.opened",
  "issues.closed", 
  "issue_comment.created",
  "pull_request.opened",
  "pull_request.closed",
  "pull_request.merged",
  "pull_request_review.submitted",
  "push.push",
  "workflow_run.completed",
  "release.published",
] as const;

// Helper function to extract relevant metadata from GitHub events
function extractEventMetadata(event: string, payload: any): any {
  const baseMetadata = {
    actor: payload.sender?.login || "unknown",
  };

  switch (event) {
    case "issues":
      return {
        ...baseMetadata,
        issueNumber: payload.issue?.number,
        title: payload.issue?.title,
      };
    
    case "pull_request":
      return {
        ...baseMetadata,
        prNumber: payload.pull_request?.number,
        title: payload.pull_request?.title,
        additions: payload.pull_request?.additions,
        deletions: payload.pull_request?.deletions,
      };
    
    case "push":
      return {
        ...baseMetadata,
        branch: payload.ref?.replace('refs/heads/', ''),
        commitCount: payload.commits?.length || 0,
        commits: payload.commits?.map((commit: any) => ({
          message: commit.message,
          id: commit.id,
        })) || [],
      };
    
    case "workflow_run":
      return {
        ...baseMetadata,
        workflowName: payload.workflow_run?.name,
        conclusion: payload.workflow_run?.conclusion,
        runNumber: payload.workflow_run?.run_number,
      };
    
    case "issue_comment":
      return {
        ...baseMetadata,
        issueNumber: payload.issue?.number,
        commentPreview: payload.comment?.body,
      };
    
    case "release":
      return {
        ...baseMetadata,
        tagName: payload.release?.tag_name,
        releaseName: payload.release?.name,
        isDraft: payload.release?.draft,
        isPrerelease: payload.release?.prerelease,
      };
    
    default:
      return baseMetadata;
  }
}

// Helper function for GitHub activity tracking logic
export async function handleGitHubActivityTracking(ctx: any, args: any) {
  try {
    const eventType = `${args.event}.${args.action}`;
    
    // Only track events we care about for APM
    if (!TRACKED_GITHUB_EVENTS.includes(eventType as any)) {
      console.log(`⏭️ [GITHUB APM] Skipping untracked event: ${eventType}`);
      return { tracked: false, reason: "Event not tracked" };
    }

    // Try to find the user by GitHub username or user ID
    let user = null;
    if (args.githubUserId) {
      user = await ctx.db
        .query("users")
        .withIndex("by_github_id", (q: any) => q.eq("githubId", args.githubUserId.toString()))
        .first();
    } else if (args.githubUsername) {
      user = await ctx.db
        .query("users")
        .filter((q: any) => q.eq(q.field("githubUsername"), args.githubUsername))
        .first();
    }

    if (!user) {
      console.log(`❌ [GITHUB APM] User not found for GitHub activity: ${args.githubUsername || args.githubUserId}`);
      return { tracked: false, reason: "User not found" };
    }

    const now = Date.now();
    const timestamp = args.timestamp ? new Date(args.timestamp).getTime() : now;
    const deviceId = `github-${args.githubUsername || args.githubUserId}`;

    console.log(`📊 [GITHUB APM] Tracking ${eventType} for user ${args.githubUsername || args.githubUserId}`);

    // Find or create GitHub device session
    let deviceSession = await ctx.db
      .query("userDeviceSessions")
      .withIndex("by_device_id", (q: any) => q.eq("deviceId", deviceId))
      .first();

    if (deviceSession) {
      // Update existing session
      await ctx.db.patch(deviceSession._id, {
        sessionPeriods: [
          ...(deviceSession.sessionPeriods || []),
          {
            start: timestamp,
            end: timestamp + 60000, // Assume 1 minute of activity per GitHub event
          }
        ],
        actionsCount: {
          messages: deviceSession.actionsCount?.messages || 0,
          toolUses: deviceSession.actionsCount?.toolUses || 0,
          githubEvents: (deviceSession.actionsCount?.githubEvents || 0) + 1,
        },
        lastActivity: timestamp,
        metadata: {
          platform: "github",
          version: "webhook",
        },
      });
    } else {
      // Create new device session
      await ctx.db.insert("userDeviceSessions", {
        userId: user._id,
        deviceId,
        deviceType: "github",
        sessionPeriods: [
          {
            start: timestamp,
            end: timestamp + 60000, // Assume 1 minute of activity per GitHub event
          }
        ],
        actionsCount: {
          messages: 0,
          toolUses: 0,
          githubEvents: 1,
        },
        lastActivity: timestamp,
        metadata: {
          platform: "github",
          version: "webhook",
        },
      });
    }

    // Log the event for detailed tracking
    await ctx.db.insert("githubEvents", {
      userId: user._id,
      eventType: args.event,
      action: args.action,
      repository: args.repository || "unknown",
      timestamp: timestamp,
      metadata: extractEventMetadata(args.event, args.payload),
    });

    console.log(`✅ [GITHUB APM] Successfully tracked ${eventType} for ${args.githubUsername || args.githubUserId}`);
    
    return { 
      tracked: true, 
      userId: user._id,
      deviceId,
      eventType,
    };
  } catch (error) {
    console.error("Error processing GitHub webhook:", error);
    return { tracked: false, reason: "Processing error", error };
  }
}

// GitHub APM webhook handler
export const trackGitHubActivity = mutation({
  args: {
    event: v.string(),
    action: v.string(),
    payload: v.any(),
    githubUserId: v.optional(v.number()),
    githubUsername: v.optional(v.string()),
    repository: v.optional(v.string()),
    timestamp: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    return await handleGitHubActivityTracking(ctx, args);
  },
});

// Get GitHub activity stats for a user
export const getGitHubActivityStats = query({
  args: {
    timeWindow: v.optional(v.union(v.literal("1h"), v.literal("6h"), v.literal("1d"), v.literal("1w"), v.literal("1m"), v.literal("lifetime"))),
  },
  handler: async (ctx, args) => {
    // Get authenticated user
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      return null;
    }

    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      return null;
    }

    const now = Date.now();
    const window = args.timeWindow || "lifetime";
    const cutoff = getTimeCutoff(now, window);

    // Get GitHub events for the user in the time window
    let eventsQuery = ctx.db
      .query("githubEvents")
      .filter((q) => q.eq(q.field("userId"), user._id));

    const events = await eventsQuery.collect();
    
    const filteredEvents = cutoff 
      ? events.filter(event => event.timestamp >= cutoff)
      : events;

    // Group events by type
    const eventCounts = filteredEvents.reduce((acc, event) => {
      acc[event.eventType] = (acc[event.eventType] || 0) + 1;
      return acc;
    }, {} as Record<string, number>);

    // Calculate APM for GitHub activities
    const totalEvents = filteredEvents.length;
    const timeSpan = cutoff ? (now - cutoff) / (1000 * 60) : 
      filteredEvents.length > 0 ? 
        (now - Math.min(...filteredEvents.map(e => e.timestamp))) / (1000 * 60) : 0;
    
    const githubAPM = timeSpan > 0 ? totalEvents / timeSpan : 0;

    // Get repository activity
    const repositoryActivity = filteredEvents.reduce((acc, event) => {
      if (event.repository) {
        acc[event.repository] = (acc[event.repository] || 0) + 1;
      }
      return acc;
    }, {} as Record<string, number>);

    return {
      githubAPM,
      totalEvents,
      timeSpan,
      eventCounts,
      repositoryActivity,
      recentEvents: filteredEvents
        .sort((a, b) => b.timestamp - a.timestamp)
        .slice(0, 10)
        .map(event => ({
          eventType: event.eventType,
          repository: event.repository,
          timestamp: event.timestamp,
        })),
    };
  },
});

// Helper function for time cutoffs (matching claude.ts)
function getTimeCutoff(now: number, window: string): number | null {
  switch (window) {
    case "1h": return now - (1 * 60 * 60 * 1000);
    case "6h": return now - (6 * 60 * 60 * 1000);
    case "1d": return now - (24 * 60 * 60 * 1000);
    case "1w": return now - (7 * 24 * 60 * 60 * 1000);
    case "1m": return now - (30 * 24 * 60 * 60 * 1000);
    case "lifetime": return null;
    default: return null;
  }
}

// GitHub webhook endpoint for external integration
export const handleGitHubWebhook = mutation({
  args: {
    headers: v.any(),
    body: v.any(),
  },
  handler: async (ctx, args) => {
    const event = args.headers["x-github-event"];
    const payload = args.body;

    if (!event || !payload) {
      return { error: "Missing event or payload" };
    }

    // Extract user information from payload
    let githubUserId: number | undefined;
    let githubUsername: string | undefined;
    let repository: string | undefined;

    if (payload.sender) {
      githubUserId = payload.sender.id;
      githubUsername = payload.sender.login;
    }

    if (payload.repository) {
      repository = payload.repository.full_name;
    }

    // Track the GitHub activity directly
    const trackingArgs = {
      event,
      action: payload.action || "unknown",
      payload,
      githubUserId,
      githubUsername,
      repository,
      timestamp: new Date().toISOString(),
    };

    // Execute the tracking logic directly
    return await handleGitHubActivityTracking(ctx, trackingArgs);
  },
});