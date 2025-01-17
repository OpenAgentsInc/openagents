# Template System Guide

## Overview
Our website uses Askama for templating. This document explains how to handle HTML content, markdown rendering, and common issues with template filters.

## HTML Escaping

### The Problem
By default, Askama escapes HTML content for security. This means if you pass HTML content to a template, it will be displayed as text:

```html
<!-- What you write -->
<p>Hello <strong>world</strong></p>

<!-- What gets displayed -->
&lt;p&gt;Hello &lt;strong&gt;world&lt;/strong&gt;&lt;/p&gt;
```

### Solutions

There are several ways to handle HTML content in templates:

1. Using the `safe` filter:
```html
{{ content | safe }}
```

2. Using `escape = "none"` in template attributes:
```rust
#[derive(Template)]
#[template(path = "my_template.html", escape = "none")]
struct MyTemplate {
    content: String,
}
```

3. Implementing a custom filter:
```rust
pub mod filters {
    pub fn safe(s: &str) -> ::askama::Result<String> {
        Ok(s.to_string())
    }
}
```

## Markdown Rendering

### Current Implementation
We use pulldown-cmark to render markdown to HTML. The process is:

1. Convert markdown to HTML:
```rust
pub fn render_markdown(content: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    
    let parser = Parser::new_ext(content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    
    html_output
}
```

2. Pass the rendered HTML to templates:
```html
{% call blog::blog_post(
    "Title",
    "Date",
    render_markdown("**Bold** text")
) %}
```

### Common Issues

1. **Double Escaping**: If you see HTML tags displayed as text, you're hitting double escaping. Solutions:
   - Use the `safe` filter
   - Set `escape = "none"` on the template
   - Pre-render HTML before passing to template

2. **Filter Not Found**: Errors like `no field 'safe' on type &Template<'a>` mean Askama can't find the filter. Solutions:
   - Add the filter as a method on the template struct
   - Use built-in filters
   - Configure filters in .askama.toml

3. **Method vs Field**: Errors about taking value of method indicate confusion between methods and fields. Solutions:
   - Use fields instead of methods for filters
   - Implement filters as standalone functions
   - Use built-in Askama functionality

## Best Practices

1. **Pre-render HTML**: When possible, convert markdown to HTML before passing to templates:
```rust
let html = render_markdown(markdown_content);
template.render(html)
```

2. **Use Built-in Filters**: Prefer Askama's built-in filters over custom ones:
```html
{{ content | escape }}
{{ content | safe }}
```

3. **Template Structure**: Keep templates simple and move logic to Rust code:
```html
<!-- Good -->
<div>{{ pre_processed_content }}</div>

<!-- Avoid -->
<div>{{ content | my_filter | another_filter }}</div>
```

4. **Error Handling**: Always handle rendering errors:
```rust
match template.render() {
    Ok(html) => html,
    Err(e) => handle_error(e),
}
```

## Configuration

### .askama.toml
If using custom filters, register them in .askama.toml:
```toml
[general]
dirs = ["templates"]

[general.filters]
markdown = "crate::filters::markdown"
safe = "askama::filters::safe"
```

### Template Attributes
Use template attributes to control behavior:
```rust
#[derive(Template)]
#[template(path = "template.html", escape = "none")]
struct Template {
    content: String,
}
```