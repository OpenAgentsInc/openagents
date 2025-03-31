# Thread Rename Dialog Fix

## Problem

When clicking the "Edit" button in the ThreadList component, the following error was occurring:

```
ThreadList.tsx:123 Uncaught Error: prompt() is not supported.
    at onClick (ThreadList.tsx:123:47)
```

The issue was that the thread renaming functionality was using `window.prompt()`, which is not supported in Electron environment.

## Root Cause

The ThreadList component was using the browser's native `window.prompt()` function to get user input for renaming threads:

```typescript
const newTitle = window.prompt('Enter new title:', thread.title);
if (newTitle) {
  onRenameThread(thread.id, newTitle);
}
```

However, the `prompt()` method is not supported in Electron for security reasons.

## Fix

Replaced the browser's native `window.prompt()` with a proper React-based dialog component:

1. Added state variables to manage the dialog:
```typescript
const [isRenameDialogOpen, setIsRenameDialogOpen] = useState(false);
const [threadToRename, setThreadToRename] = useState<Thread | null>(null);
const [newTitle, setNewTitle] = useState('');
```

2. Modified the edit button to open the dialog instead of using prompt():
```typescript
onClick={(e) => {
  e.stopPropagation();
  setThreadToRename(thread);
  setNewTitle(thread.title || '');
  setIsRenameDialogOpen(true);
}}
```

3. Added a proper dialog component with input field and buttons:
```jsx
<Dialog open={isRenameDialogOpen} onOpenChange={setIsRenameDialogOpen}>
  <DialogContent className="sm:max-w-md">
    <DialogHeader>
      <DialogTitle>Rename Chat</DialogTitle>
    </DialogHeader>
    <div className="py-4">
      <Input
        value={newTitle}
        onChange={(e) => setNewTitle(e.target.value)}
        placeholder="Enter new title"
        className="w-full"
        autoFocus
        onKeyDown={(e) => {
          if (e.key === 'Enter' && threadToRename && onRenameThread) {
            onRenameThread(threadToRename.id, newTitle);
            setIsRenameDialogOpen(false);
          }
        }}
      />
    </div>
    <DialogFooter>
      <Button variant="outline" onClick={() => setIsRenameDialogOpen(false)}>
        Cancel
      </Button>
      <Button
        onClick={() => {
          if (threadToRename && onRenameThread) {
            onRenameThread(threadToRename.id, newTitle);
            setIsRenameDialogOpen(false);
          }
        }}
      >
        Save
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
```

## Impact

The thread rename functionality now works properly in both web and Electron environments. The user experience is also improved with a more polished UI element rather than the browser's basic prompt dialog.