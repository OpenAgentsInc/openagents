import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  // ─── Nostr cache (read-optimized) ─────────────────────────────────────────
  nostr_events: defineTable({
    event_id: v.string(),
    kind: v.number(),
    pubkey: v.string(),
    created_at: v.number(),
    content: v.string(),
    tags_json: v.string(),
    identifier: v.optional(v.string()),
    subclaw: v.optional(v.string()),
    parent_id: v.optional(v.string()),
    is_top_level: v.optional(v.boolean()),
    is_ai: v.optional(v.boolean()),
    seen_at: v.number(),
    relay: v.optional(v.string()),
  })
    .index("by_event_id", ["event_id"])
    .index("by_created_at", ["created_at"])
    .index("by_kind_created_at", ["kind", "created_at"])
    .index("by_kind_parent_id", ["kind", "parent_id"])
    .index("by_subclaw_created_at", ["subclaw", "created_at"])
    .index("by_pubkey_created_at", ["pubkey", "created_at"])
    .index("by_parent_id", ["parent_id"]),

  nostr_profiles: defineTable({
    pubkey: v.string(),
    name: v.optional(v.string()),
    picture: v.optional(v.string()),
    about: v.optional(v.string()),
    updated_at: v.number(),
  }).index("by_pubkey", ["pubkey"]),

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
    nostr_pubkey: v.optional(v.string()),
    nostr_npub: v.optional(v.string()),
    nostr_verified_at: v.optional(v.number()),
    nostr_verification_method: v.optional(v.string()),
    github_access_token: v.optional(v.string()),
    github_refresh_token: v.optional(v.string()),
    github_token_expires_at: v.optional(v.number()),
    github_scopes: v.optional(v.string()),
    stripe_customer_id: v.optional(v.string()),
  })
    .index("by_email", ["email"])
    .index("by_user_id", ["user_id"])
    .index("by_nostr_pubkey", ["nostr_pubkey"])
    .index("by_stripe_customer_id", ["stripe_customer_id"]),

  openclaw_instances: defineTable({
    user_id: v.string(),
    status: v.string(),
    runtime_url: v.optional(v.string()),
    runtime_name: v.optional(v.string()),
    cf_account_id: v.optional(v.string()),
    cf_worker_name: v.optional(v.string()),
    cf_worker_id: v.optional(v.string()),
    cf_container_app_id: v.optional(v.string()),
    cf_container_app_name: v.optional(v.string()),
    r2_bucket_name: v.optional(v.string()),
    service_token_encrypted: v.optional(v.string()),
    service_token_iv: v.optional(v.string()),
    service_token_alg: v.optional(v.string()),
    provider_keys_encrypted: v.optional(v.string()),
    provider_keys_iv: v.optional(v.string()),
    provider_keys_alg: v.optional(v.string()),
    created_at: v.number(),
    updated_at: v.number(),
    last_ready_at: v.optional(v.number()),
  })
    .index("by_user_id", ["user_id"])
    .index("by_status", ["status"]),

  credit_ledger: defineTable({
    user_id: v.string(),
    kind: v.string(),
    amount_usd: v.number(),
    meta: v.optional(v.any()),
    created_at: v.number(),
  })
    .index("by_user_id", ["user_id"])
    .index("by_user_id_created_at", ["user_id", "created_at"])
    .index("by_user_id_kind", ["user_id", "kind"]),

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
    default_tools: v.optional(v.array(v.string())),
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

  project_repos: defineTable({
    project_id: v.id("projects"),
    repo_id: v.id("repos"),
    created_at: v.number(),
  })
    .index("by_project_id", ["project_id"])
    .index("by_repo_id", ["repo_id"])
    .index("by_project_id_and_repo_id", ["project_id", "repo_id"]),

  repos: defineTable({
    name: v.string(),
    provider: v.string(),
    owner: v.string(),
    default_branch: v.optional(v.string()),
    url: v.optional(v.string()),
    created_at: v.number(),
  }).index("by_provider_and_owner_and_name", ["provider", "owner", "name"]),

  threads: defineTable({
    chat_id: v.string(),
    user_id: v.string(),
    organization_id: v.optional(v.id("organizations")),
    project_id: v.optional(v.id("projects")),
    agent_slug: v.optional(v.string()),
    metadata: v.optional(v.any()),
    is_archived: v.optional(v.boolean()),
    created_at: v.number(),
    updated_at: v.number(),
    is_shared: v.optional(v.boolean()),
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
    .index("by_thread_and_created_at", ["thread_id", "created_at"])
    .index("by_organization_id", ["organization_id"]),

  message_embeddings: defineTable({
    message_id: v.string(),
    content_embedding: v.array(v.float64()),
    tool_embedding: v.optional(v.array(v.float64())),
    thread_id: v.optional(v.id("threads")),
    organization_id: v.optional(v.id("organizations")),
    user_id: v.string(),
    created_at: v.number(),
  })
    .index("by_message_id", ["message_id"])
    .index("by_organization_id", ["organization_id"]),

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

  issue_threads: defineTable({
    issue_id: v.id("issues"),
    thread_id: v.id("threads"),
    created_at: v.number(),
  })
    .index("by_issue_id", ["issue_id"])
    .index("by_thread_id", ["thread_id"])
    .index("by_issue_id_and_thread_id", ["issue_id", "thread_id"]),

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
