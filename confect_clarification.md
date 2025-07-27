## 🧁 **CORRECTION: Using Confect Directly**

I was being inconsistent. In the main issue #1234 analysis, we decided on **Confect integration** but then I created this issue with a wrapper approach. That's stupid.

### **Updated Approach: Direct Confect Implementation**

Since we're refactoring mobile sync anyway and want this done in **48 hours** (not weeks), we should go **straight to Confect**:

```typescript
// packages/convex/confect/schema.ts
export const confectSchema = defineSchema({
  claudeSessions: defineTable(
    Schema.Struct({
      sessionId: Schema.String.pipe(Schema.nonEmpty()),
      projectPath: Schema.String.pipe(Schema.nonEmpty()),
      status: Schema.Literal("active", "inactive", "error", "processed"),
      createdBy: Schema.Literal("desktop", "mobile"),
      lastActivity: Schema.Number,
      userId: Schema.optional(Id.Id("users")),
      metadata: Schema.optional(Schema.Struct({
        workingDirectory: Schema.optional(Schema.String),
        originalMobileSessionId: Schema.optional(Schema.String),
      }))
    })
  ).index("by_session_id", ["sessionId"])
});

// packages/convex/confect/mobile-sync.ts
export const processMobileSession = mutation({
  args: ProcessMobileSessionArgs,
  returns: ProcessMobileSessionResult,
  handler: ({ sessionId, projectPath, title }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      
      const existingSession = yield* db
        .query("claudeSessions")
        .withIndex("by_session_id", (q) => q.eq("sessionId", sessionId))
        .first();
        
      return yield* Option.match(existingSession, {
        onNone: () => db.insert("claudeSessions", {
          sessionId,
          projectPath,
          status: "active" as const,
          createdBy: "mobile" as const,
          lastActivity: Date.now(),
          title
        }),
        onSome: (session) => db.patch(session._id, {
          status: "processed" as const,
          lastActivity: Date.now()
        }).pipe(Effect.as(session._id))
      });
    })
});
```

### **Why Confect Now vs Wrapper Later**

**Confect Direct Benefits:**
- ✅ `Option<Session>` instead of `Session | null` (no more manual null checks)
- ✅ Effect Schema validation throughout
- ✅ Automatic encode/decode at boundaries
- ✅ Tagged errors with exhaustive handling
- ✅ One migration instead of two

**Implementation:**
1. **Install Confect** in `packages/convex`
2. **Convert mobile sync schema** to Effect Schema 
3. **Rewrite mobile sync functions** with Confect patterns
4. **Update `useMobileSessionSync.ts`** to use Confect hooks
5. **Done in 48 hours** - no intermediate wrapper step

This is the right approach for the 48-hour timeline.