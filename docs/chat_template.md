# Chat Template Structure

The chat interface uses a specialized template structure that differs from the main site layout. While maintaining the same visual styling, it removes the navigation header for a more focused chat experience.

## Template Files

### Base Layout (`templates/layouts/chat_base.html`)
- Provides the basic HTML structure
- Includes core CSS and JavaScript dependencies
- Maintains the same styling as the main site
- Omits the site-wide navigation header
- Includes only chat-specific content

### Content Layout (`templates/layouts/chat_content.html`)
- Simplified content wrapper
- Directly includes the chat page content
- No additional navigation or structural elements

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

While using a different layout structure, the chat interface maintains visual consistency by:
- Using the same CSS files (`main.css`, `fonts.css`)
- Keeping the same color scheme and typography
- Maintaining the same border and spacing patterns

## Purpose

This specialized template structure allows the chat interface to:
- Provide a more focused user experience
- Remove unnecessary navigation elements
- Maintain visual consistency with the main site
- Support both full page loads and HTMX partial updates