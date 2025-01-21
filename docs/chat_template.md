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
- Scrollable messages area
- Fixed-height input bar at bottom
- Input spans full width without border
- Send button with left border only

#### Right Sidebar (`w-64`)
- Same width as left sidebar
- White left border
- Available for additional content

### Input Area
- Matches header height (57px)
- White top border
- Borderless input field
- Send button with left border only
- Hover effects on interactive elements

## Template Files

### Base Layout (`templates/layouts/chat_base.html`)
- Provides the basic HTML structure
- Includes core CSS and JavaScript dependencies
- Implements the three-column layout
- Maintains consistent styling across all sections

### Content Layout (`templates/layouts/chat_content.html`)
- Simplified content wrapper for chat messages
- Directly includes the chat page content
- Handles message display and formatting

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

## Route Handler

The chat route uses these templates in its handler:

```rust
async fn chat(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Chat";
    let path = "/chat";

    if is_htmx {
        let content = ChatContentTemplate.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = ChatPageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}
```

## HTMX Integration

The chat templates maintain HTMX compatibility:
- Full page loads use `ChatPageTemplate`
- HTMX partial updates use `ChatContentTemplate`
- Title updates are handled via HX-Title header

## Styling

The chat interface maintains visual consistency through:
- Black background with white borders
- Monospace font throughout
- Consistent heights for header and input areas
- Clean, minimal design with subtle hover effects
- Proper spacing and padding in all sections

## Purpose

This specialized template structure provides:
- A focused chat experience
- Clear visual hierarchy
- Consistent spacing and alignment
- Support for both full page loads and HTMX partial updates
- Flexibility for additional features in sidebars