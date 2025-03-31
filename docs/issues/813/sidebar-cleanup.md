# Sidebar UI Cleanup

## Problem

There was a duplicate "New Chat" button in the sidebar. One was part of the ThreadList component, and another was explicitly added in the HomePage's sidebar markup:

```jsx
<SidebarContent>
  <SidebarGroup>
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarMenuButton onClick={handleCreateThread}>
          <MessageSquareIcon className="mr-2" />
          <span>New Chat</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>

    <ThreadList
      currentThreadId={currentThreadId ?? ''}
      onSelectThread={handleSelectThread}
      onDeleteThread={handleDeleteThread}
      onRenameThread={handleRenameThread}
      onCreateThread={handleCreateThread}
    />
  </SidebarGroup>
</SidebarContent>
```

This created redundancy and a confusing UI with two different buttons for the same action.

## Fix

Removed the redundant "New Chat" button from the HomePage.tsx file, keeping only the one that's part of the ThreadList component:

```jsx
<SidebarContent>
  <SidebarGroup>
    <ThreadList
      currentThreadId={currentThreadId ?? ''}
      onSelectThread={handleSelectThread}
      onDeleteThread={handleDeleteThread}
      onRenameThread={handleRenameThread}
      onCreateThread={handleCreateThread}
    />
  </SidebarGroup>
</SidebarContent>
```

## Impact

The UI is now cleaner and more intuitive with a single button for creating new chats. This removes confusion and redundancy from the interface.