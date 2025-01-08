# HTMX Nostr Chat Extension Specification

This document specifies the `nostr-chat` HTMX extension that implements [NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md) for public chat functionality.

## Overview

The `nostr-chat` extension provides a declarative way to interact with Nostr chat channels using HTMX attributes. It handles:

- Channel creation and management
- Message posting and threading
- Real-time updates
- Client-side moderation
- Message templates and rendering

## Installation

```html
<!-- Include NDK -->
<script src="https://unpkg.com/@nostr-dev-kit/ndk@{VERSION}/dist/browser/ndk.js"></script>

<!-- Include the extension -->
<script src="/static/nostr/nostr-chat.js"></script>
```

## Configuration

Global configuration can be set through a JavaScript object:

```javascript
htmx.defineConfig({
  nostrChat: {
    defaultRelays: ['wss://relay.damus.io', 'wss://nos.lol'],
    messageTemplate: '#message-template',
    autoScroll: true,
    moderationEnabled: true,
    pollInterval: 5000, // ms between updates
    messageLimit: 50    // max messages to show
  }
})
```

## Attributes

### Channel Management

#### Creating a Channel
```html
<form hx-ext="nostr-chat" 
      nostr-chat-create="true"
      hx-trigger="submit">
  <input name="name" required>
  <input name="about">
  <input name="picture">
  <input name="relays" value="wss://relay1.com,wss://relay2.com">
  <button type="submit">Create Channel</button>
</form>
```

#### Subscribing to a Channel
```html
<div hx-ext="nostr-chat"
     nostr-chat-channel="<channel_id>"
     nostr-chat-relays="wss://relay1.com,wss://relay2.com"
     nostr-chat-template="#custom-template">
  <div id="messages"></div>
</div>
```

### Message Operations

#### Posting Messages
```html
<form hx-ext="nostr-chat"
      nostr-chat-post="true"
      nostr-chat-channel="<channel_id>"
      hx-trigger="submit">
  <input name="content" required>
  <button type="submit">Send</button>
</form>
```

#### Replying to Messages
```html
<form hx-ext="nostr-chat"
      nostr-chat-reply="<message_id>"
      nostr-chat-channel="<channel_id>"
      hx-trigger="submit">
  <input name="content" required>
  <button type="submit">Reply</button>
</form>
```

### Moderation

#### Hide Message
```html
<button hx-ext="nostr-chat"
        nostr-chat-hide="<message_id>"
        nostr-chat-reason="spam"
        hx-trigger="click">
  Hide
</button>
```

#### Mute User
```html
<button hx-ext="nostr-chat"
        nostr-chat-mute="<pubkey>"
        nostr-chat-reason="harassment"
        hx-trigger="click">
  Mute
</button>
```

## Templates

### Default Message Template
```html
<template id="message-template">
  <div class="message" id="msg-{{id}}" 
       data-pubkey="{{pubkey}}" 
       data-timestamp="{{created_at}}">
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

### Template Variables
- `{{id}}` - Event ID
- `{{pubkey}}` - Full public key
- `{{pubkey_short}}` - Shortened public key (first 8 chars)
- `{{content}}` - Message content
- `{{created_at}}` - Timestamp
- `{{formatted_time}}` - Human readable time
- `{{channel_id}}` - Channel ID
- `{{reply_to}}` - ID of message being replied to (if applicable)
- `{{relay_url}}` - Source relay URL

## Events

The extension triggers several custom events:

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

### Moderation Events
- `nostr-chat:user-muted` - User was muted
- `nostr-chat:moderation-sync` - Moderation state synced

## Storage

The extension maintains state in:

### LocalStorage
- Hidden messages
- Muted users
- Channel metadata cache
- User preferences

### Memory
- Active subscriptions
- Message cache
- Template cache
- Current channel state

## Error Handling

The extension adds error classes to elements when operations fail:

```html
<!-- Example error states -->
<form class="nostr-chat-error" 
      data-nostr-chat-error="Failed to send message: relay unreachable">
  ...
</form>
```

Error events include detailed information:
```javascript
document.body.addEventListener('nostr-chat:error', (e) => {
  console.error(e.detail.message, e.detail.code, e.detail.context)
})
```

## Security Considerations

1. Content Sanitization
   - All message content is HTML-escaped by default
   - Links are rendered as clickable but with noopener/noreferrer
   - Images are lazy-loaded with placeholder

2. Rate Limiting
   - Message posting is rate-limited per channel
   - Subscription connections are pooled and reused
   - Local caching reduces relay load

3. Privacy
   - No automatic loading of external content
   - Optional client-side encryption support
   - Moderation actions are stored locally only

## Example Implementation

Complete example showing common usage:

```html
<!-- Channel container -->
<div class="chat-container">
  <!-- Channel subscription -->
  <div hx-ext="nostr-chat"
       nostr-chat-channel="<channel_id>"
       nostr-chat-relays="wss://relay1.com,wss://relay2.com">
    
    <!-- Messages will appear here -->
    <div id="messages"></div>
    
    <!-- Message input -->
    <form nostr-chat-post="true" hx-trigger="submit">
      <input name="content" 
             placeholder="Type a message..."
             required>
      <button type="submit">Send</button>
    </form>
  </div>
  
  <!-- Moderation panel -->
  <div class="moderation-panel">
    <h3>Hidden Messages</h3>
    <div id="hidden-messages"></div>
    
    <h3>Muted Users</h3>
    <div id="muted-users"></div>
  </div>
</div>

<!-- Message template -->
<template id="message-template">
  <!-- As shown above -->
</template>
```

## Development and Testing

1. Local Development
```bash
# Install dependencies
npm install

# Build extension
npm run build

# Run tests
npm test
```

2. Testing Utilities
```javascript
// Mock relay for testing
const mockRelay = new NostrChatMockRelay()

// Test helpers
NostrChat.debugMode = true
NostrChat.logLevel = 'debug'
```

3. Browser Support
- Modern browsers (Chrome, Firefox, Safari, Edge)
- Fallback behavior for older browsers
- Progressive enhancement approach