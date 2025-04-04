# ThreadList Performance Fix Implementation Log

This log tracks the implementation of the ThreadList performance optimizations to fix excessive rerendering and adding visual deletion feedback.

## Overview

The ThreadList component in Coder was rerendering approximately every 300ms, causing performance issues. Furthermore, even after reducing the refresh interval, the component was still rerendering too frequently (every 1-2 seconds) without actual changes. Additionally, when deleting chats with similar names, it was difficult to tell which one was being deleted. The fix involves performance optimizations, deep comparison to prevent unnecessary rerenders, and adding visual feedback animation for deleting threads.

## Implementation Steps

### Step 1: Create the useThreadDeletion custom hook

Created a new custom hook at `/packages/core/src/chat/useThreadDeletion.ts` to extract and memoize thread deletion logic. This hook:

- Takes threads, refresh, createThread, updateThread, onDeleteThread, and onSelectThread as props
- Maintains pendingDeletes state for optimistic UI updates
- Has a useRef to track database repository initialization status
- Initializes repositories once on mount via useEffect
- Provides a memoized handleDeleteWithToast function with useCallback
- Preserves all undo/restore functionality from the original implementation
- Properly handles all edge cases for thread restoration

### Step 2: Create memoized sub-components

Created two new components to break down the large ThreadList component:

1. `/apps/coder/src/components/ThreadItem.tsx` - A memoized component for individual thread items:
   - Uses React.memo for performance optimization
   - Takes thread, isSelected, onSelect, and onDelete props
   - Renders thread title and delete button
   - Handles click events for selection

2. `/apps/coder/src/components/ThreadGroup.tsx` - A memoized component for thread groups:
   - Uses React.memo for performance optimization
   - Takes label, threads, selectedThreadId, onSelectThread, and onDeleteThread props
   - Renders group label and a list of ThreadItem components
   - Skips rendering if threads array is empty

### Step 3: Optimize the main ThreadList component

Refactored `/apps/coder/src/components/ThreadList.tsx` to:

- Wrap the component with React.memo
- Increase refreshInterval from 300ms to 3000ms (10x improvement)
- Use the custom useThreadDeletion hook
- Memoize date calculations (now, sevenDaysAgo, thirtyDaysAgo)
- Memoize the groupedThreads calculation with proper dependencies
- Memoize the handleDeleteClick event handler with useCallback
- Use a single initial refresh instead of refreshing on every render
- Use the new ThreadGroup components for rendering

### Step 4: Update the core package exports

Updated `/packages/core/src/chat/index.ts` to export the new useThreadDeletion hook so it can be imported from @openagents/core.

## Performance Improvements

The changes make several critical improvements:

1. **Reduced polling frequency**: Changed from 300ms to 3000ms, a 10x reduction in database queries
2. **Eliminated redundant mounts**: Fixed issues with multiple refresh mechanisms
3. **Extracted deletion logic**: Moved complex database operations out of render cycle
4. **Memoized handlers**: Prevented recreation of event handlers on every render
5. **Isolated rendering**: Divided UI into smaller, memoized components that only re-render when needed
6. **Optimized calculations**: Prevented unnecessary date and array calculations

## Preserved Fast-Delete Functionality

The solution fully preserves the fast-delete-threads functionality by:

1. **Maintaining optimistic updates**: The pendingDeletes set still immediately hides deleted threads
2. **Preserving undo capability**: The same toast notification with undo button is implemented
3. **Keeping full thread restoration**: Complete thread and message restoration is preserved
4. **Retaining user preferences**: The confirmation dialog preference is still respected
5. **Maintaining delayed deletion**: The actual deletion is still delayed to allow for undo

### Step 5: Fix the useThreads hook to prevent unnecessary rerenders

Identified the root cause of continued rerenders: even though we increased the refresh interval, the `useThreads` hook was updating state on every refresh interval regardless of whether there were actual changes.

The updated implementation:

1. Added a deep comparison function `areThreadListsEqual` to:
   - Compare thread lists by checking each thread's ID, title, updatedAt, modelId, and systemPrompt 
   - Only trigger state updates when actual changes are detected

2. Added safeguards to prevent memory leaks and state updates after unmount:
   - Added an `isMountedRef` to track component mount status
   - Check mount status before updating state
   - Properly clean up in the useEffect's return function

3. Fixed dependency issues in useCallback:
   - Improved dependency arrays to prevent unnecessary recreations of callbacks
   - Ensured proper state tracking between refreshes

4. Fixed optimistic updates to maintain consistent references:
   - Updated `lastThreadsRef` when making optimistic updates
   - Ensured all optimistic updates use a single pattern

These changes ensure that state only updates when threads actually change, preventing unnecessary rerenders while maintaining reactivity to real changes.

### Step 6: Add visual deletion feedback

Added visual feedback for thread deletion:

1. Added a CSS animation for deletion in `/apps/coder/src/styles/global.css`:
   - Created `@keyframes delete-flash` that alternates between light and bright destructive colors
   - Added `.animate-delete-flash` class that applies the animation

2. Updated `ThreadItem.tsx` component to:
   - Accept an `isDeleting` prop to track deletion state
   - Apply red background flash animation to threads being deleted
   - Apply destructive text color during deletion

3. Updated `ThreadGroup.tsx` to:
   - Accept a `deletingThreadIds` Set to track which threads are being deleted
   - Pass the deletion state to each ThreadItem

4. Updated the main `ThreadList.tsx` to:
   - Track deleting threads with a new `deletingThreadIds` state
   - Update the delete handler to set threads as deleting before actual deletion
   - Add small delays to ensure the animation is visible
   - Pass the deleting threads state to each ThreadGroup

## Expected Results

### Performance Improvements

The ThreadList component should now only re-render when:
- A new thread is created or deleted
- The user preference for thread deletion confirmation changes 
- The actual list of threads in the database changes (not on every refresh interval)

The optimizations apply at two levels:
1. **Component level**: Using React.memo and optimized state management
2. **Data level**: Using deep comparison to prevent state updates when thread data hasn't changed

The component maintains the 3-second refresh interval but will not trigger rerenders unless the thread data actually changes, providing a significant performance improvement.

### Visual Improvements

When a thread is deleted:
1. The thread item will flash with a red background animation
2. The text will appear in the destructive color
3. This animation will be visible briefly before the thread is removed from the UI

This makes it much clearer which thread is being deleted, especially when multiple threads have similar or identical names.