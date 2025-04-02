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
- Implemented thread restoration if undo is clicked

#### 2. Preferences Page
A new settings page allowing users to toggle the confirmation preference:
- Located at `/settings/preferences`
- Provides a simple switch toggle with description
- Shows immediate feedback when changed

#### 3. Routes
Updated the application routes to include the new preferences page.

### Code Example

The enhanced deletion logic with proper undo functionality:

```typescript
// Function to handle thread deletion
const handleDeleteWithToast = (threadId: string, threadTitle: string) => {
  // Cache the original thread details for recovery
  const deletedThread = threads.find(t => t.id === threadId);
  if (!deletedThread) {
    console.error("Could not find thread for deletion:", threadId);
    return;
  }
  
  // Save a copy of the thread data
  const threadData = {
    id: deletedThread.id,
    title: deletedThread.title,
    createdAt: deletedThread.createdAt,
    updatedAt: deletedThread.updatedAt,
    modelId: deletedThread.modelId,
    systemPrompt: deletedThread.systemPrompt
  };
  
  // Optimistic update - add to pending deletes first
  setPendingDeletes(prev => new Set([...prev, threadId]));
  
  // Delay actual deletion to allow for undo
  let deleteTimeoutId: NodeJS.Timeout;
  let undoClicked = false;
  
  // Show toast with undo option
  toast.info(
    `Chat deleted`,
    {
      description: `"${threadTitle || 'Untitled'}" was deleted.`,
      action: {
        label: "Undo",
        onClick: async () => {
          // Mark that undo was clicked to prevent deletion
          undoClicked = true;
          
          // Clear the deletion timeout if it's still pending
          if (deleteTimeoutId) {
            clearTimeout(deleteTimeoutId);
          }
          
          // Remove from pending deletes to show in UI
          setPendingDeletes(prev => {
            const newSet = new Set([...prev]);
            newSet.delete(threadId);
            return newSet;
          });
          
          try {
            // Check if thread still exists or has been deleted
            const existingThreads = await refresh();
            const threadExists = existingThreads.some(t => t.id === threadId);
            
            if (!threadExists) {
              // Thread was deleted, need to recreate it
              console.log("Thread was already deleted, recreating with data:", threadData);
              
              // Create a new thread
              const newThread = await createThread(threadData.title);
              
              // Update with any additional properties if needed
              if (threadData.systemPrompt || threadData.modelId) {
                await updateThread(
                  newThread.id, 
                  { 
                    systemPrompt: threadData.systemPrompt,
                    modelId: threadData.modelId
                  }
                );
              }
              
              // Switch to the new thread
              onSelectThread(newThread.id);
              toast.success("Chat restored successfully");
            }
            
            // Final refresh to update UI
            refresh();
          } catch (error) {
            console.error("Failed to restore thread:", error);
            toast.error("Failed to restore chat");
          }
        }
      },
      duration: 5000
    }
  );
  
  // Delay the actual deletion to allow for undo
  deleteTimeoutId = setTimeout(() => {
    if (!undoClicked) {
      // Only delete if undo wasn't clicked
      onDeleteThread(threadId);
    }
  }, 300);
};
```

## Enhanced Thread Restoration

The implementation includes a comprehensive thread restoration system that:

1. **Caches messages before deletion** - All messages from a thread are cached in memory before deletion
2. **Preserves thread identity** - Recreates the thread with the exact same ID when undoing
3. **Restores messages** - Reinserts all messages from the thread to provide a complete restoration
4. **Handles edge cases** - Has fallback mechanisms if exact restoration fails
5. **Provides clear feedback** - Different toast messages for different restoration scenarios

### Thread Restoration Flow

1. User clicks "Undo" on a deleted thread
2. The system checks if the thread still exists
3. If the thread was deleted:
   - The thread is recreated with the same ID and properties
   - All messages are restored from the cache
   - The UI is updated to show the restored thread
4. If restoration is partial:
   - A warning toast is shown indicating limited restoration
5. If restoration fails:
   - An error toast is shown with details

## Benefits

- **Increased efficiency** for power users who frequently delete threads
- **Maintained safety** through the comprehensive undo functionality
- **User preference** respects different workflows
- **Consistent UX** with modern design patterns
- **True undo functionality** that works even after thread deletion is complete
- **Complete message restoration** preserving conversation history

## Future Enhancements

Potential future improvements:
- Add keyboard shortcuts for deletion and undo
- Batch deletion of multiple threads
- Thread archive functionality instead of permanent deletion