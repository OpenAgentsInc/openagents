import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  // ─── Website (feed, posting identity) ─────────────────────────────────────
  posting_identities: defineTable({
    name: v.string(),
    description: v.optional(v.string()),
    user_id: v.optional(v.string()),
    claim_url: v.optional(v.string()),
    created_at: v.number(),
  })
    .index("by_user_id", ["user_id"])
    .index("by_created_at", ["created_at"]),

  identity_tokens: defineTable({
    posting_identity_id: v.id("posting_identities"),
    token_hash: v.string(),
    name: v.optional(v.string()),
    last_used_at: v.optional(v.number()),
    created_at: v.number(),
    expires_at: v.optional(v.number()),
  })
    .index("by_posting_identity_id", ["posting_identity_id"])
    .index("by_token_hash", ["token_hash"]),

  posts: defineTable({
    title: v.string(),
    content: v.string(),
    posting_identity_id: v.id("posting_identities"),
    created_at: v.number(),
    updated_at: v.optional(v.number()),
  })
    .index("by_posting_identity_id", ["posting_identity_id"])
    .index("by_created_at", ["created_at"]),

  comments: defineTable({
    post_id: v.optional(v.id("posts")),
    posting_identity_id: v.optional(v.id("posting_identities")),
    content: v.string(),
    created_at: v.optional(v.number()),
    author: v.optional(v.string()), // legacy field
  })
    .index("by_post_id", ["post_id"])
    .index("by_post_id_and_created_at", ["post_id", "created_at"]),

  post_upvotes: defineTable({
    post_id: v.id("posts"),
    voter_id: v.id("posting_identities"),
    created_at: v.number(),
  })
    .index("by_post_id", ["post_id"])
    .index("by_voter", ["voter_id"]),

  comment_upvotes: defineTable({
    comment_id: v.id("comments"),
    voter_id: v.id("posting_identities"),
    created_at: v.number(),
  })
    .index("by_comment_id", ["comment_id"])
    .index("by_voter", ["voter_id"]),

  // ─── Core (control plane) ─────────────────────────────────────────────────
  users: defineTable({
    user_id: v.string(),
    name: v.optional(v.string()),
    username: v.optional(v.string()),
    email: v.optional(v.string()),
    image: v.optional(v.string()),
    credits: v.optional(v.number()),
    created_at: v.optional(v.number()),
    referrer_id: v.optional(v.string()),
    plan: v.optional(v.string()),
    github_access_token: v.optional(v.string()),
    github_refresh_token: v.optional(v.string()),
    github_token_expires_at: v.optional(v.number()),
    github_scopes: v.optional(v.string()),
    stripeCustomerId: v.optional(v.string()),
  })
    .index("by_email", ["email"])
    .index("by_user_id", ["user_id"])
    .index("by_stripe_customer_id", ["stripeCustomerId"]),

  api_tokens: defineTable({
    user_id: v.string(),
    token_hash: v.string(),
    name: v.string(),
    last_used_at: v.optional(v.number()),
    created_at: v.number(),
    expires_at: v.optional(v.number()),
  })
    .index("by_user_id", ["user_id"])
    .index("by_token_hash", ["token_hash"]),

  organizations: defineTable({
    name: v.string(),
    logo: v.optional(v.string()),
    plan: v.optional(v.string()),
    credits: v.optional(v.number()),
    created_at: v.number(),
    owner_id: v.string(),
  }).index("by_owner", ["owner_id"]),

  organization_members: defineTable({
    organization_id: v.id("organizations"),
    user_id: v.string(),
    role: v.string(),
    joined_at: v.number(),
  })
    .index("by_organization", ["organization_id"])
    .index("by_user", ["user_id"])
    .index("by_organization_and_user", ["organization_id", "user_id"]),

  projects: defineTable({
    name: v.string(),
    description: v.optional(v.string()),
    user_id: v.optional(v.string()),
    organization_id: v.optional(v.id("organizations")),
    system_prompt: v.optional(v.string()),
    default_model: v.optional(v.string()),
    default_tools: v.optional(v.string()),
    autopilot_spec: v.optional(v.string()),
    autopilot_plan: v.optional(v.string()),
    autopilot_plan_updated_at: v.optional(v.number()),
    created_at: v.number(),
    updated_at: v.number(),
    is_archived: v.boolean(),
  })
    .index("by_user", ["user_id"])
    .index("by_organization", ["organization_id"])
    .index("by_updated", ["updated_at"])
    .index("by_archived", ["is_archived"]),

  projectRepos: defineTable({
    projectId: v.id("projects"),
    repoId: v.id("repos"),
    createdAt: v.number(),
  })
    .index("by_projectId", ["projectId"])
    .index("by_repoId", ["repoId"])
    .index("by_projectId_and_repoId", ["projectId", "repoId"]),

  repos: defineTable({
    name: v.string(),
    provider: v.string(),
    owner: v.string(),
    default_branch: v.optional(v.string()),
    url: v.optional(v.string()),
    createdAt: v.number(),
  }).index("by_provider_and_owner_and_name", ["provider", "owner", "name"]),

  threads: defineTable({
    chat_id: v.string(),
    user_id: v.string(),
    organization_id: v.optional(v.id("organizations")),
    project_id: v.optional(v.id("projects")),
    agent_slug: v.optional(v.string()),
    metadata: v.optional(v.any()),
    isArchived: v.optional(v.boolean()),
    created_at: v.number(),
    updated_at: v.number(),
    isShared: v.optional(v.boolean()),
  })
    .index("by_chat_id", ["chat_id"])
    .index("by_user_id", ["user_id"])
    .index("by_organization_id", ["organization_id"])
    .index("by_project_id", ["project_id"]),

  messages: defineTable({
    thread_id: v.optional(v.id("threads")),
    user_id: v.string(),
    organization_id: v.optional(v.id("organizations")),
    project_id: v.optional(v.id("projects")),
    id: v.optional(v.string()),
    role: v.string(),
    content: v.string(),
    created_at: v.number(),
    tool_invocations: v.optional(v.any()),
    parts_json: v.optional(v.any()),
    annotations_json: v.optional(v.any()),
    finish_reason: v.optional(v.string()),
    embedding_id: v.optional(v.string()),
  })
    .index("by_thread_id", ["thread_id"])
    .index("by_thread_and_created_at", ["thread_id", "created_at"]),

  messageEmbeddings: defineTable({
    message_id: v.string(),
    content_embedding: v.array(v.float64()),
    tool_embedding: v.optional(v.array(v.float64())),
    thread_id: v.optional(v.id("threads")),
    organization_id: v.optional(v.id("organizations")),
    user_id: v.string(),
    created_at: v.number(),
  }).index("by_message_id", ["message_id"]),

  issues: defineTable({
    user_id: v.string(),
    organization_id: v.optional(v.id("organizations")),
    project_id: v.optional(v.id("projects")),
    identifier: v.string(),
    title: v.string(),
    description: v.optional(v.string()),
    status_id: v.string(),
    priority_id: v.string(),
    assignee_id: v.optional(v.string()),
    label_ids: v.optional(v.array(v.string())),
    rank: v.optional(v.number()),
    created_at: v.number(),
    updated_at: v.number(),
    due_date: v.optional(v.number()),
  })
    .index("by_project_id", ["project_id"])
    .index("by_organization_id", ["organization_id"])
    .index("by_user_id", ["user_id"])
    .index("by_status_id", ["status_id"])
    .index("by_updated_at", ["updated_at"]),

  issueThreads: defineTable({
    issueId: v.id("issues"),
    threadId: v.id("threads"),
    createdAt: v.number(),
  })
    .index("by_issueId", ["issueId"])
    .index("by_threadId", ["threadId"])
    .index("by_issueId_and_threadId", ["issueId", "threadId"]),

  knowledge: defineTable({
    title: v.optional(v.string()),
    content: v.string(),
    embedding: v.array(v.float64()),
    tags: v.optional(v.array(v.string())),
    source: v.optional(v.string()),
    user_id: v.optional(v.string()),
    organization_id: v.optional(v.id("organizations")),
    project_id: v.optional(v.id("projects")),
    created_at: v.number(),
  })
    .index("by_user", ["user_id"])
    .index("by_organization", ["organization_id"])
    .index("by_project", ["project_id"]),

  agents: defineTable({
    slug: v.string(),
    name: v.string(),
    description: v.optional(v.string()),
    system_prompt: v.optional(v.string()),
    model_id: v.optional(v.string()),
    sort_order: v.optional(v.number()),
    is_default: v.optional(v.boolean()),
    created_at: v.number(),
    updated_at: v.number(),
    requires_github: v.optional(v.boolean()),
    requires_repo: v.optional(v.boolean()),
    supports_tools: v.optional(v.boolean()),
    agent_type: v.optional(v.string()),
  }).index("by_slug", ["slug"]),

  numbers: defineTable({
    value: v.number(),
  }),
});
