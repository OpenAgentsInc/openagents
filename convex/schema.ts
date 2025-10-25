import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  threads: defineTable({
    // threadId is optional to allow migrating existing rows created before this field existed.
    // New inserts should set it; the app upsert does.
    threadId: v.optional(v.string()),
    title: v.string(),
    rolloutPath: v.string(),
    resumeId: v.string(),
    projectId: v.string(),
    source: v.string(),
    createdAt: v.number(),
    updatedAt: v.number(),
  }),
  // Projects: mirrored from ~/.openagents/projects/<id>/PROJECT.md (filesystem is source of truth)
  // - id: folder name / slug identifier for the project
  // - repo: optional metadata for Git remotes
  projects: defineTable({
    id: v.string(),
    name: v.string(),
    workingDir: v.string(),
    repo: v.optional(v.object({
      provider: v.optional(v.string()),
      remote: v.optional(v.string()),
      url: v.optional(v.string()),
      branch: v.optional(v.string()),
    })),
    agentFile: v.optional(v.string()),
    instructions: v.optional(v.string()),
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    // Optional chaining keeps older Convex versions tolerant if index() is not available at typecheck time
    // Note: avoid reserved names like "by_id" and "by_creation_time"
    .index?.('by_project_id', ['id'])
    .index?.('by_name', ['name']),

  // Skills: merged view of personal (~/.openagents/skills), repo registry (./skills), and project-scoped (<workingDir>/skills)
  // - skillId: lowercase hyphen-case id (= folder name)
  // - source: 'user' | 'registry' | 'project'
  // - projectId: present only when source === 'project'
  skills: defineTable({
    skillId: v.string(),
    name: v.string(),
    description: v.string(),
    license: v.optional(v.string()),
    allowed_tools: v.optional(v.array(v.string())),
    metadata: v.optional(v.any()),
    source: v.string(), // 'user' | 'registry' | 'project'
    projectId: v.optional(v.string()),
    path: v.optional(v.string()),
    createdAt: v.number(),
    updatedAt: v.number(),
  })
    .index?.('by_skill_source_project', ['skillId', 'source', 'projectId'])
    .index?.('by_project', ['projectId']),
  messages: defineTable({
    threadId: v.string(),
    role: v.optional(v.string()), // 'user' | 'assistant' | 'system' (optional for non-message items)
    kind: v.string(), // 'message' | 'reason' | 'cmd' | 'file' | 'search' | 'mcp' | 'todo' | 'turn' | etc
    text: v.optional(v.string()),
    data: v.optional(v.any()),
    ts: v.number(),
    createdAt: v.number(),
  }).index?.('by_thread_ts', ['threadId', 'ts']),
  runs: defineTable({
    threadDocId: v.string(), // convex threads doc _id (string form)
    projectId: v.optional(v.string()),
    text: v.string(),
    role: v.string(), // usually 'user'
    status: v.string(), // 'pending' | 'processing' | 'done' | 'error'
    error: v.optional(v.string()),
    createdAt: v.number(),
    updatedAt: v.number(),
  }),
});
