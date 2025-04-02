# Fast Thread Deletion Feature

This document covers the implementation of the fast thread deletion feature in OpenAgents, which allows users to toggle between confirmation dialogs and instant deletion for chat threads.

## Overview

The fast thread deletion feature offers users two options for deleting chat threads:

1. **Confirmation Mode (Default)**: A confirmation dialog appears before deleting a thread
2. **Fast Mode**: Threads are deleted instantly without confirmation, with an undo option available 

This provides flexibility while maintaining a safety net for accidental deletions.

## User Experience

### With Confirmation (Default)
1. User clicks the delete icon (X) on a thread
2. A confirmation dialog appears: "Are you sure you want to delete this chat?"
3. User confirms or cancels the deletion
4. If confirmed, thread is deleted with a toast notification
5. Toast provides a 5-second undo option

### Without Confirmation (Fast Mode)
1. User clicks the delete icon (X) on a thread
2. Thread is deleted immediately
3. A toast notification appears with the thread name
4. Toast provides a 5-second undo option

## Implementation Details

### Settings Storage
- Preference is stored as `confirmThreadDeletion` in the settings database
- Default value is `true` (show confirmation)
- Setting persists across sessions

### Components and Files

#### 1. ThreadList Component
The main component handling thread deletion logic. Key changes:
- Added integration with the settings system
- Implemented conditional confirmation dialog
- Added toast notifications with undo functionality

#### 2. Preferences Page
A new settings page allowing users to toggle the confirmation preference:
- Located at `/settings/preferences`
- Provides a simple switch toggle with description
- Shows immediate feedback when changed

#### 3. Routes
Updated the application routes to include the new preferences page.

### Code Example

The core deletion logic:

```typescript
// Function to handle thread deletion
const handleDeleteWithToast = (threadId: string, threadTitle: string) => {
  // Optimistic update - add to pending deletes first
  setPendingDeletes(prev => new Set([...prev, threadId]));
  
  // Show toast with undo option
  toast.info(
    `Chat deleted`,
    {
      description: `"${threadTitle || 'Untitled'}" was deleted.`,
      action: {
        label: "Undo",
        onClick: () => {
          // Remove from pending deletes
          setPendingDeletes(prev => {
            const newSet = new Set([...prev]);
            newSet.delete(threadId);
            return newSet;
          });
          // Refresh to restore the thread in UI
          refresh();
        }
      },
      duration: 5000
    }
  );
  
  // Then call the actual delete function
  onDeleteThread(threadId);
};

// Use conditional logic based on user preference
if (confirmThreadDeletion) {
  // Show confirmation dialog if preference is set
  if (window.confirm('Are you sure you want to delete this chat?')) {
    handleDeleteWithToast(thread.id, thread.title);
  }
} else {
  // Delete immediately with toast if confirmation is disabled
  handleDeleteWithToast(thread.id, thread.title);
}
```

## Benefits

- **Increased efficiency** for power users who frequently delete threads
- **Maintained safety** through the undo option
- **User preference** respects different workflows
- **Consistent UX** with modern design patterns

## Future Enhancements

Potential future improvements:
- Add keyboard shortcuts for deletion and undo
- Batch deletion of multiple threads
- Thread archive functionality instead of permanent deletion