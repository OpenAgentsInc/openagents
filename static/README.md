# Nostr-HTMX Integration

This project demonstrates a lightweight integration between Nostr and HTMX, enabling real-time Nostr event subscriptions with minimal JavaScript.

## How It Works

### HTMX Extension

The project creates a custom HTMX extension called 'nostr-sub' that bridges Nostr and HTMX. This is used in the HTML with:

```html
hx-ext="nostr-sub, client-side-templates"
```

### Form Triggering

When you submit the form, HTMX triggers the Nostr subscription through:

```html
hx-trigger="submit"
```

### Data Flow

When you input a pubkey and click "ok":

1. The form triggers the HTMX extension
2. The extension creates a Nostr subscription with the filter `{"kinds":[0]}` plus the author from the form
3. When events come in from Nostr, they're transformed into HTML using the Mustache template

### Template Rendering

The Mustache template defines how each Nostr event is displayed:

```html
<template id="event">
  <div id="main" hx-swap-oob="afterbegin">
    at <em>{{created_at}}</em> <b>{{pubkey}}</b> said:
    <p>{{content}}</p>
  </div>
</template>
```

### Real-time Updates

- The Nostr subscription stays active (`closeOnEose: false`)
- New events are automatically rendered and inserted into the page using HTMX's out-of-band swaps (`hx-swap-oob`)
- Events are processed every 5000ms (5 seconds)

## Key Integration Points

- HTMX handles the UI interactions and DOM updates
- Nostr handles the real-time data subscription
- The custom extension ('nostr-sub') bridges between them
- Mustache templates handle the rendering of Nostr events into HTML

This creates a reactive UI where Nostr events are automatically rendered into the page without needing to write complex JavaScript event handlers or DOM manipulation code.

## Getting Started

1. Install dependencies:

```bash
npm install
```

2. Install just command runner:

```bash
# On macOS:
brew install just

# On Linux:
# Check your package manager or visit https://github.com/casey/just#installation

# On Windows:
choco install just
# or
scoop install just
```

3. Install esbuild:

```bash
npm install -g esbuild
```

4. Build the project:

```bash
just build
```

5. Serve the project (using Python's built-in server):

```bash
python3 -m http.server
```

Then visit http://localhost:8000 in your browser.

## Testing

You can test the application by entering a Nostr public key. Here's a sample pubkey to try:

```
3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d
```

## License

[Add appropriate license]
