# Chat Template Structure

The chat interface uses a specialized template structure that differs from the main site layout. While maintaining the same visual styling, it provides a focused chat experience with a three-column layout and persistent header.

## Layout Components

### Header (`h-[57px]`)

- Full-width header with white bottom border
- Three sections matching the layout below:
  - Left (w-64): OpenAgents link centered
  - Center (flex-1): Available for additional content
  - Right (w-64): Available for additional content

### Main Layout

Three-column design with consistent widths:

#### Left Sidebar (`w-64`)

- New Chat button at top
- Scrollable chat list area
- White right border

#### Main Chat Area (`flex-1`)

- Scrollable messages area with user and AI messages
- Fixed-height input bar at bottom
- Input spans full width without border
- Send button with left border only
- WebSocket connection status indicator

#### Right Sidebar (`w-64`)

- Same width as left sidebar
- White left border
- Available for additional content (model settings, etc)

### Input Area

- Matches header height (57px)
- White top border
- Borderless input field
- Send button with left border only
- Hover effects on interactive elements
- Disabled state during message processing

## Template Files

### Base Layout (`templates/layouts/chat_base.html`)

- Provides the basic HTML structure
- Includes core CSS and JavaScript dependencies
- Implements the three-column layout
- Sets up WebSocket connection
- Handles message sending and receiving
- Maintains consistent styling across all sections

### Content Layout (`templates/layouts/chat_content.html`)

- Simplified content wrapper for chat messages
- Directly includes the chat page content
- Handles message display and formatting
- Supports streaming message updates

## Template Structs

The chat templates are implemented using two Rust structs:

```rust
#[derive(Template)]
#[template(path = "layouts/chat_base.html", escape = "none")]
pub struct ChatPageTemplate<'a> {
    pub title: &'a str,
    pub path: &'a str,
}

#[derive(Template)]
#[template(path = "layouts/chat_content.html", escape = "none")]
pub struct ChatContentTemplate;
```

## WebSocket Integration

The chat interface maintains a WebSocket connection for real-time messaging:

- Connection established on page load
- Automatic reconnection on disconnection
- Message type handling for chat and system messages
- Support for streaming responses
- Connection status indicators

## Message Format

Messages follow a structured format:

```javascript
// User message
{
    "content": "Hello, how can you help me?"
}

// Or with explicit type
{
    "type": "chat",
    "message": {
        "type": "user_message",
        "content": "Hello"
    }
}

// AI response
{
    "type": "chat",
    "message": {
        "type": "agent_response",
        "content": "I can help you with..."
    }
}
```

## HTMX Integration

The chat templates maintain HTMX compatibility:

- Full page loads use `ChatPageTemplate`
- HTMX partial updates use `ChatContentTemplate`
- Title updates are handled via HX-Title header
- WebSocket connections persist across HTMX updates

## Styling

The chat interface maintains visual consistency through:

- Black background with white borders
- Monospace font throughout
- Consistent heights for header and input areas
- Clean, minimal design with subtle hover effects
- Proper spacing and padding in all sections
- Visual feedback for connection status
- Message status indicators

## Purpose

This specialized template structure provides:

- A focused chat experience
- Real-time message updates via WebSocket
- Clear visual hierarchy
- Consistent spacing and alignment
- Support for both full page loads and HTMX partial updates
- Flexibility for additional features in sidebars
- Proper handling of streaming responses