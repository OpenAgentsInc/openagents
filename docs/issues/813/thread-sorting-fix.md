# Thread Sorting Fix

## Problem

When renaming a thread, it was moving to the top of the thread list because the threads were being sorted by `updatedAt` timestamp. This behavior was confusing for users who expected threads to maintain their position in the list after simple operations like renaming.

## Root Cause

The thread list was being sorted by the `updatedAt` timestamp in the ThreadRepository:

```typescript
// Sort by updated time (newest first)
const threads = await this.db!.threads
  .find()
  .sort({ updatedAt: 'desc' })
  .exec();
```

This meant that any operation that updated a thread (like renaming) would cause it to appear at the top of the list because its `updatedAt` timestamp would be the most recent.

## Fix

Changed the sorting criteria in the ThreadRepository to use `createdAt` instead of `updatedAt`:

1. Updated the `getAllThreads` method:
```typescript
const threads = await this.db!.threads
  .find()
  .sort({ createdAt: 'desc' })
  .exec();
```

2. Updated the reactive query method for consistency:
```typescript
return this.db!.threads
  .find()
  .sort({ createdAt: 'desc' });
```

3. Updated the ThreadList component to display the creation date instead of the last updated date:
```typescript
<div className="text-xs text-muted-foreground">
  {formatDate(thread.createdAt)}
</div>
```

## Impact

Now threads maintain their position in the list based on when they were created, regardless of updates like renaming. This provides a more intuitive and stable user experience where threads don't jump around in the list after simple edits.

This change means:

1. Newest threads are still displayed at the top of the list when created
2. Renaming a thread no longer changes its position in the list
3. The displayed date now correctly reflects when the thread was created