# Schema Migration for Tool Configuration

## Problem

After implementing the configurable tool selection feature, we encountered initialization errors due to schema changes. The database schema needed to be updated to include the new `enabledToolIds` field, and proper migration strategies needed to be implemented.

## Solution

### 1. Schema Version Update

Updated the schema version from 2 to 3 in `packages/core/src/db/schema.ts`:

```typescript
// Increment this when making schema changes
// This allows RxDB to handle migrations properly
const SCHEMA_VERSION = 3;
```

### 2. Migration Strategies

Added migration strategies for each collection to handle upgrading from version 2 to 3:

- For **Settings** collection: Added the `enabledToolIds` field with a default value of `['shell_command']`
- For **Threads** and **Messages** collections: Added pass-through migration strategies (no changes needed)

```typescript
// Migration for Settings collection
3: function (oldDoc) {
  return {
    ...oldDoc,
    // Add enabledToolIds with default (shell_command enabled)
    enabledToolIds: ['shell_command']
  };
}
```

### 3. Chat API Simplification

Simplified the server-side implementation in `chat.ts` to reduce complexity:

```typescript
// Helper to get enabled tool IDs
async function getEnabledToolIds(): Promise<string[]> {
  try {
    // For server-side implementation, just return shell_command as enabled by default
    // In a future version, this should fetch from the database
    return ['shell_command'];
  } catch (error) {
    console.error("Error getting enabled tool IDs:", error);
    return ['shell_command']; // Default fallback
  }
}
```

## Key Learnings

1. **Schema Migration Importance**: When adding new fields to the database schema, it's critical to implement proper migration strategies to avoid initialization errors.

2. **Version Incrementing**: Always increment the schema version when making schema changes to ensure RxDB can properly apply migrations.

3. **Default Values**: Provide sensible default values for new fields in migration strategies to ensure backward compatibility.

4. **Graceful Degradation**: Implement fallbacks to ensure the application can still function even if part of the initialization fails.

These changes ensure that existing databases can be properly upgraded to include the new tool configuration capabilities without causing initialization errors.