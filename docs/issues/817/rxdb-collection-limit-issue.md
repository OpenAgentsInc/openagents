# RxDB Collection Limit Issue in OpenAgents

## Problem Description

The OpenAgents app is experiencing a critical RxDB error that occurs during development:

```
RxError (COL23): In the open-source version of RxDB, the amount of collections that can exist in parallel is limited to 16.
```

This error has been challenging to fix because:

1. RxDB maintains persistent connections in IndexedDB
2. React's development mode causes multiple mounts/unmounts that create multiple database instances
3. Hot reloading exacerbates the problem by creating additional instances
4. Database cleanup is complicated by asynchronous operations and connection locking

## Root Causes

### 1. RxDB Collection Limit

The open-source version of RxDB has a hard limit of 16 parallel collections. Each database we create uses 3 collections (threads, messages, settings), so we can only have about 5 database instances before hitting this limit.

### 2. React Development Mode

React development mode, especially with Strict Mode, mounts components twice. This means our database initialization runs multiple times, creating multiple database instances that persist in IndexedDB.

### 3. Persistent IndexedDB Connections

IndexedDB connections persist across page refreshes in some cases, especially if there are pending operations. This leads to "zombie" connections that count against our collection limit.

### 4. Database Name Collision

When using a fixed database name (e.g., "openagents"), all instances use the same name, which leads to conflicts when trying to create multiple collections with the same name.

### 5. Connection Blocking

IndexedDB operations can block each other, particularly when trying to delete a database while it's being accessed. This causes our cleanup operations to fail silently.

## Solution Approaches

### Approach 1: Fixed Database Name with Better Cleanup (Failed)

Initially, we tried using a single database name with more aggressive cleanup:
- Problem: Database connections weren't being fully closed
- Problem: deleteDatabase operations were being blocked
- Problem: The destroy() method wasn't available in some instances

### Approach 2: In-Memory Fallback (Too Complex)

We tried implementing an in-memory fallback database:
- Problem: Too complex to mimic the full RxDB API
- Problem: Would require reimplementing many features

### Approach 3: Unique Database Names (Current Solution)

The current solution uses:
1. **Unique database names** for each instance in development using timestamps
2. **Instance counter** to track and limit recreations
3. **Force cleanup** that:
   - Safely destroys the current instance
   - Uses the IndexedDB.databases() API to find all databases
   - Deletes all matching database names
   - Uses fallback patterns for browsers without the databases() API
4. **Error recovery** to handle cases when collection limit is reached

## Implementation Details

### Database Creation

1. Each database gets a unique name with timestamp: `openagents_${timestamp}`
2. We track the number of instances created to prevent unlimited growth
3. After hitting a threshold, we force a complete cleanup
4. We use shorter cleanup intervals (10s) in development

### Error Handling

1. We detect COL23 collection limit errors specifically
2. When a collection limit error occurs, we:
   - Force all databases to be cleaned up
   - Wait for cleanup to complete
   - Reset the instance counter
   - Try creating a fresh database

### Database Cleanup

We implement two levels of cleanup:
1. **Normal cleanup**: Safely destroys the current instance
2. **Force cleanup**: Tries to delete ALL databases in IndexedDB that match our patterns

## Challenges That Remain

Despite the current solution, some issues still occur:

1. **Race conditions**: Multiple components might initialize databases simultaneously
2. **Browser variations**: Different browsers handle IndexedDB slightly differently
3. **Cleanup timing**: Database deletion operations can be delayed or blocked
4. **Error recovery**: Some errors might not be properly caught or handled

## Recommendations

For developers working with this code:

1. **Turn off React Strict Mode** during development to reduce double-mounting
2. **Use a simple database architecture** with as few collections as possible
3. **Consider the RxDB Premium version** if this is a production application
4. **Implement database singletons carefully** to prevent multiple instances
5. **Add more defensive error handling** in the application code

## Alternative Approaches

1. **Server-side database**: Move persistence to a server-side database
2. **LocalStorage fallback**: Use simpler storage mechanisms for development
3. **Different storage providers**: RxDB supports other storage options like memory
4. **Purchasing Premium RxDB**: Would remove the collection limit entirely

## Lessons Learned

1. **IndexedDB is complex**: Browser storage has many edge cases and limitations
2. **React development mode complicates state**: Double-mounting makes persistent connections difficult
3. **Error handling is critical**: Any database operations need robust error handling
4. **Cleanup is essential**: Always clean up database connections properly
5. **Development vs Production**: Consider different strategies for each environment

## References

1. [RxDB Collection Limit Documentation](https://rxdb.info/rx-collection.html#faq)
2. [IndexedDB Database Deletion](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/deleteDatabase)
3. [React Strict Mode Effects](https://react.dev/reference/react/StrictMode)
4. [RxDB Premium Features](https://rxdb.info/premium)