# ThreadList Performance Fix Implementation Log

This log tracks the implementation of the ThreadList performance optimizations to fix excessive rerendering.

## Overview

The ThreadList component in Coder was rerendering approximately every 300ms, causing performance issues. The fix involves several optimizations while preserving the fast-delete functionality with undo capabilities.

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

## Expected Result

The ThreadList component should now only re-render when:
- A new thread is created or deleted
- The user preference for thread deletion confirmation changes
- Every 3 seconds for the regular refresh (vs. every 0.3 seconds before)

This should dramatically improve performance and reduce unnecessary renders.