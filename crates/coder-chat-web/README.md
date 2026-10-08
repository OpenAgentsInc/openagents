# Chat browser interaction

This Rust/Wasm adapter adds composer keyboard input, card focus, in-memory
drafts, and transcript scroll behavior. HTMX sends forms and receives server
HTML and SSE fragments. The adapter calls no host or provider API and uses no
browser storage.

Keep `#chat-form`, `#chat-input`, and `#chat-card` mounted outside transcript
swaps. Use `#chat-thread` for the scroll container. The server identifies the
selected chat with `#demo-selected`, `#chat-selected`, or the form's
`data-chat-id`; it supplies and replaces request IDs. History controls use
`data-chat-history="start"` or `data-chat-history="end"`. Set
`data-chat-scroll="start"` on the initial thread to open at the beginning.

Enter submits and Shift+Enter inserts a line. Composition input never submits.
A successful HTMX form POST clears the submitted draft if it is still current;
failed requests and text typed while a request runs remain intact. Transcript
updates preserve the textarea and follow the latest message only when the
reader is near the bottom. Selecting another chat restores its draft, caret,
and scroll position from memory.

When the server replaces `#chat-selected`, the adapter points the stable form
at `/chat/{uuid}`. SSE message IDs use `{uuid}:{revision}` for real chats and
`{index}:{revision}` for demo chats. The adapter cancels messages whose prefix
does not match the current server-selected chat.

Configure HTMX with `allowEval: false`, `allowScriptTags: false`, and
`selfRequestsOnly: true` before processing page markup. `start()` reinforces
these settings without evaluating script. Load the self-hosted HTMX and SSE
extension before the generated Wasm loader.

Private account views retain `coder-cloud-web` as their privacy owner. Place
their composer and transcript inside `#cloud-private`. This adapter checks the
privacy-ready marker, clears drafts on `openagents-cloud-retired`, and blocks
late requests and swaps after retirement. The server still checks authority
for every request and stream. Page departure clears all local drafts.
