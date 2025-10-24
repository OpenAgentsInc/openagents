import { defineSchema, defineTable } from "convex/server";

export default defineSchema({
  threads: defineTable({
    title: "string",
    rolloutPath: "string",
    resumeId: "string",
    projectId: "string",
    source: "string",
    createdAt: "number",
    updatedAt: "number",
  }),
});

