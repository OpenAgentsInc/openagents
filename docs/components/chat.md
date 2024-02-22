# Chat

openagents.com/chat loads the Chat component:
- app/livewire/Chat.php
- resources/views/livewire/chat.php

Chat is a Livewire component with subcomponents:
- ChatSidebar
  - app/livewire/ChatSidebar.php
  - resources/views/livewire/chat-sidebar.php
  - Subcomponents:
    - SessionItem - List item representing an individual chat session.
- ChatHeader
  - app/livewire/ChatHeader.php
  - resources/views/livewire/chat-header.php
  - Subcomponents:
    - SessionDetails - Displays the current session's title and other meta information.
    - StatusIndicator - Shows the online/offline status of the user or session.
- ChatMessages
  - app/livewire/ChatMessages.php
  - resources/views/livewire/chat-messages.php
  - Subcomponents:
    - MessageBubble - Individual message container with text and background.
    - Timestamp - Time indicator for each message.
- ChatInput
  - app/livewire/ChatInput.php
  - resources/views/livewire/chat-input.php
  - Subcomponents:
    - TextInputField - Field where the user types their message.
    - SendButton - Button to submit the typed message.
