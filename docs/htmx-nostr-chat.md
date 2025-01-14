# HTMX Nostr Chat Extension Specification

This document specifies the `nostr-chat` HTMX extension that implements [NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md) for public chat functionality.

## Overview

The `nostr-chat` extension provides a declarative way to interact with Nostr chat channels using HTMX attributes. It handles:

- Channel creation and management
- Message posting and threading
- Real-time updates
- Client-side moderation
- Message templates and rendering

## Architecture

The extension is built using a composition-based architecture with these main components:

1. `NostrChatBase` - Base class with shared state and utilities
2. `ChannelMethods` - Channel-related functionality
3. `MessageMethods` - Message-related functionality
4. `ChatStorage` - Local storage management

### File Structure

```
static/nostr/
├── base.ts           # Base class with shared state
├── channel-methods.ts # Channel operations
├── message-methods.ts # Message operations
├── storage.ts        # Local storage management
├── types.ts         # TypeScript interfaces
└── nostr-chat.ts    # Main extension class
```

## Installation

```html
<!-- Include NDK -->
<script src="../dist/ndk.js"></script>

<!-- Include the extension -->
<script src="../dist/nostr/nostr-chat.js"></script>
```

## Configuration

The extension uses NDK (Nostr Development Kit) for Nostr interactions:

```typescript
const config = {
  defaultRelays: [
    "wss://nostr-pub.wellorder.net",
    "wss://nostr.mom",
    "wss://relay.nostr.band",
  ],
  messageTemplate: "#message-template",
  autoScroll: true,
  moderationEnabled: true,
  pollInterval: 5000,
  messageLimit: 50,
};
```

## HTML Structure

### Channel Creation Form

```html
<div class="channel-form">
  <h2>Create New Channel</h2>
  <form
    id="create-channel-form"
    nostr-chat-create="true"
    hx-on="submit: event.preventDefault()"
  >
    <div class="form-group">
      <label for="channel-name">Channel Name</label>
      <input type="text" id="channel-name" name="name" required />
    </div>
    <div class="form-group">
      <label for="channel-about">Description</label>
      <textarea id="channel-about" name="about" required></textarea>
    </div>
    <div class="form-group">
      <label for="channel-picture">Picture URL (optional)</label>
      <input type="url" id="channel-picture" name="picture" />
    </div>
    <button type="submit">Create Channel</button>
  </form>
</div>
```

### Channel List

```html
<div class="channel-list">
  <h3>Your Channels</h3>
  <div id="channel-items">
    <!-- Channel items will be inserted here -->
  </div>
</div>
```

### Chat Interface

```html
<div id="chat-interface" style="display: none;">
  <!-- Channel metadata -->
  <div data-channel-metadata></div>

  <!-- Chat messages -->
  <div hx-ext="nostr-chat" class="chat-container">
    <div class="messages" data-messages></div>

    <!-- Message input -->
    <form
      class="message-form"
      nostr-chat-post="true"
      hx-on="submit: event.preventDefault()"
    >
      <input name="content" placeholder="Type a message..." required />
      <button type="submit">Send</button>
    </form>
  </div>
</div>
```

## Templates

### Message Template

```html
<template id="message-template">
  <div
    class="message"
    id="msg-{{id}}"
    data-pubkey="{{pubkey}}"
    data-timestamp="{{created_at}}"
  >
    <span class="author">{{pubkey_short}}</span>
    <span class="time">{{formatted_time}}</span>
    <div class="content">{{content}}</div>
    <div class="actions">
      <button nostr-chat-reply="{{id}}">Reply</button>
      <button nostr-chat-hide="{{id}}">Hide</button>
      <button nostr-chat-mute="{{pubkey}}">Mute User</button>
    </div>
  </div>
</template>
```

### Channel Metadata Template

```html
<template id="channel-metadata-template">
  <div class="channel-header">
    <h2>{{name}}</h2>
    <p>{{about}}</p>
    <img
      src="{{picture}}"
      alt="Channel picture"
      style="max-width: 100px; border-radius: 50%;"
      onerror="this.style.display='none'"
    />
  </div>
</template>
```

### Channel Item Template

```html
<template id="channel-item-template">
  <div class="channel-item" data-channel-id="{{id}}">
    <h4>{{name}}</h4>
    <p>{{about}}</p>
  </div>
</template>
```

## Events

The extension dispatches several custom events:

### Channel Events

- `nostr-chat:channel-created` - New channel created
- `nostr-chat:channel-updated` - Channel metadata updated
- `nostr-chat:channel-subscribed` - Successfully subscribed to channel
- `nostr-chat:channel-error` - Error in channel operations

### Message Events

- `nostr-chat:message-sent` - Message successfully sent
- `nostr-chat:message-received` - New message received
- `nostr-chat:message-hidden` - Message was hidden
- `nostr-chat:message-error` - Error sending/receiving message

## Storage

The extension maintains state in:

### LocalStorage

- Hidden messages
- Muted users
- Channel metadata cache

### Memory

- Active subscriptions
- Message cache
- Template cache
- Current channel state

## Error Handling

The extension shows error messages in two ways:

1. Visual Feedback:

```html
<div class="error-message">
  Failed to send message. Make sure your Nostr extension is unlocked.
</div>
```

2. Console Logs:

```javascript
console.error("Failed to send message:", error);
this.dispatchEvent("nostr-chat:error", { message, error });
```

## Security Considerations

1. **Content Sanitization**

   - All message content is HTML-escaped
   - Images are lazy-loaded with error handling
   - Links are not auto-converted

2. **Authentication**

   - Uses NIP-07 for signing
   - Requires a Nostr extension (like nos2x or Alby)
   - Verifies pubkey ownership

3. **Data Privacy**
   - Local storage for user preferences
   - No sensitive data cached
   - Moderation actions stored locally

## Development

1. Build the extension:

```bash
cd static
just build
```

2. Watch for changes:

```bash
just dev
```

3. Build for production:

```bash
just prod
```

## Dependencies

- NDK (Nostr Development Kit)
- HTMX
- Mustache (for templating)

## Browser Support

- Modern browsers (Chrome, Firefox, Safari, Edge)
- Requires NIP-07 compatible extension
- Progressive enhancement for older browsers

## License

[Add appropriate license]
