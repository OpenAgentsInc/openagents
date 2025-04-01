# RxDB Collection Limit and Settings Persistence Resolution
**Timestamp:** 2025-03-31 20:45:00

## Summary of Fixed Issues

I successfully resolved multiple RxDB-related issues in the OpenAgents application:

1. Collection limit errors: 
```
RxError (COL23): In the open-source version of RxDB, the amount of collections that can exist in parallel is limited to 16.
```

2. Settings conflict errors when changing models:
```
Error inserting new settings document: RxError (CONFLICT)
```

3. Page refresh/alert loops when updating settings

These issues were primarily due to React's Strict Mode causing components to mount/unmount twice, and RxDB document revision conflicts.

## Solution Implemented

The solution has several comprehensive components:

### 1. Database Creation Synchronization

```typescript
// Track database creation attempts to handle Strict Mode double-mounting
let dbCreationInProgress = false;
let dbCreationPromise: Promise<Database> | null = null;

export async function createDatabase(): Promise<Database> {
  // If database creation is already in progress, return the promise
  if (dbCreationInProgress && dbCreationPromise) {
    return dbCreationPromise;
  }
  
  // Set flag to indicate we're creating the database
  dbCreationInProgress = true;
  
  // Create a promise to handle concurrent calls
  dbCreationPromise = (async () => {
    try {
      // Database creation logic...
    } finally {
      // Clear the creation flags regardless of outcome
      dbCreationInProgress = false;
      dbCreationPromise = null;
    }
  })();
  
  return dbCreationPromise;
}
```

### 2. Fixed Stable Database Name

```typescript
// Store a static database name for development instead of a dynamic one
// Using a reproducible name helps with development and prevents creating multiple databases
let DEV_DB_NAME = 'openagents_dev';
```

Using a fixed name rather than a timestamp-based name helps maintain consistent storage.

### 3. Enhanced RxDB Configuration

```typescript
// Create database with more resilient options
const db = await createRxDatabase<DatabaseCollections>({
  name: dbName,
  storage,
  multiInstance: false, // Single instance mode for better reliability
  ignoreDuplicate: true, // Ignore duplicate db creation for strict mode
  allowMultipleInstances: true, // Allow reopening closed instances
  eventReduce: true, // Reduce event load
  cleanupPolicy: {
    // Automatically clean up old revisions to prevent storage issues
    minimumCollectionAge: 1000 * 60 * 60 * 24, // 1 day
    minimumDeletedTime: 1000 * 60 * 60 * 24, // 1 day
    runEach: 1000 * 60 * 60 // every hour
  }
});
```

These options make RxDB more resilient to React Strict Mode's double mounting.

### 4. Proper Atomic Updates for Settings

```typescript
// Use the RxDB atomic update pattern WITHOUT touching the cache yet
await currentDoc.atomicUpdate(oldData => {
  // Create a new document with both old data and updates
  return {
    ...oldData,
    ...updates,
    _updateAttempt: retries,
    _updateTime: new Date().toISOString()
  };
});
```

This ensures we respect RxDB's revision tracking system, avoiding document conflicts.

### 5. Enhanced UI Recovery Without Page Refresh

```typescript
// Handle error without page reload
// Just show an error message and keep the UI state
alert("There was an error saving your model preference. The model will be used for this session only.");

// No reload, no localStorage.clear()
// Just maintain the UI state with the selected model
```

This prevents the page refresh loops that were happening when settings updates failed.

### 6. Improved Database Cleanup

```typescript
// Clean up all matching indexedDB databases
if (typeof window !== 'undefined' && window.indexedDB) {
  try {
    // Try to list and delete existing databases
    if (typeof window.indexedDB.databases === 'function') {
      const dbs = await window.indexedDB.databases() || [];
      for (const db of dbs) {
        if (db.name && db.name.includes('openagents')) {
          await window.indexedDB.deleteDatabase(db.name);
        }
      }
    } else {
      // Safari fallback - try known database names
      await window.indexedDB.deleteDatabase(PROD_DB_NAME);
      await window.indexedDB.deleteDatabase(DEV_DB_NAME);
    }
  } catch (err) {
    console.warn('Error cleaning up IndexedDB:', err);
  }
}
```

This handles cleanup across different browsers, including Safari which doesn't support `databases()`.

### 7. Graceful Settings Reset 

```typescript
if (defaultSettings) {
  // Update UI to reflect new settings
  setDefaultModelId(defaultSettings.defaultModel || 'qwen-qwq-32b');
  setApiKeys({});
  
  alert("Settings reset successfully.");
  
  // Load API keys (there should be none after reset)
  await loadApiKeys();
}
```

This updates the UI directly rather than forcing a page reload on settings reset.

## Technical Analysis

The root causes of the issues were:

1. **Collection Limit**: React Strict Mode's double-mounting combined with RxDB's 16 collection limit
2. **Revision Conflicts**: Multiple components trying to update settings simultaneously
3. **Page Refresh Loops**: Error recovery that forced page reloads, creating new issues

My solution addresses these problems by:

1. Using a mutex-style lock for database creation
2. Using a consistent, fixed database name
3. Using RxDB's atomic update pattern correctly
4. Improving error recovery without page refreshes
5. Enhancing database configuration for better resilience

## Results

After implementing these changes:
- No more collection limit errors
- Settings persist correctly between sessions
- No page refresh loops when changing models
- UI remains consistent and stable
- Better error resilience throughout the application

## Lessons Learned

1. React Strict Mode requires careful management of database resources
2. RxDB document revisions must be respected with atomic updates
3. Error recovery should preserve the user experience
4. Databases in web applications need browser-specific handling
5. Synchronization patterns are essential for shared resources

The solution provides a rock-solid foundation for the OpenAgents application, ensuring settings and preferences persist correctly while maintaining a smooth user experience.