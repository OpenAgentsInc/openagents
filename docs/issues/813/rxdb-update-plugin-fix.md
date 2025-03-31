# RxDB Update Plugin Fix

## Problem

When trying to update thread timestamps, we encountered the following error:

```
Error updating thread timestamp: Error: You are using a function which must be overwritten by a plugin.
You should either prevent the usage of this function or add the plugin via:
   import { RxDBUpdatePlugin } from 'rxdb/plugins/update';
   addRxPlugin(RxDBUpdatePlugin);
```

This error occurs because we were using the `.update()` method on RxDB documents without including the required update plugin.

## Affected Code

The issue affects the following repositories that use the `.update()` method:

1. `threadRepository.updateThread()` - Used to update thread timestamps and metadata
2. `messageRepository.updateMessage()` - Used to update message content
3. `settingsRepository.updateSettings()` - Used to update user settings

## Solution

The fix was straightforward - we needed to add the RxDBUpdatePlugin to the database initialization in `database.ts`:

```typescript
import { RxDBUpdatePlugin } from 'rxdb/plugins/update';

// Add required plugins
addRxPlugin(RxDBQueryBuilderPlugin);
addRxPlugin(RxDBUpdatePlugin);
```

## Implementation Notes

1. Added the import for RxDBUpdatePlugin from 'rxdb/plugins/update'
2. Added the plugin registration via addRxPlugin
3. This enables the update functionality across all repositories

## Benefits

1. Proper updating of documents in the database
2. Thread timestamps now correctly update when messages are added
3. Message updates and settings changes work as expected

## Related Issues

This fix is part of the complete RxDB persistence implementation for the chat system, specifically addressing an issue with updating documents after they've been created.